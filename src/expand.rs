use std::borrow::Cow;

use crate::arena::ByteArena;
use crate::bstr;
use crate::syntax::Word;
use crate::sys;

#[derive(Debug)]
pub struct ExpandError {
    pub message: Box<[u8]>,
}

pub trait Context {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>>;
    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>>;
    fn positional_params(&self) -> &[Vec<u8>];
    fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &[u8];
    fn command_substitute(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError>;
    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn set_lineno(&mut self, _line: usize) {}
    fn inc_lineno(&mut self) {}
    fn lineno(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    Quoted,
    Literal,
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(Vec<u8>, QuoteState),
    AtBreak,
    AtEmpty,
}

#[derive(Debug, PartialEq, Eq)]
enum Expansion {
    One(Vec<u8>),
    AtFields(Vec<Vec<u8>>),
}

pub fn expand_words<'a, C: Context>(
    ctx: &mut C,
    words: &[Word],
    arena: &'a ByteArena,
) -> Result<Vec<&'a [u8]>, ExpandError> {
    let mut result = Vec::new();
    for word in words {
        result.extend(expand_word(ctx, word, arena)?);
    }
    Ok(result)
}

pub fn expand_word_as_declaration_assignment<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(word.line);
    let value_raw = word_assignment_value(&word.raw).unwrap_or(&word.raw);
    let name = &word.raw[..word.raw.len() - value_raw.len()];
    let value_word = Word {
        raw: value_raw.into(),
        line: word.line,
    };
    let expanded_value = expand_word_text_assignment(ctx, &value_word, true, arena)?;
    let mut combined = Vec::with_capacity(name.len() + expanded_value.len());
    combined.extend_from_slice(name);
    combined.extend_from_slice(expanded_value);
    Ok(arena.intern_vec(combined))
}

pub fn word_is_assignment(raw: &[u8]) -> bool {
    word_assignment_value(raw).is_some()
}

fn word_assignment_value(raw: &[u8]) -> Option<&[u8]> {
    if raw.is_empty() {
        return None;
    }
    let first = raw[0];
    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return None;
    }
    let mut i = 1;
    while i < raw.len() {
        let b = raw[i];
        if b == b'=' {
            return Some(&raw[i + 1..]);
        }
        if !(b == b'_' || b.is_ascii_alphanumeric()) {
            return None;
        }
        i += 1;
    }
    None
}

pub fn expand_word<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<Vec<&'a [u8]>, ExpandError> {
    ctx.set_lineno(word.line);
    let expanded = expand_raw(ctx, &word.raw)?;

    if expanded.has_at_expansion {
        let fields = expand_word_with_at_fields(&expanded, expanded.had_quoted_null_outside_at)?;
        return Ok(fields.into_iter().map(|s| arena.intern_vec(s)).collect());
    }

    if expanded.segments.is_empty() {
        if expanded.had_quoted_content {
            return Ok(vec![arena.intern_vec(Vec::new())]);
        }
        return Ok(Vec::new());
    }

    let has_expanded = expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, QuoteState::Expanded)));

    let ifs_cow = ctx.env_var(b"IFS").unwrap_or(Cow::Borrowed(b" \t\n"));
    let fields = if has_expanded {
        split_fields_from_segments(&expanded.segments, &ifs_cow)
    } else {
        let has_glob = expanded.segments.iter().any(|seg| {
            matches!(seg, Segment::Text(text, QuoteState::Literal) if text.iter().any(|&b| is_glob_byte(b)))
        });
        vec![Field {
            text: flatten_segments(&expanded.segments),
            has_unquoted_glob: has_glob,
        }]
    };
    if fields.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for field in fields {
        if field.has_unquoted_glob && ctx.pathname_expansion_enabled() {
            let matches = expand_pathname(&field.text);
            if matches.is_empty() {
                result.push(arena.intern_vec(field.text));
            } else {
                for m in matches {
                    result.push(arena.intern_vec(m));
                }
            }
        } else {
            result.push(arena.intern_vec(field.text));
        }
    }
    Ok(result)
}

pub fn expand_redirect_word<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(word.line);
    let expanded = expand_raw(ctx, &word.raw)?;

    if expanded.segments.is_empty() {
        return Ok(arena.intern_vec(Vec::new()));
    }

    let has_expanded = expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, QuoteState::Expanded)));

    let ifs_cow = ctx.env_var(b"IFS").unwrap_or(Cow::Borrowed(b" \t\n"));
    let fields = if has_expanded {
        split_fields_from_segments(&expanded.segments, &ifs_cow)
    } else {
        vec![Field {
            text: flatten_segments(&expanded.segments),
            has_unquoted_glob: false,
        }]
    };

    Ok(arena.intern_vec(
        bstr::join_bstrings(
            &fields.into_iter().map(|f| f.text).collect::<Vec<_>>(),
            b" ",
        ),
    ))
}

fn expand_word_with_at_fields(
    expanded: &ExpandedWord,
    had_quoted_null_outside_at: bool,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let has_at_empty = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtEmpty));
    let has_at_break = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak));

    if has_at_empty && !has_at_break {
        let mut text = Vec::new();
        for seg in &expanded.segments {
            if let Segment::Text(t, _) = seg {
                text.extend_from_slice(t);
            }
        }
        if !text.is_empty() || had_quoted_null_outside_at {
            return Ok(vec![text]);
        }
        return Ok(Vec::new());
    }

    if !has_at_break {
        return Ok(vec![flatten_segments(&expanded.segments)]);
    }

    let mut fields = Vec::new();
    let mut current = Vec::new();

    for seg in &expanded.segments {
        if let Segment::Text(text, _) = seg {
            current.extend_from_slice(text);
        } else if matches!(seg, Segment::AtBreak) {
            fields.push(std::mem::take(&mut current));
        }
    }
    fields.push(current);

    Ok(fields)
}

pub fn expand_word_text<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(word.line);
    expand_word_text_assignment(ctx, word, false, arena)
}

pub fn expand_word_pattern<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(word.line);
    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(arena.intern_vec(render_pattern_from_segments(&expanded.segments)))
}

pub fn expand_assignment_value<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(word.line);
    expand_word_text_assignment(ctx, word, true, arena)
}

fn expand_word_text_assignment<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    assignment_rhs: bool,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    if !assignment_rhs {
        let expanded = expand_raw(ctx, &word.raw)?;
        return Ok(arena.intern_vec(flatten_segments(&expanded.segments)));
    }
    let raw = &word.raw;
    let mut result = Vec::new();
    let mut first = true;
    for part in split_on_unquoted_colons(raw) {
        if !first {
            result.push(b':');
        }
        first = false;
        let expanded = expand_raw(ctx, &part)?;
        result.extend_from_slice(&flatten_segments(&expanded.segments));
    }
    Ok(arena.intern_vec(result))
}

fn split_on_unquoted_colons(raw: &[u8]) -> Vec<Vec<u8>> {
    let mut parts = Vec::new();
    let mut current = Vec::new();
    let mut i = 0;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    while i < raw.len() {
        match raw[i] {
            b'\'' if brace_depth == 0 && paren_depth == 0 => {
                current.push(b'\'');
                i += 1;
                while i < raw.len() && raw[i] != b'\'' {
                    current.push(raw[i]);
                    i += 1;
                }
                if i < raw.len() {
                    current.push(b'\'');
                    i += 1;
                }
            }
            b'"' if brace_depth == 0 && paren_depth == 0 => {
                current.push(b'"');
                i += 1;
                while i < raw.len() && raw[i] != b'"' {
                    if raw[i] == b'\\' && i + 1 < raw.len() {
                        current.push(b'\\');
                        i += 1;
                    }
                    current.push(raw[i]);
                    i += 1;
                }
                if i < raw.len() {
                    current.push(b'"');
                    i += 1;
                }
            }
            b'\\' => {
                current.push(b'\\');
                i += 1;
                if i < raw.len() {
                    current.push(raw[i]);
                    i += 1;
                }
            }
            b'$' if i + 1 < raw.len() && raw[i + 1] == b'{' => {
                current.extend_from_slice(b"${");
                brace_depth += 1;
                i += 2;
            }
            b'}' if brace_depth > 0 => {
                brace_depth -= 1;
                current.push(b'}');
                i += 1;
            }
            b'$' if i + 1 < raw.len() && raw[i + 1] == b'(' => {
                current.extend_from_slice(b"$(");
                paren_depth += 1;
                i += 2;
            }
            b')' if paren_depth > 0 => {
                paren_depth -= 1;
                current.push(b')');
                i += 1;
            }
            b':' if brace_depth == 0 && paren_depth == 0 => {
                parts.push(std::mem::take(&mut current));
                i += 1;
            }
            _ => {
                current.push(raw[i]);
                i += 1;
            }
        }
    }
    parts.push(current);
    parts
}

pub fn expand_parameter_text<'a, C: Context>(
    ctx: &mut C,
    raw: &[u8],
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    Ok(arena.intern_vec(expand_parameter_text_owned(ctx, raw)?))
}

fn expand_parameter_text_owned<C: Context>(ctx: &mut C, raw: &[u8]) -> Result<Vec<u8>, ExpandError> {
    let mut result = Vec::new();
    let mut index = 0usize;

    while index < raw.len() {
        if raw[index] == b'$' {
            let (value, consumed) = expand_parameter_dollar(ctx, &raw[index..])?;
            result.extend_from_slice(&value);
            index += consumed;
        } else {
            result.push(raw[index]);
            index += 1;
        }
    }

    Ok(result)
}

fn flatten_expansion(expansion: Expansion) -> Vec<u8> {
    match expansion {
        Expansion::One(s) => s,
        Expansion::AtFields(fields) => bstr::join_bstrings(&fields, b" "),
    }
}

fn apply_expansion(
    segments: &mut Vec<Segment>,
    expansion: Expansion,
    quoted: bool,
    has_at: &mut bool,
) {
    let state = if quoted {
        QuoteState::Quoted
    } else {
        QuoteState::Expanded
    };
    match expansion {
        Expansion::One(s) => push_segment(segments, s, state),
        Expansion::AtFields(params) => {
            *has_at = true;
            if params.is_empty() {
                segments.push(Segment::AtEmpty);
            } else {
                for (i, param) in params.into_iter().enumerate() {
                    if i > 0 {
                        segments.push(Segment::AtBreak);
                    }
                    push_segment(segments, param, QuoteState::Quoted);
                }
            }
        }
    }
}

fn expand_raw<C: Context>(ctx: &mut C, raw: &[u8]) -> Result<ExpandedWord, ExpandError> {
    let mut index = 0usize;
    let mut segments = Vec::new();
    let mut had_quoted_content = false;
    let mut had_quoted_null_outside_at = false;
    let mut has_at_expansion = false;

    while index < raw.len() {
        match raw[index] {
            b'\'' => {
                had_quoted_content = true;
                had_quoted_null_outside_at = true;
                index += 1;
                let start = index;
                while index < raw.len() && raw[index] != b'\'' {
                    if raw[index] == b'\n' {
                        ctx.inc_lineno();
                    }
                    index += 1;
                }
                if index >= raw.len() {
                    return Err(ExpandError {
                        message: b"unterminated single quote".as_ref().into(),
                    });
                }
                push_segment_slice(&mut segments, &raw[start..index], QuoteState::Quoted);
                index += 1;
            }
            b'"' => {
                had_quoted_content = true;
                index += 1;
                let mut buffer = Vec::new();
                let at_before = has_at_expansion;
                while index < raw.len() && raw[index] != b'"' {
                    match raw[index] {
                        b'\\' => {
                            if index + 1 < raw.len() {
                                let next = raw[index + 1];
                                if matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n' | b'}') {
                                    index += 1;
                                    if next == b'\n' {
                                        ctx.inc_lineno();
                                    } else {
                                        buffer.push(next);
                                    }
                                    index += 1;
                                } else {
                                    buffer.push(b'\\');
                                    index += 1;
                                }
                            } else {
                                buffer.push(b'\\');
                                index += 1;
                            }
                        }
                        b'$' => {
                            if !buffer.is_empty() {
                                push_segment(
                                    &mut segments,
                                    std::mem::take(&mut buffer),
                                    QuoteState::Quoted,
                                );
                            }
                            let (expansion, consumed) = expand_dollar(ctx, &raw[index..], true)?;
                            apply_expansion(&mut segments, expansion, true, &mut has_at_expansion);
                            index += consumed;
                        }
                        b'`' => {
                            if !buffer.is_empty() {
                                push_segment(
                                    &mut segments,
                                    std::mem::take(&mut buffer),
                                    QuoteState::Quoted,
                                );
                            }
                            index += 1;
                            let command = scan_backtick_command(raw, &mut index, true)?;
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = trim_trailing_newlines(&output).to_vec();
                            push_segment(&mut segments, trimmed, QuoteState::Quoted);
                        }
                        b'\n' => {
                            ctx.inc_lineno();
                            buffer.push(b'\n');
                            index += 1;
                        }
                        _ => {
                            buffer.push(raw[index]);
                            index += 1;
                        }
                    }
                }
                if index >= raw.len() {
                    return Err(ExpandError {
                        message: b"unterminated double quote".as_ref().into(),
                    });
                }
                if !buffer.is_empty() {
                    push_segment(&mut segments, buffer, QuoteState::Quoted);
                }
                if !has_at_expansion || at_before == has_at_expansion {
                    had_quoted_null_outside_at = true;
                }
                index += 1;
            }
            b'\\' => {
                had_quoted_null_outside_at = true;
                index += 1;
                if index < raw.len() {
                    if raw[index] == b'\n' {
                        ctx.inc_lineno();
                    }
                    push_segment_slice(&mut segments, &raw[index..index + 1], QuoteState::Quoted);
                    index += 1;
                }
            }
            b'$' => {
                let dollar_single_quotes = raw.get(index + 1) == Some(&b'\'');
                if dollar_single_quotes {
                    had_quoted_content = true;
                }
                let (expansion, consumed) = expand_dollar(ctx, &raw[index..], false)?;
                apply_expansion(
                    &mut segments,
                    expansion,
                    dollar_single_quotes,
                    &mut has_at_expansion,
                );
                index += consumed;
            }
            b'`' => {
                index += 1;
                let command = scan_backtick_command(raw, &mut index, false)?;
                let output = ctx.command_substitute(&command)?;
                let trimmed = trim_trailing_newlines(&output).to_vec();
                push_segment(&mut segments, trimmed, QuoteState::Expanded);
            }
            b'~' if index == 0 => {
                index += 1;
                let mut user = Vec::new();
                let at_start = index;
                while index < raw.len() && raw[index] != b'/' {
                    let b = raw[index];
                    if b == b'\'' || b == b'"' || b == b'\\' || b == b'$' || b == b'`' {
                        break;
                    }
                    user.push(raw[index]);
                    index += 1;
                }
                let broke_on_non_login =
                    index == at_start && index < raw.len() && raw[index] != b'/';
                if broke_on_non_login {
                    push_segment_slice(&mut segments, b"~", QuoteState::Literal);
                } else if user.is_empty() {
                    match ctx.env_var(b"HOME") {
                        Some(home) if !home.is_empty() => {
                            push_segment(&mut segments, home.into_owned(), QuoteState::Quoted);
                        }
                        Some(_) => {
                            segments.push(Segment::Text(Vec::new(), QuoteState::Quoted));
                        }
                        None => {
                            push_segment_slice(&mut segments, b"~", QuoteState::Literal);
                        }
                    }
                } else if let Some(dir) = ctx.home_dir_for_user(&user) {
                    push_segment(&mut segments, dir.into_owned(), QuoteState::Quoted);
                } else {
                    let mut literal = vec![b'~'];
                    literal.extend_from_slice(&user);
                    push_segment(&mut segments, literal, QuoteState::Literal);
                }
            }
            b'\n' => {
                ctx.inc_lineno();
                push_segment_slice(&mut segments, b"\n", QuoteState::Literal);
                index += 1;
            }
            _ => {
                push_segment_slice(
                    &mut segments,
                    &raw[index..index + 1],
                    QuoteState::Literal,
                );
                index += 1;
            }
        }
    }

    Ok(ExpandedWord {
        segments,
        had_quoted_content,
        had_quoted_null_outside_at,
        has_at_expansion,
    })
}

fn trim_trailing_newlines(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    while end > 0 && s[end - 1] == b'\n' {
        end -= 1;
    }
    &s[..end]
}

fn scan_backtick_command(
    source: &[u8],
    index: &mut usize,
    in_double_quotes: bool,
) -> Result<Vec<u8>, ExpandError> {
    let mut command = Vec::new();
    while *index < source.len() {
        let ch = source[*index];
        if ch == b'`' {
            *index += 1;
            return Ok(command);
        }
        if ch == b'\\' && *index + 1 < source.len() {
            let next = source[*index + 1];
            let special = if in_double_quotes {
                matches!(next, b'$' | b'`' | b'\\' | b'"' | b'\n')
            } else {
                matches!(next, b'$' | b'`' | b'\\')
            };
            if special {
                command.push(next);
                *index += 2;
                continue;
            }
        }
        command.push(ch);
        *index += 1;
    }
    Err(ExpandError {
        message: b"unterminated backquote".as_ref().into(),
    })
}

pub fn expand_here_document<'a, C: Context>(
    ctx: &mut C,
    text: &[u8],
    body_line: usize,
    arena: &'a ByteArena,
) -> Result<&'a [u8], ExpandError> {
    ctx.set_lineno(body_line);
    let mut result = Vec::new();
    let mut index = 0usize;

    while index < text.len() {
        match text[index] {
            b'\\' => {
                index += 1;
                if index >= text.len() {
                    result.push(b'\\');
                    break;
                }
                match text[index] {
                    b'$' | b'\\' => {
                        result.push(text[index]);
                        index += 1;
                    }
                    b'\n' => {
                        ctx.inc_lineno();
                        index += 1;
                    }
                    _ => {
                        result.push(b'\\');
                        result.push(text[index]);
                        index += 1;
                    }
                }
            }
            b'\n' => {
                ctx.inc_lineno();
                result.push(b'\n');
                index += 1;
            }
            b'$' => {
                let (expansion, consumed) = expand_dollar(ctx, &text[index..], false)?;
                result.extend_from_slice(&flatten_expansion(expansion));
                index += consumed;
            }
            b'`' => {
                index += 1;
                let command = scan_backtick_command(text, &mut index, true)?;
                let output = ctx.command_substitute(&command)?;
                result.extend_from_slice(trim_trailing_newlines(&output));
            }
            _ => {
                result.push(text[index]);
                index += 1;
            }
        }
    }

    Ok(arena.intern_vec(result))
}

fn expand_dollar<C: Context>(
    ctx: &mut C,
    source: &[u8],
    quoted: bool,
) -> Result<(Expansion, usize), ExpandError> {
    if source.len() < 2 {
        return Ok((Expansion::One(b"$".to_vec()), 1));
    }

    let c1 = source[1];
    match c1 {
        b'\'' if !quoted => {
            let (s, n) = parse_dollar_single_quoted(source)?;
            Ok((Expansion::One(s), n))
        }
        b'{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let expansion = expand_braced_parameter(ctx, expr, quoted)?;
            Ok((expansion, end + 1))
        }
        b'(' => {
            if source.get(2) == Some(&b'(') {
                let mut index = 3usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source[index];
                    if ch == b'(' {
                        depth += 1;
                    } else if ch == b')' {
                        if depth == 1 && source.get(index + 1) == Some(&b')') {
                            let expression = source[3..index].to_vec();
                            let saved_line = ctx.lineno();
                            let pre_expanded = expand_arithmetic_expression(ctx, &expression)?;
                            ctx.set_lineno(saved_line);
                            let value = eval_arithmetic(ctx, &pre_expanded)?;
                            return Ok((Expansion::One(bstr::i64_to_bytes(value)), index + 2));
                        }
                        depth = depth.saturating_sub(1);
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: b"unterminated arithmetic expansion".as_ref().into(),
                })
            } else {
                let mut index = 2usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source[index];
                    if ch == b'(' {
                        depth += 1;
                    } else if ch == b')' {
                        depth -= 1;
                        if depth == 0 {
                            let command = source[2..index].to_vec();
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = trim_trailing_newlines(&output).to_vec();
                            return Ok((Expansion::One(trimmed), index + 1));
                        }
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: b"unterminated command substitution".as_ref().into(),
                })
            }
        }
        b'@' => {
            if quoted {
                let params = ctx.positional_params().to_vec();
                Ok((Expansion::AtFields(params), 2))
            } else {
                let joined = Cow::Owned(bstr::join_bstrings(ctx.positional_params(), b" "));
                let value = require_set_parameter(ctx, b"@", Some(joined))?;
                Ok((Expansion::One(value), 2))
            }
        }
        b'*' => {
            let ifs = ctx.env_var(b"IFS");
            let sep = match ifs.as_deref() {
                None => b" ".to_vec(),
                Some(b"") => Vec::new(),
                Some(s) => vec![s[0]],
            };
            let value = bstr::join_bstrings(ctx.positional_params(), &sep);
            Ok((Expansion::One(value), 2))
        }
        b'?' | b'$' | b'!' | b'#' | b'-' | b'0' => {
            let ch_name = &source[1..2];
            let value = if c1 == b'0' {
                require_set_parameter(ctx, b"0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_name, ctx.special_param(c1))?
            };
            Ok((Expansion::One(value), 2))
        }
        next if next.is_ascii_digit() => Ok((
            Expansion::One(require_set_parameter(
                ctx,
                &source[1..2],
                ctx.positional_param((next - b'0') as usize),
            )?),
            2,
        )),
        next if next == b'_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source[index];
                if b == b'_' || b.is_ascii_alphanumeric() {
                    index += 1;
                } else {
                    break;
                }
            }
            let name = &source[1..index];
            Ok((
                Expansion::One(require_set_parameter(ctx, name, lookup_param(ctx, name))?),
                index,
            ))
        }
        _ => Ok((Expansion::One(b"$".to_vec()), 1)),
    }
}

fn expand_parameter_dollar<C: Context>(
    ctx: &mut C,
    source: &[u8],
) -> Result<(Vec<u8>, usize), ExpandError> {
    if source.len() < 2 {
        return Ok((b"$".to_vec(), 1));
    }

    let c1 = source[1];
    match c1 {
        b'\'' => parse_dollar_single_quoted(source),
        b'{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let value = expand_braced_parameter_text(ctx, expr)?;
            Ok((value, end + 1))
        }
        b'?' | b'$' | b'!' | b'#' | b'*' | b'@' | b'-' | b'0' => {
            let ch_name = &source[1..2];
            let value = if c1 == b'0' {
                require_set_parameter(ctx, b"0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_name, ctx.special_param(c1))?
            };
            Ok((value, 2))
        }
        next if next.is_ascii_digit() => {
            let value = ctx.positional_param((next - b'0') as usize);
            Ok((require_set_parameter(ctx, &source[1..2], value)?, 2))
        }
        next if next == b'_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source[index];
                if b == b'_' || b.is_ascii_alphanumeric() {
                    index += 1;
                } else {
                    break;
                }
            }
            let name = &source[1..index];
            Ok((
                require_set_parameter(ctx, name, lookup_param(ctx, name))?,
                index,
            ))
        }
        _ => Ok((b"$".to_vec(), 1)),
    }
}

fn parse_dollar_single_quoted(source: &[u8]) -> Result<(Vec<u8>, usize), ExpandError> {
    let mut index = 2usize;
    let mut result = Vec::new();
    while index < source.len() {
        match source[index] {
            b'\'' => return Ok((result, index + 1)),
            b'\\' => {
                index += 1;
                if index >= source.len() {
                    return Err(ExpandError {
                        message: b"unterminated dollar-single-quotes".as_ref().into(),
                    });
                }
                let ch = source[index];
                match ch {
                    b'"' => result.push(b'"'),
                    b'\'' => result.push(b'\''),
                    b'\\' => result.push(b'\\'),
                    b'a' => result.push(0x07),
                    b'b' => result.push(0x08),
                    b'e' => result.push(0x1b),
                    b'f' => result.push(0x0c),
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'v' => result.push(0x0b),
                    b'c' => {
                        index += 1;
                        if index >= source.len() {
                            return Err(ExpandError {
                                message: b"unterminated dollar-single-quotes".as_ref().into(),
                            });
                        }
                        if source[index] == b'\\' && index + 1 < source.len() {
                            index += 1;
                            result.push(control_escape(source[index]));
                        } else {
                            result.push(control_escape(source[index]));
                        }
                    }
                    b'x' => {
                        let (value, consumed) =
                            parse_variable_base_escape(&source[(index + 1)..], 16, 2);
                        if consumed == 0 {
                            result.push(b'x');
                        } else {
                            result.push(value);
                            index += consumed;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut digits = vec![ch];
                        let mut consumed = 0usize;
                        while consumed < 2
                            && index + 1 + consumed < source.len()
                            && matches!(source[index + 1 + consumed], b'0'..=b'7')
                        {
                            digits.push(source[index + 1 + consumed]);
                            consumed += 1;
                        }
                        let value = parse_octal_digits(&digits);
                        result.push(value);
                        index += consumed;
                    }
                    other => result.push(other),
                }
                index += 1;
            }
            _ => {
                result.push(source[index]);
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: b"unterminated dollar-single-quotes".as_ref().into(),
    })
}

fn parse_octal_digits(digits: &[u8]) -> u8 {
    let mut val: u8 = 0;
    for &d in digits {
        val = val.wrapping_mul(8).wrapping_add(d - b'0');
    }
    val
}

fn scan_to_closing_brace(source: &[u8], start: usize) -> Result<usize, ExpandError> {
    let mut index = start;
    while index < source.len() {
        match source[index] {
            b'}' => return Ok(index),
            b'\\' => {
                index += 2;
            }
            b'\'' => {
                index += 1;
                while index < source.len() && source[index] != b'\'' {
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'"' => {
                index += 1;
                while index < source.len() && source[index] != b'"' {
                    if source[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'$' if source.get(index + 1) == Some(&b'{') => {
                index += 2;
                let inner = scan_to_closing_brace(source, index)?;
                index = inner + 1;
            }
            b'$' if source.get(index + 1) == Some(&b'(') => {
                if source.get(index + 2) == Some(&b'(') {
                    index += 3;
                    let mut depth = 1usize;
                    while index < source.len() {
                        if source[index] == b'(' {
                            depth += 1;
                        } else if source[index] == b')' {
                            if depth == 1 && source.get(index + 1) == Some(&b')') {
                                index += 2;
                                break;
                            }
                            depth = depth.saturating_sub(1);
                        }
                        index += 1;
                    }
                } else {
                    index += 2;
                    let mut depth = 1usize;
                    while index < source.len() {
                        if source[index] == b'(' {
                            depth += 1;
                        } else if source[index] == b')' {
                            depth -= 1;
                            if depth == 0 {
                                index += 1;
                                break;
                            }
                        }
                        index += 1;
                    }
                }
            }
            b'`' => {
                index += 1;
                while index < source.len() && source[index] != b'`' {
                    if source[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: b"unterminated parameter expansion".as_ref().into(),
    })
}

fn control_escape(ch: u8) -> u8 {
    match ch {
        b'\\' => 0x1c,
        b'?' => 0x7f,
        other => other & 0x1f,
    }
}

fn parse_variable_base_escape(source: &[u8], base: u32, max_digits: usize) -> (u8, usize) {
    let mut consumed = 0usize;
    while consumed < max_digits
        && consumed < source.len()
        && is_digit_for_base(source[consumed], base)
    {
        consumed += 1;
    }
    if consumed == 0 {
        return (0, 0);
    }
    let mut val: u8 = 0;
    for &b in &source[..consumed] {
        let digit = if b >= b'a' {
            b - b'a' + 10
        } else if b >= b'A' {
            b - b'A' + 10
        } else {
            b - b'0'
        };
        val = val.wrapping_mul(base as u8).wrapping_add(digit);
    }
    (val, consumed)
}

fn is_digit_for_base(b: u8, base: u32) -> bool {
    let digit = if b.is_ascii_digit() {
        (b - b'0') as u32
    } else if b.is_ascii_lowercase() {
        (b - b'a') as u32 + 10
    } else if b.is_ascii_uppercase() {
        (b - b'A') as u32 + 10
    } else {
        return false;
    };
    digit < base
}

fn expand_braced_parameter<C: Context>(
    ctx: &mut C,
    expr: &[u8],
    quoted: bool,
) -> Result<Expansion, ExpandError> {
    if expr == b"#" {
        return Ok(Expansion::One(
            lookup_param(ctx, b"#")
                .map(|c| c.into_owned())
                .unwrap_or_default(),
        ));
    }
    if expr.first() == Some(&b'#') && expr.len() > 1 {
        let name = &expr[1..];
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(Expansion::One(bstr::u64_to_bytes(value.len() as u64)));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    let value_owned = || value.as_ref().map(|c| c.clone().into_owned()).unwrap_or_default();
    if op.is_none() {
        return Ok(Expansion::One(require_set_parameter(ctx, name, value)?));
    }
    let op_bytes = op.unwrap();
    let w = word.unwrap_or(b"");
    if op_bytes == b":-" {
        if !is_set || is_null {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"-" {
        if !is_set {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":=" {
        if !is_set || is_null {
            let val = assign_parameter(ctx, name, w, quoted)?;
            Ok(Expansion::One(val))
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"=" {
        if !is_set {
            let val = assign_parameter(ctx, name, w, quoted)?;
            Ok(Expansion::One(val))
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":?" {
        if !is_set || is_null {
            let default_msg = {
                let mut m = Vec::new();
                m.extend_from_slice(name);
                m.extend_from_slice(b": parameter null or not set");
                m
            };
            let raw = match word {
                Some(w2) if !w2.is_empty() => w2,
                _ => &default_msg,
            };
            let message = expand_parameter_word(ctx, raw, quoted)?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"?" {
        if !is_set {
            let default_msg = {
                let mut m = Vec::new();
                m.extend_from_slice(name);
                m.extend_from_slice(b": parameter not set");
                m
            };
            let raw = match word {
                Some(w2) if !w2.is_empty() => w2,
                _ => &default_msg,
            };
            let message = expand_parameter_word(ctx, raw, quoted)?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":+" {
        if is_set && !is_null {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(Vec::new()))
        }
    } else if op_bytes == b"+" {
        if is_set {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(Vec::new()))
        }
    } else if op_bytes == b"%" || op_bytes == b"%%" || op_bytes == b"#" || op_bytes == b"##" {
        let val = require_set_parameter(ctx, name, value)?;
        let pat = expand_parameter_pattern_word(ctx, w)?;
        let mode = if op_bytes == b"%" {
            PatternRemoval::SmallestSuffix
        } else if op_bytes == b"%%" {
            PatternRemoval::LargestSuffix
        } else if op_bytes == b"#" {
            PatternRemoval::SmallestPrefix
        } else {
            PatternRemoval::LargestPrefix
        };
        Ok(Expansion::One(remove_parameter_pattern(val, &pat, mode)?))
    } else {
        Err(ExpandError {
            message: b"unsupported parameter expansion".as_ref().into(),
        })
    }
}

fn expand_braced_parameter_text<C: Context>(
    ctx: &mut C,
    expr: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    if expr == b"#" {
        return Ok(lookup_param(ctx, b"#")
            .map(|c| c.into_owned())
            .unwrap_or_default());
    }
    if expr.first() == Some(&b'#') && expr.len() > 1 {
        let name = &expr[1..];
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(bstr::u64_to_bytes(value.len() as u64));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    let value_owned = || value.as_ref().map(|c| c.clone().into_owned()).unwrap_or_default();
    if op.is_none() {
        return require_set_parameter(ctx, name, value);
    }
    let op_bytes = op.unwrap();
    let w = word.unwrap_or(b"");
    if op_bytes == b":-" {
        if !is_set || is_null {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"-" {
        if !is_set {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b":=" {
        if !is_set || is_null {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"=" {
        if !is_set {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b":?" {
        if !is_set || is_null {
            let message =
                expand_parameter_error_text(ctx, name, word, b"parameter null or not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"?" {
        if !is_set {
            let message = expand_parameter_error_text(ctx, name, word, b"parameter not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b":+" {
        if is_set && !is_null {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(Vec::new())
        }
    } else if op_bytes == b"+" {
        if is_set {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(Vec::new())
        }
    } else if op_bytes == b"%" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestSuffix,
        )
    } else if op_bytes == b"%%" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestSuffix,
        )
    } else if op_bytes == b"#" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestPrefix,
        )
    } else if op_bytes == b"##" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestPrefix,
        )
    } else {
        Err(ExpandError {
            message: b"unsupported parameter expansion".as_ref().into(),
        })
    }
}

fn assign_parameter<C: Context>(
    ctx: &mut C,
    name: &[u8],
    raw_word: &[u8],
    quoted: bool,
) -> Result<Vec<u8>, ExpandError> {
    if !is_name(name) {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": cannot assign in parameter expansion");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    let value = expand_parameter_word(ctx, raw_word, quoted)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn assign_parameter_text<C: Context>(
    ctx: &mut C,
    name: &[u8],
    raw_word: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    if !is_name(name) {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": cannot assign in parameter expansion");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    let value = expand_parameter_text_owned(ctx, raw_word)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn expand_parameter_error_text<C: Context>(
    ctx: &mut C,
    name: &[u8],
    word: Option<&[u8]>,
    default_message: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let owned;
    let raw = match word {
        Some(w) if !w.is_empty() => w,
        _ => {
            let mut m = Vec::new();
            m.extend_from_slice(name);
            m.extend_from_slice(b": ");
            m.extend_from_slice(default_message);
            owned = m;
            &owned
        }
    };
    expand_parameter_text_owned(ctx, raw)
}

fn expand_parameter_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    _quoted: bool,
) -> Result<Vec<u8>, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_segments(&expanded.segments))
}

fn expand_parameter_word_as_expansion<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    _quoted: bool,
) -> Result<Expansion, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    let has_at = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak | Segment::AtEmpty));
    if has_at {
        let mut fields = Vec::new();
        let mut current = Vec::new();
        for seg in &expanded.segments {
            match seg {
                Segment::Text(s, _) => current.extend_from_slice(s),
                Segment::AtBreak => {
                    fields.push(std::mem::take(&mut current));
                }
                Segment::AtEmpty => {}
            }
        }
        fields.push(current);
        Ok(Expansion::AtFields(fields))
    } else {
        Ok(Expansion::One(flatten_segments(&expanded.segments)))
    }
}

fn expand_parameter_pattern_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
}

fn parse_parameter_expression(
    expr: &[u8],
) -> Result<(&[u8], Option<&[u8]>, Option<&[u8]>), ExpandError> {
    if expr.is_empty() {
        return Err(ExpandError {
            message: b"empty parameter expansion".as_ref().into(),
        });
    }
    let mut index = 0usize;
    let b0 = expr[0];
    let name: &[u8] = if b0.is_ascii_digit() {
        while index < expr.len() && expr[index].is_ascii_digit() {
            index += 1;
        }
        &expr[..index]
    } else if matches!(b0, b'?' | b'$' | b'!' | b'#' | b'*' | b'@') {
        index = 1;
        &expr[..index]
    } else if b0 == b'_' || b0.is_ascii_alphabetic() {
        while index < expr.len()
            && (expr[index] == b'_' || expr[index].is_ascii_alphanumeric())
        {
            index += 1;
        }
        &expr[..index]
    } else {
        return Err(ExpandError {
            message: b"invalid parameter expansion".as_ref().into(),
        });
    };

    if index == expr.len() {
        return Ok((name, None, None));
    }

    let rest = &expr[index..];
    let (op, word): (&[u8], &[u8]) = match rest[0] {
        b':' if rest.len() > 1 => match rest[1] {
            b'-' => (b":-", &rest[2..]),
            b'=' => (b":=", &rest[2..]),
            b'?' => (b":?", &rest[2..]),
            b'+' => (b":+", &rest[2..]),
            _ => (&rest[..1], &rest[1..]),
        },
        b'%' if rest.len() > 1 && rest[1] == b'%' => (b"%%", &rest[2..]),
        b'#' if rest.len() > 1 && rest[1] == b'#' => (b"##", &rest[2..]),
        b'-' => (b"-", &rest[1..]),
        b'=' => (b"=", &rest[1..]),
        b'?' => (b"?", &rest[1..]),
        b'+' => (b"+", &rest[1..]),
        b'%' => (b"%", &rest[1..]),
        b'#' => (b"#", &rest[1..]),
        _ => (&rest[..1], &rest[1..]),
    };
    Ok((name, Some(op), Some(word)))
}

fn lookup_param<'a, C: Context>(ctx: &'a C, name: &[u8]) -> Option<Cow<'a, [u8]>> {
    if name == b"0" {
        return Some(Cow::Borrowed(ctx.shell_name()));
    }
    if !name.is_empty() && name.iter().all(|b| b.is_ascii_digit()) {
        return bstr::parse_i64(name)
            .and_then(|n| {
                if n >= 0 {
                    Some(n as usize)
                } else {
                    None
                }
            })
            .and_then(|index| ctx.positional_param(index));
    }
    if name.len() == 1 {
        if let Some(value) = ctx.special_param(name[0]) {
            return Some(value);
        }
    }
    ctx.env_var(name)
}

fn require_set_parameter<C: Context>(
    ctx: &C,
    name: &[u8],
    value: Option<Cow<'_, [u8]>>,
) -> Result<Vec<u8>, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != b"@" && name != b"*" {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": parameter not set");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    Ok(value.map(|c| c.into_owned()).unwrap_or_default())
}

#[derive(Debug)]
struct ExpandedWord {
    segments: Vec<Segment>,
    had_quoted_content: bool,
    had_quoted_null_outside_at: bool,
    has_at_expansion: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Field {
    text: Vec<u8>,
    has_unquoted_glob: bool,
}

fn segment_bytes(segments: &[Segment]) -> impl Iterator<Item = (u8, QuoteState)> + '_ {
    segments
        .iter()
        .flat_map(|seg| match seg {
            Segment::Text(text, state) => {
                let s = *state;
                Some(text.iter().map(move |&b| (b, s)))
            }
            _ => None,
        })
        .flatten()
}

fn split_fields_from_segments(segments: &[Segment], ifs: &[u8]) -> Vec<Field> {
    if ifs.is_empty() {
        return vec![Field {
            text: flatten_segments(segments),
            has_unquoted_glob: segments.iter().any(|seg| {
                matches!(seg, Segment::Text(text, state) if *state != QuoteState::Quoted && text.iter().any(|&b| is_glob_byte(b)))
            }),
        }];
    }

    let ifs_ws: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|b| b.is_ascii_whitespace())
        .collect();
    let ifs_other: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let chars: Vec<(u8, QuoteState)> = segment_bytes(segments).collect();

    let mut fields = Vec::new();
    let mut current = Vec::new();
    let mut current_glob = false;
    let mut index = 0usize;

    while index < chars.len() {
        let (b, state) = chars[index];
        let splittable = state == QuoteState::Expanded;
        if splittable && ifs_other.contains(&b) {
            fields.push(Field {
                text: std::mem::take(&mut current),
                has_unquoted_glob: current_glob,
            });
            current_glob = false;
            index += 1;
            while index < chars.len()
                && chars[index].1 == QuoteState::Expanded
                && ifs_ws.contains(&chars[index].0)
            {
                index += 1;
            }
            continue;
        }
        if splittable && ifs_ws.contains(&b) {
            if !current.is_empty() {
                fields.push(Field {
                    text: std::mem::take(&mut current),
                    has_unquoted_glob: current_glob,
                });
                current_glob = false;
            }
            while index < chars.len()
                && chars[index].1 == QuoteState::Expanded
                && ifs_ws.contains(&chars[index].0)
            {
                index += 1;
            }
            continue;
        }
        current_glob |= state != QuoteState::Quoted && is_glob_byte(b);
        current.push(b);
        index += 1;
    }

    if !current.is_empty() {
        fields.push(Field {
            text: current,
            has_unquoted_glob: current_glob,
        });
    }

    fields
}

fn push_segment(segments: &mut Vec<Segment>, text: Vec<u8>, state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.extend_from_slice(&text);
            return;
        }
    }
    segments.push(Segment::Text(text, state));
}

fn push_segment_slice(segments: &mut Vec<Segment>, text: &[u8], state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.extend_from_slice(text);
            return;
        }
    }
    segments.push(Segment::Text(text.to_vec(), state));
}

fn flatten_segments(segments: &[Segment]) -> Vec<u8> {
    let mut result = Vec::new();
    for seg in segments {
        if let Segment::Text(part, _) = seg {
            result.extend_from_slice(part);
        }
    }
    result
}

fn render_pattern_from_segments(segments: &[Segment]) -> Vec<u8> {
    let mut pattern = Vec::new();
    for seg in segments {
        if let Segment::Text(text, state) = seg {
            if *state == QuoteState::Quoted {
                for &b in text.iter() {
                    pattern.push(b'\\');
                    pattern.push(b);
                }
            } else {
                pattern.extend_from_slice(text);
            }
        }
    }
    pattern
}

fn is_glob_byte(b: u8) -> bool {
    matches!(b, b'*' | b'?' | b'[')
}

#[derive(Clone, Copy)]
enum PatternRemoval {
    SmallestSuffix,
    LargestSuffix,
    SmallestPrefix,
    LargestPrefix,
}

fn remove_parameter_pattern(
    value: Vec<u8>,
    pattern: &[u8],
    mode: PatternRemoval,
) -> Result<Vec<u8>, ExpandError> {
    let boundaries: Vec<usize> = (0..=value.len()).collect();
    match mode {
        PatternRemoval::SmallestPrefix => {
            for &end in &boundaries {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_vec());
                }
            }
        }
        PatternRemoval::LargestPrefix => {
            for &end in boundaries.iter().rev() {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_vec());
                }
            }
        }
        PatternRemoval::SmallestSuffix => {
            for &start in boundaries.iter().rev() {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_vec());
                }
            }
        }
        PatternRemoval::LargestSuffix => {
            for &start in &boundaries {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_vec());
                }
            }
        }
    }
    Ok(value)
}

fn expand_pathname(pattern: &[u8]) -> Vec<Vec<u8>> {
    if !pattern.iter().any(|&b| is_glob_byte(b)) {
        return vec![pattern.to_vec()];
    }
    let absolute = pattern.first() == Some(&b'/');
    let segments: Vec<&[u8]> = pattern
        .split(|&b| b == b'/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let base: Vec<u8> = if absolute { b"/".to_vec() } else { b".".to_vec() };
    let mut matches = Vec::new();
    expand_path_segments(&base, &segments, 0, absolute, &mut matches);
    matches.sort();
    matches
}

fn expand_path_segments(
    base: &[u8],
    segments: &[&[u8]],
    index: usize,
    absolute: bool,
    matches: &mut Vec<Vec<u8>>,
) {
    if index == segments.len() {
        let text = if absolute {
            base.to_vec()
        } else {
            if base.starts_with(b"./") && base.len() > 2 {
                base[2..].to_vec()
            } else {
                base.to_vec()
            }
        };
        matches.push(if text.is_empty() {
            b".".to_vec()
        } else {
            text
        });
        return;
    }

    let segment = segments[index];

    if !segment.iter().any(|&b| is_glob_byte(b)) {
        let next = path_join(base, segment);
        if sys::file_exists(&next) {
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
        return;
    }

    let Ok(mut names) = sys::read_dir_entries(base) else {
        return;
    };
    names.sort();
    for name in names {
        if name.starts_with(b".") && !segment.starts_with(b".") {
            continue;
        }
        if pattern_matches(&name, segment) {
            let next = path_join(base, &name);
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
    }
}

fn path_join(base: &[u8], name: &[u8]) -> Vec<u8> {
    let mut result = base.to_vec();
    if !result.is_empty() && *result.last().unwrap() != b'/' {
        result.push(b'/');
    }
    result.extend_from_slice(name);
    result
}

pub fn pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
    pattern_matches_inner(text, 0, pattern, 0)
}

fn pattern_matches_inner(text: &[u8], ti: usize, pattern: &[u8], pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    let pc = pattern[pi];
    match pc {
        b'*' => {
            let mut pos = ti;
            loop {
                if pattern_matches_inner(text, pos, pattern, pi + 1) {
                    return true;
                }
                if pos == text.len() {
                    break;
                }
                pos += 1;
            }
            false
        }
        b'?' => {
            ti < text.len() && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
        }
        b'[' => {
            let tc = if ti < text.len() {
                Some(text[ti])
            } else {
                None
            };
            match match_bracket(tc, pattern, pi) {
                Some((matched, next_pi)) => {
                    matched
                        && ti < text.len()
                        && pattern_matches_inner(text, ti + 1, pattern, next_pi)
                }
                None => {
                    ti < text.len()
                        && text[ti] == b'['
                        && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
                }
            }
        }
        b'\\' if pi + 1 < pattern.len() => {
            let escaped = pattern[pi + 1];
            ti < text.len()
                && text[ti] == escaped
                && pattern_matches_inner(text, ti + 1, pattern, pi + 2)
        }
        ch => {
            ti < text.len()
                && text[ti] == ch
                && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
        }
    }
}

fn match_charclass(class: &[u8], ch: u8) -> bool {
    crate::sys::classify_byte(class, ch)
}

fn match_bracket(current: Option<u8>, pattern: &[u8], start: usize) -> Option<(bool, usize)> {
    let current = current?;
    let mut index = start + 1;
    if index >= pattern.len() {
        return None;
    }

    let mut negate = false;
    if index < pattern.len() && matches!(pattern[index], b'!' | b'^') {
        negate = true;
        index += 1;
    }

    let mut matched = false;
    let mut saw_closer = false;
    let mut first_elem = true;
    while index < pattern.len() {
        let pc = pattern[index];
        if pc == b']' && !first_elem {
            saw_closer = true;
            index += 1;
            break;
        }

        first_elem = false;

        if pc == b'[' && index + 1 < pattern.len() && pattern[index + 1] == b':' {
            let class_start = index + 2;
            let mut found_end = None;
            let mut ci = class_start;
            while ci + 1 < pattern.len() {
                if pattern[ci] == b':' && pattern[ci + 1] == b']' {
                    found_end = Some(ci);
                    break;
                }
                ci += 1;
            }
            if let Some(end) = found_end {
                let class_name = &pattern[class_start..end];
                matched |= match_charclass(class_name, current);
                index = end + 2;
                continue;
            }
        }

        let first = if pc == b'\\' && index + 1 < pattern.len() {
            index += 1;
            pattern[index]
        } else {
            pc
        };
        if index + 2 < pattern.len()
            && pattern[index + 1] == b'-'
            && pattern[index + 2] != b']'
        {
            let last = pattern[index + 2];
            matched |= first <= current && current <= last;
            index += 3;
        } else {
            matched |= current == first;
            index += 1;
        }
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn is_name(name: &[u8]) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }
    name[1..].iter().all(|&b| b == b'_' || b.is_ascii_alphanumeric())
}

fn expand_arithmetic_expression<C: Context>(
    ctx: &mut C,
    expression: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < expression.len() {
        if expression[i] == b'$' {
            let (expansion, consumed) = expand_dollar(ctx, &expression[i..], true)?;
            match expansion {
                Expansion::One(s) => result.extend_from_slice(&s),
                Expansion::AtFields(fields) => {
                    result.extend_from_slice(&bstr::join_bstrings(&fields, b" "));
                }
            }
            i += consumed;
        } else if expression[i] == b'`' {
            i += 1;
            let command = scan_backtick_command(expression, &mut i, true)?;
            let output = ctx.command_substitute(&command)?;
            result.extend_from_slice(trim_trailing_newlines(&output));
        } else if expression[i] == b'\n' {
            ctx.inc_lineno();
            result.push(b'\n');
            i += 1;
        } else {
            result.push(expression[i]);
            i += 1;
        }
    }
    Ok(result)
}

fn eval_arithmetic<C: Context>(ctx: &mut C, expression: &[u8]) -> Result<i64, ExpandError> {
    let mut parser = ArithmeticParser::new(ctx, expression);
    let value = parser.parse_assignment()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ExpandError {
            message: b"unexpected trailing arithmetic tokens".as_ref().into(),
        });
    }
    Ok(value)
}

struct ArithmeticParser<'a, 'src, C> {
    source: &'src [u8],
    index: usize,
    ctx: &'a mut C,
    start_line: usize,
    skip_depth: usize,
}

fn arith_err(msg: &[u8]) -> ExpandError {
    ExpandError {
        message: msg.into(),
    }
}

impl<'a, 'src, C: Context> ArithmeticParser<'a, 'src, C> {
    fn new(ctx: &'a mut C, raw: &'src [u8]) -> Self {
        let start_line = ctx.lineno();
        Self {
            source: raw,
            index: 0,
            ctx,
            start_line,
            skip_depth: 0,
        }
    }

    fn error_at_current(&mut self, msg: &[u8]) -> ExpandError {
        let newlines = self.source[..self.index.min(self.source.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        self.ctx.set_lineno(self.start_line + newlines);
        arith_err(msg)
    }

    fn parse_assignment(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let save = self.index;
        if let Some(name) = self.try_scan_name() {
            self.skip_ws();
            if let Some(op) = self.try_consume_assign_op() {
                let rhs = self.parse_assignment()?;
                if self.skip_depth > 0 {
                    return Ok(rhs);
                }
                let value = if op == b"=" {
                    rhs
                } else {
                    let lhs = self.resolve_var(&name)?;
                    apply_compound_assign(&op, lhs, rhs)?
                };
                self.ctx
                    .set_var(&name, bstr::i64_to_bytes(value))
                    .map_err(|e| ExpandError { message: e.message })?;
                return Ok(value);
            }
            self.index = save;
        }
        self.parse_ternary()
    }

    fn try_consume_assign_op(&mut self) -> Option<Vec<u8>> {
        let remaining = &self.source[self.index..];
        for op in &[
            b"<<=".as_ref(), b">>=", b"&=", b"^=", b"|=", b"*=", b"/=", b"%=", b"+=", b"-=", b"=",
        ] {
            if remaining.starts_with(op) {
                if *op == b"=" && remaining.starts_with(b"==") {
                    return None;
                }
                self.index += op.len();
                return Some(op.to_vec());
            }
        }
        None
    }

    fn parse_ternary(&mut self) -> Result<i64, ExpandError> {
        let cond = self.parse_logical_or()?;
        self.skip_ws();
        if self.consume(b'?') {
            if cond == 0 {
                self.skip_depth += 1;
            }
            let then_val = self.parse_assignment()?;
            if cond == 0 {
                self.skip_depth -= 1;
            }
            self.skip_ws();
            if !self.consume(b':') {
                return Err(self.error_at_current(b"expected ':' in ternary expression"));
            }
            if cond != 0 {
                self.skip_depth += 1;
            }
            let else_val = self.parse_assignment()?;
            if cond != 0 {
                self.skip_depth -= 1;
            }
            Ok(if cond != 0 { then_val } else { else_val })
        } else {
            Ok(cond)
        }
    }

    fn parse_logical_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_logical_and()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"||") {
                if value != 0 {
                    self.skip_depth += 1;
                    let _ = self.parse_logical_and()?;
                    self.skip_depth -= 1;
                    value = 1;
                } else {
                    let rhs = self.parse_logical_and()?;
                    value = i64::from(rhs != 0);
                }
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_logical_and(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_or()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"&&") {
                if value == 0 {
                    self.skip_depth += 1;
                    let _ = self.parse_bitwise_or()?;
                    self.skip_depth -= 1;
                } else {
                    let rhs = self.parse_bitwise_or()?;
                    value = i64::from(rhs != 0);
                }
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_xor()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'|')
                && self.peek_at(1) != Some(b'|')
                && self.peek_at(1) != Some(b'=')
            {
                self.index += 1;
                value |= self.parse_bitwise_xor()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_xor(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'^') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value ^= self.parse_bitwise_and()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_and(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_equality()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'&')
                && self.peek_at(1) != Some(b'&')
                && self.peek_at(1) != Some(b'=')
            {
                self.index += 1;
                value &= self.parse_equality()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_relational()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"==") {
                value = i64::from(value == self.parse_relational()?);
            } else if self.consume_bytes(b"!=") {
                value = i64::from(value != self.parse_relational()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_relational(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_shift()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"<=") {
                value = i64::from(value <= self.parse_shift()?);
            } else if self.consume_bytes(b">=") {
                value = i64::from(value >= self.parse_shift()?);
            } else if self.peek() == Some(b'<') && self.peek_at(1) != Some(b'<') {
                self.index += 1;
                value = i64::from(value < self.parse_shift()?);
            } else if self.peek() == Some(b'>') && self.peek_at(1) != Some(b'>') {
                self.index += 1;
                value = i64::from(value > self.parse_shift()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_shift(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_additive()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'<')
                && self.peek_at(1) == Some(b'<')
                && self.peek_at(2) != Some(b'=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shl(rhs as u32);
            } else if self.peek() == Some(b'>')
                && self.peek_at(1) == Some(b'>')
                && self.peek_at(2) != Some(b'=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shr(rhs as u32);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_additive(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_multiplicative()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'+') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.peek() == Some(b'-') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_sub(self.parse_multiplicative()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'*') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.peek() == Some(b'/') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error_at_current(b"division by zero"));
                }
                value /= rhs;
            } else if self.peek() == Some(b'%') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error_at_current(b"division by zero"));
                }
                value %= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume(b'+') {
            return self.parse_unary();
        }
        if self.consume(b'-') {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.consume(b'~') {
            return Ok(!self.parse_unary()?);
        }
        if self.peek() == Some(b'!') && self.peek_at(1) != Some(b'=') {
            self.index += 1;
            return Ok(i64::from(self.parse_unary()? == 0));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume(b'(') {
            let value = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(b')') {
                return Err(self.error_at_current(b"missing ')'"));
            }
            return Ok(value);
        }

        if let Some(name) = self.try_scan_name() {
            return self.resolve_var(&name);
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let start = self.index;
        if self.peek() == Some(b'0') {
            self.index += 1;
            if self.peek() == Some(b'x') || self.peek() == Some(b'X') {
                self.index += 1;
                let hex_start = self.index;
                while self.index < self.source.len()
                    && self.source[self.index].is_ascii_hexdigit()
                {
                    self.index += 1;
                }
                if self.index == hex_start {
                    return Err(self.error_at_current(b"invalid hex constant"));
                }
                return bstr::parse_hex_i64(&self.source[hex_start..self.index])
                    .ok_or_else(|| self.error_at_current(b"invalid hex constant"));
            }
            if self.peek().map_or(false, |c| c.is_ascii_digit()) {
                while self.index < self.source.len()
                    && self.source[self.index].is_ascii_digit()
                {
                    self.index += 1;
                }
                return bstr::parse_octal_i64(&self.source[start + 1..self.index])
                    .ok_or_else(|| self.error_at_current(b"invalid octal constant"));
            }
            return Ok(0);
        }

        while self.index < self.source.len() && self.source[self.index].is_ascii_digit()
        {
            self.index += 1;
        }
        if start == self.index {
            return Err(self.error_at_current(b"expected arithmetic operand"));
        }
        bstr::parse_i64(&self.source[start..self.index])
            .ok_or_else(|| self.error_at_current(b"invalid arithmetic operand"))
    }

    fn try_scan_name(&mut self) -> Option<Vec<u8>> {
        self.skip_ws();
        let start = self.index;
        if self.index < self.source.len() {
            let b = self.source[self.index];
            if b.is_ascii_alphabetic() || b == b'_' {
                self.index += 1;
                while self.index < self.source.len() {
                    let b2 = self.source[self.index];
                    if b2.is_ascii_alphanumeric() || b2 == b'_' {
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                return Some(self.source[start..self.index].to_vec());
            }
        }
        None
    }

    fn resolve_var(&mut self, name: &[u8]) -> Result<i64, ExpandError> {
        let val_opt = self.ctx.env_var(name);
        if val_opt.is_none() && self.ctx.nounset_enabled() {
            let mut msg = Vec::new();
            msg.extend_from_slice(name);
            msg.extend_from_slice(b": parameter not set");
            return Err(self.error_at_current(&msg));
        }
        let val_bytes = val_opt.unwrap_or_default();
        if val_bytes.is_empty() {
            return Ok(0);
        }
        let trimmed = trim_ascii_whitespace(&val_bytes).to_vec();
        let mut err_msg = Vec::new();
        err_msg.extend_from_slice(b"invalid variable value for '");
        err_msg.extend_from_slice(name);
        err_msg.push(b'\'');
        if trimmed.starts_with(b"0x") || trimmed.starts_with(b"0X") {
            bstr::parse_hex_i64(&trimmed[2..])
                .ok_or_else(|| self.error_at_current(&err_msg))
        } else if trimmed.starts_with(b"0")
            && trimmed.len() > 1
            && trimmed[1..].iter().all(|b| b.is_ascii_digit())
        {
            bstr::parse_octal_i64(&trimmed[1..])
                .ok_or_else(|| self.error_at_current(&err_msg))
        } else {
            bstr::parse_i64(&trimmed)
                .ok_or_else(|| self.error_at_current(&err_msg))
        }
    }

    fn skip_ws(&mut self) {
        while self.index < self.source.len()
            && self.source[self.index].is_ascii_whitespace()
        {
            self.index += 1;
        }
    }

    fn consume(&mut self, ch: u8) -> bool {
        if self.source.get(self.index) == Some(&ch) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_bytes(&mut self, s: &[u8]) -> bool {
        if self.source[self.index..].starts_with(s) {
            self.index += s.len();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.index + offset).copied()
    }

    fn is_eof(&self) -> bool {
        self.index >= self.source.len()
    }
}

fn trim_ascii_whitespace(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |p| p + 1);
    &s[start..end]
}

fn apply_compound_assign(op: &[u8], lhs: i64, rhs: i64) -> Result<i64, ExpandError> {
    match op {
        b"+=" => Ok(lhs.wrapping_add(rhs)),
        b"-=" => Ok(lhs.wrapping_sub(rhs)),
        b"*=" => Ok(lhs.wrapping_mul(rhs)),
        b"/=" => {
            if rhs == 0 {
                return Err(arith_err(b"division by zero"));
            }
            Ok(lhs / rhs)
        }
        b"%=" => {
            if rhs == 0 {
                return Err(arith_err(b"division by zero"));
            }
            Ok(lhs % rhs)
        }
        b"<<=" => Ok(lhs.wrapping_shl(rhs as u32)),
        b">>=" => Ok(lhs.wrapping_shr(rhs as u32)),
        b"&=" => Ok(lhs & rhs),
        b"^=" => Ok(lhs ^ rhs),
        b"|=" => Ok(lhs | rhs),
        _ => {
            let mut msg = b"unknown assignment operator '".to_vec();
            msg.extend_from_slice(op);
            msg.push(b'\'');
            Err(arith_err(&msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use std::collections::HashMap;

    struct FakeContext {
        env: HashMap<Vec<u8>, Vec<u8>>,
        positional: Vec<Vec<u8>>,
        pathname_expansion_enabled: bool,
        nounset_enabled: bool,
    }

    impl FakeContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
            env.insert(b"USER".to_vec(), b"meiksh".to_vec());
            env.insert(b"IFS".to_vec(), b" \t\n,".to_vec());
            env.insert(b"WORDS".to_vec(), b"one,two three".to_vec());
            env.insert(b"DELIMS".to_vec(), b",,,".to_vec());
            env.insert(b"EMPTY".to_vec(), Vec::new());
            env.insert(b"X".to_vec(), b"fallback".to_vec());
            Self {
                env,
                positional: vec![b"alpha".to_vec(), b"beta".to_vec()],
                pathname_expansion_enabled: true,
                nounset_enabled: false,
            }
        }
    }

    impl Context for FakeContext {
        fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
            self.env.get(name).map(|v| Cow::Borrowed(v.as_slice()))
        }

        fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>> {
            match name {
                b'?' => Some(Cow::Owned(b"0".to_vec())),
                b'#' => Some(Cow::Owned(bstr::u64_to_bytes(self.positional.len() as u64))),
                b'-' => Some(Cow::Owned(b"aC".to_vec())),
                b'*' | b'@' => Some(Cow::Owned(bstr::join_bstrings(&self.positional, b" "))),
                _ => None,
            }
        }

        fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
            if index == 0 {
                Some(Cow::Owned(b"meiksh".to_vec()))
            } else {
                self.positional
                    .get(index - 1)
                    .map(|v| Cow::Borrowed(v.as_slice()))
            }
        }

        fn positional_params(&self) -> &[Vec<u8>] {
            &self.positional
        }

        fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), ExpandError> {
            self.env.insert(name.to_vec(), value);
            Ok(())
        }

        fn pathname_expansion_enabled(&self) -> bool {
            self.pathname_expansion_enabled
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &[u8] {
            b"meiksh"
        }

        fn command_substitute(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
            let mut out = command.to_vec();
            out.push(b'\n');
            Ok(out)
        }

        fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
            if name == b"testuser" {
                Some(Cow::Owned(b"/home/testuser".to_vec()))
            } else {
                None
            }
        }
    }

    #[test]
    fn expands_home_and_params() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~/$USER".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("expand");
        assert_eq!(fields, vec![b"/tmp/home/meiksh".as_ref()]);
    }

    #[test]
    fn expands_arithmetic_expressions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 + 2 * 3))".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"7".as_ref()]
        );
    }

    #[test]
    fn expands_command_substitution() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_words(
                &mut ctx,
                &[
                    Word {
                        raw: b"$WORDS".as_ref().into(),
                        line: 0
                    },
                    Word {
                        raw: b"$(printf hi)".as_ref().into(),
                        line: 0
                    },
                ],
                &arena,
            )
            .expect("expand"),
            vec![
                b"one".as_ref(),
                b"two".as_ref(),
                b"three".as_ref(),
                b"printf".as_ref(),
                b"hi".as_ref(),
            ]
        );
    }

    #[test]
    fn preserves_quoted_and_escaped_characters() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$0 $1\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"meiksh alpha".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\\$HOME".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"$HOME".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"a\\ b".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"a b".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"'literal text'".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"literal text".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"cost:\\$USER\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"cost:$USER".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$'a b'".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"a b".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$'line\\nnext'".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"line\nnext".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$'a b'\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"$'a b'".as_ref()]
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"$'tab\\tstop'", &arena).expect("parameter text"),
            b"tab\tstop".as_ref()
        );
    }

    #[test]
    fn rejects_unterminated_quotes_and_expansions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        for raw in [
            b"'oops".as_ref(),
            b"\"oops",
            b"${USER",
            b"$(echo",
            b"$((1 + 2)",
            b"$'oops",
        ] {
            let error = expand_word(
                &mut ctx,
                &Word {
                    raw: raw.into(),
                    line: 0,
                },
                &arena,
            )
            .expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn dollar_single_quote_helpers_cover_escape_matrix() {
        let input = b"$'\\\"\\'\\\\\\a\\b\\e\\f\\n\\r\\t\\v\\cA\\c\\\\\\x41\\101Z'";
        let (value, consumed) = parse_dollar_single_quoted(input).expect("parse");
        assert_eq!(consumed, input.len());
        let mut expected = Vec::new();
        expected.push(b'"');
        expected.push(b'\'');
        expected.push(b'\\');
        expected.push(0x07); // \a
        expected.push(0x08); // \b
        expected.push(0x1b); // \e
        expected.push(0x0c); // \f
        expected.push(b'\n');
        expected.push(b'\r');
        expected.push(b'\t');
        expected.push(0x0b); // \v
        expected.push(0x01); // \cA
        expected.push(0x1c); // \c\\
        expected.push(b'A');  // \x41
        expected.push(b'A');  // \101
        expected.push(b'Z');
        assert_eq!(value, expected);

        assert!(parse_dollar_single_quoted(b"$'\\").is_err());

        assert!(parse_dollar_single_quoted(b"$'\\c").is_err());

        let (value, _) = parse_dollar_single_quoted(b"$'\\xZ'").expect("parse no hex");
        assert_eq!(value, b"xZ");

        let (value, _) = parse_dollar_single_quoted(b"$'\\x41'").expect("parse hex");
        assert_eq!(value, b"A");

        let (value, _) = parse_dollar_single_quoted(b"$'\\z'").expect("parse unspecified");
        assert_eq!(value, b"z");

        assert_eq!(control_escape(b'\\'), 0x1c);
        assert_eq!(control_escape(b'?'), 0x7f);
        assert_eq!(control_escape(b'A'), 0x01);
        assert_eq!(parse_variable_base_escape(b"412", 16, 2), (0x41, 2));
        assert_eq!(parse_variable_base_escape(b"1017", 8, 3), (0o101, 3));
        assert_eq!(parse_variable_base_escape(b"Z", 16, 2), (0, 0));
    }

    #[test]
    fn rejects_bad_arithmetic() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        for raw in [
            b"$((1 / 0))".as_ref(),
            b"$((1 + ))",
            b"$((1 1))",
        ] {
            let error = expand_word(
                &mut ctx,
                &Word {
                    raw: raw.into(),
                    line: 0,
                },
                &arena,
            )
            .expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn supports_parameter_operators_and_positionals() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![
            b"a".to_vec(),
            b"b".to_vec(),
            b"c".to_vec(),
            b"d".to_vec(),
            b"e".to_vec(),
            b"f".to_vec(),
            b"g".to_vec(),
            b"h".to_vec(),
            b"i".to_vec(),
            b"j".to_vec(),
        ];

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${10}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"j".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$10".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![b"a0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${#10}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"1".as_ref()]
        );
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$*".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![
                b"a".as_ref(),
                b"b".as_ref(),
                b"c".as_ref(),
                b"d".as_ref(),
                b"e".as_ref(),
                b"f".as_ref(),
                b"g".as_ref(),
                b"h".as_ref(),
                b"i".as_ref(),
                b"j".as_ref()
            ]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"a:b:c:d:e:f:g:h:i:j".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET:-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${EMPTY-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${EMPTY:-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${USER:+alt}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"alt".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET+alt}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${NEW:=value}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"value".as_ref()]
        );
        assert_eq!(ctx.env.get(b"NEW".as_ref()).map(|v| v.as_slice()), Some(b"value".as_ref()));
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${#}".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![b"10".as_ref()]
        );

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?boom}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("unset error");
        assert_eq!(&*error.message, b"boom".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?$'unterminated}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon-question word expansion error");
        assert!(!error.message.is_empty());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${MISSING?$'unterminated}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("plain-question word expansion error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn performs_field_splitting_more_like_posix() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$WORDS".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"one".as_ref(), b"two".as_ref(), b"three".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$DELIMS".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"".as_ref() as &[u8], b"", b""]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$EMPTY".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert!(split_fields_from_segments(&[], b" \t\n").is_empty());
    }

    #[test]
    fn expands_text_without_field_splitting_or_pathname_expansion() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"WORDS".to_vec(), b"one two".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"$WORDS".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            b"one two"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"*".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            b"*"
        );
    }

    #[test]
    fn performs_pathname_expansion() {
        let arena = ByteArena::new();
        let dir_entries = || {
            vec![
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntryBytes(b"a.txt".to_vec()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntryBytes(b"b.txt".to_vec()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntryBytes(b".hidden.txt".to_vec()),
                ),
                t("readdir", vec![ArgMatcher::Any], TraceResult::Int(0)),
            ]
        };
        let mut trace = vec![
            t(
                "access",
                vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "opendir",
                vec![ArgMatcher::Str("/testdir".into())],
                TraceResult::Int(1),
            ),
        ];
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        trace.push(t(
            "access",
            vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        trace.push(t(
            "opendir",
            vec![ArgMatcher::Str("/testdir".into())],
            TraceResult::Int(1),
        ));
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        run_trace(trace, || {
            let mut ctx = FakeContext::new();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: b"/testdir/*.txt".as_ref().into(),
                        line: 0
                    },
                    &arena,
                )
                .expect("glob"),
                vec![b"/testdir/a.txt".as_ref(), b"/testdir/b.txt".as_ref()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: b"\\*.txt".as_ref().into(),
                        line: 0
                    },
                    &arena,
                )
                .expect("escaped glob"),
                vec![b"*.txt".as_ref()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: b"/testdir/.*.txt".as_ref().into(),
                        line: 0
                    },
                    &arena,
                )
                .expect("hidden glob"),
                vec![b"/testdir/.hidden.txt".as_ref()]
            );
        });
    }

    #[test]
    fn can_disable_pathname_expansion_via_context() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let pattern = b"/testdir/*.txt";
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: pattern.as_ref().into(),
                        line: 0
                    },
                    &arena,
                )
                .expect("noglob"),
                vec![pattern.as_ref()]
            );
        });
    }

    #[test]
    fn helper_paths_cover_remaining_branches() {
        let ctx = FakeContext::new();
        assert_eq!(lookup_param(&ctx, b"?").as_deref(), Some(b"0".as_ref()));
        assert_eq!(lookup_param(&ctx, b"0").as_deref(), Some(b"meiksh".as_ref()));
        assert_eq!(lookup_param(&ctx, b"X").as_deref(), Some(b"fallback".as_ref()));
        assert_eq!(lookup_param(&ctx, b"99"), None);
        assert_eq!(
            ctx.positional_params(),
            &[b"alpha".to_vec(), b"beta".to_vec()][..]
        );
        assert_eq!(ctx.positional_param(0).as_deref(), Some(b"meiksh".as_ref()));

        let mut segs = Vec::new();
        push_segment(&mut segs, b"a".to_vec(), QuoteState::Expanded);
        push_segment(&mut segs, Vec::new(), QuoteState::Expanded);
        push_segment(&mut segs, b"b".to_vec(), QuoteState::Expanded);
        push_segment(&mut segs, b"c".to_vec(), QuoteState::Quoted);
        assert_eq!(
            segs,
            vec![
                Segment::Text(b"ab".to_vec(), QuoteState::Expanded),
                Segment::Text(b"c".to_vec(), QuoteState::Quoted)
            ]
        );

        assert_eq!(flatten_segments(&segs), b"abc".to_vec());
        assert!(pattern_matches(b"beta", b"b*"));
        assert!(!pattern_matches(b"beta", b"a*"));
        let mut ctx2 = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx2, b"42").expect("direct eval"), 42);
        assert!(eval_arithmetic(&mut ctx2, b"(1 + 2").is_err());

        let arith_bt =
            expand_arithmetic_expression(&mut ctx2, b"`printf 5`").expect("backtick in arith");
        assert_eq!(arith_bt, b"printf 5");

        let mut parser = ArithmeticParser::new(&mut ctx2, b"9");
        parser.index = 99;
        assert!(parser.is_eof());
    }

    #[test]
    fn nounset_option_rejects_plain_unset_parameter_expansions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"$UNSET".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset variable");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset braced");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"$9".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset positional");
        assert_eq!(&*error.message, b"9: parameter not set".as_ref());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("default still works"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("star exempt"),
            vec![b"alpha beta".as_ref()]
        );
    }

    struct DefaultPathContext {
        env: HashMap<Vec<u8>, Vec<u8>>,
        nounset_enabled: bool,
    }

    impl DefaultPathContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
            Self {
                env,
                nounset_enabled: false,
            }
        }
    }

    impl Context for DefaultPathContext {
        fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
            self.env.get(name).map(|v| Cow::Borrowed(v.as_slice()))
        }

        fn special_param(&self, _name: u8) -> Option<Cow<'_, [u8]>> {
            None
        }

        fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
            if index == 0 {
                Some(Cow::Owned(b"meiksh".to_vec()))
            } else {
                None
            }
        }

        fn positional_params(&self) -> &[Vec<u8>] {
            &[]
        }

        fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), ExpandError> {
            self.env.insert(name.to_vec(), value);
            Ok(())
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &[u8] {
            b"meiksh"
        }

        fn command_substitute(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
            let mut out = command.to_vec();
            out.push(b'\n');
            Ok(out)
        }

        fn home_dir_for_user(&self, _name: &[u8]) -> Option<Cow<'_, [u8]>> {
            None
        }
    }

    fn expect_one(result: Result<(Expansion, usize), ExpandError>) -> (Vec<u8>, usize) {
        let (expansion, consumed) = result.expect("expansion");
        let Expansion::One(s) = expansion else {
            panic!("expected One, got AtFields")
        };
        (s, consumed)
    }

    #[test]
    fn direct_expand_dollar_covers_fallbacks_and_nesting() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$", false)),
            (b"$".to_vec(), 1)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$-", false)),
            (b"aC".to_vec(), 2)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$$", false)),
            (b"".to_vec(), 2)
        );

        let (at_expansion, at_consumed) = expand_dollar(&mut ctx, b"$@", true).expect("quoted at");
        assert_eq!(at_consumed, 2);
        let Expansion::AtFields(fields) = at_expansion else {
            panic!("expected AtFields for quoted $@")
        };
        assert_eq!(fields, vec![b"alpha".to_vec(), b"beta".to_vec()]);

        let arithmetic_input = b"$((1 + (2 * 3)))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, arithmetic_input, false)),
            (b"7".to_vec(), arithmetic_input.len())
        );

        let command_input = b"$(printf (hi))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, command_input, false)),
            (b"printf (hi)".to_vec(), command_input.len())
        );
    }

    #[test]
    fn parameter_text_expansion_avoids_command_substitution() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());

        assert_eq!(
            expand_parameter_text(&mut ctx, b"${HOME:-/fallback}/.shrc", &arena)
                .expect("parameter text"),
            b"/tmp/home/.shrc"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"${EMPTY:-$HOME}/nested", &arena)
                .expect("nested default"),
            b"/tmp/home/nested"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"$(printf nope)${HOME}", &arena)
                .expect("literal command"),
            b"$(printf nope)/tmp/home"
        );
    }

    #[test]
    fn parameter_text_dollar_helpers_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$").expect("single"),
            (b"$".to_vec(), 1)
        );
        assert!(expand_parameter_dollar(&mut ctx, b"${HOME").is_err());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$0").expect("zero"),
            (b"meiksh".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$?").expect("special"),
            (b"0".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$1").expect("positional"),
            (b"alpha".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$HOME").expect("name"),
            (b"/tmp/home".to_vec(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$HOME+rest").expect("name stops at +"),
            (b"/tmp/home".to_vec(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$-").expect("dash"),
            (b"aC".to_vec(), 2)
        );
    }

    #[test]
    fn parameter_text_assignment_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"#").expect("hash"),
            b"2"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"#HOME").expect("length"),
            b"9"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME-word").expect("dash set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"UNSET-word").expect("dash unset"),
            b"word"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:=value").expect("colon assign set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"UNSET:=value").expect("assign unset"),
            b"value"
        );
        assert_eq!(ctx.env.get(b"UNSET".as_ref()).map(|v| v.as_slice()), Some(b"value".as_ref()));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING3=value").expect("assign equals unset"),
            b"value"
        );
        assert_eq!(ctx.env.get(b"MISSING3".as_ref()).map(|v| v.as_slice()), Some(b"value".as_ref()));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME=value").expect("assign set"),
            b"/tmp/home"
        );
        assert!(assign_parameter_text(&mut ctx, b"1", b"value").is_err());

        let err = expand_braced_parameter_text(&mut ctx, b"MISSING4?").expect_err("? no word");
        assert_eq!(&*err.message, b"MISSING4: parameter not set".as_ref());
        let text =
            expand_parameter_error_text(&mut ctx, b"X", Some(b""), b"my default").expect("empty word");
        assert_eq!(text, b"X: my default");
    }

    #[test]
    fn nounset_option_rejects_length_and_pattern_expansions_of_unset_parameters() {
        let mut ctx = DefaultPathContext::new();
        ctx.nounset_enabled = true;

        let error = expand_braced_parameter_text(&mut ctx, b"#UNSET").expect_err("nounset length");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error =
            expand_braced_parameter_text(&mut ctx, b"UNSET%.*").expect_err("nounset pattern");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());
    }

    #[test]
    fn parameter_text_question_operator_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:?boom").expect("colon question set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME?boom").expect("question set"),
            b"/tmp/home"
        );
        let colon_question = expand_braced_parameter_text(&mut ctx, b"EMPTY:?boom")
            .expect_err("colon question unset");
        assert_eq!(&*colon_question.message, b"boom".as_ref());
        let question =
            expand_braced_parameter_text(&mut ctx, b"MISSING?boom").expect_err("question unset");
        assert_eq!(&*question.message, b"boom".as_ref());
        let colon_default =
            expand_braced_parameter_text(&mut ctx, b"EMPTY:?").expect_err("colon default");
        assert_eq!(&*colon_default.message, b"EMPTY: parameter null or not set".as_ref());
        let question_default =
            expand_braced_parameter_text(&mut ctx, b"MISSING?").expect_err("question default");
        assert_eq!(&*question_default.message, b"MISSING: parameter not set".as_ref());
    }

    #[test]
    fn parameter_text_question_propagates_word_expansion_error() {
        let mut ctx = FakeContext::new();
        let err = expand_braced_parameter_text(&mut ctx, b"MISSING:?$'unterminated")
            .expect_err("colon-question text expansion error");
        assert!(!err.message.is_empty());
        let err = expand_braced_parameter_text(&mut ctx, b"MISSING?$'unterminated")
            .expect_err("plain-question text expansion error");
        assert!(!err.message.is_empty());
    }

    #[test]
    fn parameter_text_plus_and_pattern_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"DOTTED".to_vec(), b"alpha.beta".to_vec());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:+alt").expect("colon plus"),
            b"alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING2:+alt").expect("colon plus unset"),
            b""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME+alt").expect("plus set"),
            b"alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING2+alt").expect("plus unset"),
            b""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED%.*").expect("suffix"),
            b"alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED%%.*").expect("largest suffix"),
            b"alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED#*.").expect("prefix"),
            b"beta"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED##*.").expect("largest prefix"),
            b"beta"
        );
        assert!(expand_braced_parameter_text(&mut ctx, b"HOME::word").is_err());
    }

    #[test]
    fn parameter_helpers_cover_more_edge_cases() {
        let mut ctx = FakeContext::new();

        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:-word", false).expect("default set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:=word", false).expect("assign set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"MISSING=value", false).expect("assign unset"),
            Expansion::One(b"value".to_vec())
        );
        assert_eq!(ctx.env.get(b"MISSING".as_ref()).map(|v| v.as_slice()), Some(b"value".as_ref()));
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER=value", false).expect("assign set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER?boom", false).expect("question set"),
            Expansion::One(b"meiksh".to_vec())
        );
        let error =
            expand_braced_parameter(&mut ctx, b"UNSET?boom", false).expect_err("question unset");
        assert_eq!(&*error.message, b"boom".as_ref());
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:?boom", false).expect("colon question set"),
            Expansion::One(b"meiksh".to_vec())
        );

        let error = assign_parameter(&mut ctx, b"1", b"value", false).expect_err("invalid assign");
        assert_eq!(&*error.message, b"1: cannot assign in parameter expansion".as_ref());

        let parsed = parse_parameter_expression(b"@").expect("special name");
        assert_eq!(parsed, (b"@".as_ref(), None, None));

        let error = parse_parameter_expression(b"").expect_err("empty expr");
        assert_eq!(&*error.message, b"empty parameter expansion".as_ref());

        let error = parse_parameter_expression(b"%oops").expect_err("invalid expr");
        assert_eq!(&*error.message, b"invalid parameter expansion".as_ref());
        let parsed = parse_parameter_expression(b"USER%%tail").expect("largest suffix");
        assert_eq!(parsed, (b"USER".as_ref(), Some(b"%%".as_ref()), Some(b"tail".as_ref())));
        let parsed = parse_parameter_expression(b"USER/tail").expect("unknown operator");
        assert_eq!(parsed, (b"USER".as_ref(), Some(b"/".as_ref()), Some(b"tail".as_ref())));

        let error =
            expand_braced_parameter(&mut ctx, b"USER/tail", false).expect_err("unsupported expr");
        assert_eq!(&*error.message, b"unsupported parameter expansion".as_ref());
    }

    #[test]
    fn field_and_pattern_helpers_cover_corner_cases() {
        run_trace(
            vec![t(
                "opendir",
                vec![ArgMatcher::Any],
                TraceResult::Err(crate::sys::ENOENT),
            )],
            || {
                let segs = vec![Segment::Text(b"*.txt".to_vec(), QuoteState::Expanded)];
                assert_eq!(
                    split_fields_from_segments(&segs, b""),
                    vec![Field {
                        text: b"*.txt".to_vec(),
                        has_unquoted_glob: true,
                    }]
                );

                assert_eq!(
                    split_fields_from_segments(
                        &[Segment::Text(
                            b"alpha,  beta".to_vec(),
                            QuoteState::Expanded
                        )],
                        b" ,"
                    ),
                    vec![
                        Field {
                            text: b"alpha".to_vec(),
                            has_unquoted_glob: false,
                        },
                        Field {
                            text: b"beta".to_vec(),
                            has_unquoted_glob: false,
                        },
                    ]
                );

                assert_eq!(expand_pathname(b"plain.txt"), vec![b"plain.txt".to_vec()]);

                let mut matches = Vec::new();
                expand_path_segments(
                    b"/definitely/not/a/real/dir",
                    &[b"*.txt".as_ref()],
                    0,
                    false,
                    &mut matches,
                );
                assert!(matches.is_empty());

                let mut matches = Vec::new();
                expand_path_segments(b".", &[], 0, false, &mut matches);
                assert_eq!(matches, vec![b".".to_vec()]);

                assert!(pattern_matches(b"x", b"?"));
                assert!(pattern_matches(b"[", b"["));
                assert!(pattern_matches(b"]", b"\\]"));
                assert!(pattern_matches(b"b", b"[a-c]"));
                assert!(pattern_matches(b"d", b"[!a-c]"));

                assert!(pattern_matches(b"a", b"[[:alpha:]]"));
                assert!(pattern_matches(b"Z", b"[[:alpha:]]"));
                assert!(!pattern_matches(b"5", b"[[:alpha:]]"));
                assert!(pattern_matches(b"3", b"[[:alnum:]]"));
                assert!(pattern_matches(b"z", b"[[:alnum:]]"));
                assert!(!pattern_matches(b"!", b"[[:alnum:]]"));
                assert!(pattern_matches(b" ", b"[[:blank:]]"));
                assert!(pattern_matches(b"\t", b"[[:blank:]]"));
                assert!(!pattern_matches(b"a", b"[[:blank:]]"));
                assert!(pattern_matches(b"\x01", b"[[:cntrl:]]"));
                assert!(!pattern_matches(b"a", b"[[:cntrl:]]"));
                assert!(pattern_matches(b"9", b"[[:digit:]]"));
                assert!(!pattern_matches(b"a", b"[[:digit:]]"));
                assert!(pattern_matches(b"!", b"[[:graph:]]"));
                assert!(!pattern_matches(b" ", b"[[:graph:]]"));
                assert!(pattern_matches(b"a", b"[[:lower:]]"));
                assert!(!pattern_matches(b"A", b"[[:lower:]]"));
                assert!(pattern_matches(b" ", b"[[:print:]]"));
                assert!(pattern_matches(b"a", b"[[:print:]]"));
                assert!(!pattern_matches(b"\x01", b"[[:print:]]"));
                assert!(pattern_matches(b".", b"[[:punct:]]"));
                assert!(!pattern_matches(b"a", b"[[:punct:]]"));
                assert!(pattern_matches(b"\n", b"[[:space:]]"));
                assert!(!pattern_matches(b"a", b"[[:space:]]"));
                assert!(pattern_matches(b"A", b"[[:upper:]]"));
                assert!(!pattern_matches(b"a", b"[[:upper:]]"));
                assert!(pattern_matches(b"f", b"[[:xdigit:]]"));
                assert!(pattern_matches(b"F", b"[[:xdigit:]]"));
                assert!(!pattern_matches(b"g", b"[[:xdigit:]]"));
                assert!(!pattern_matches(b"a", b"[[:bogus:]]"));
                assert!(pattern_matches(b"x", b"[[:x]"));
                assert!(!pattern_matches(b"", b"[a-z]"));

                assert_eq!(match_bracket(None, b"[a]", 0), None);
                assert_eq!(match_bracket(Some(b'a'), b"[", 0), None);
                assert_eq!(match_bracket(Some(b']'), b"[\\]]", 0), Some((true, 4)));
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        b"*".to_vec(),
                        QuoteState::Quoted
                    )]),
                    b"\\*".to_vec()
                );
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        b"ab".to_vec(),
                        QuoteState::Literal
                    )]),
                    b"ab".to_vec()
                );
                assert_eq!(
                    render_pattern_from_segments(&[
                        Segment::Text(b"x".to_vec(), QuoteState::Literal),
                        Segment::AtBreak,
                        Segment::Text(b"y".to_vec(), QuoteState::Expanded),
                    ]),
                    b"xy".to_vec()
                );
            },
        );
    }

    #[test]
    fn supports_pattern_removal_parameter_expansions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"PATHNAME".to_vec(), b"src/bin/main.rs".to_vec());
        ctx.env.insert(b"DOTTED".to_vec(), b"alpha.beta.gamma".to_vec());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME#*/}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("small prefix"),
            vec![b"bin/main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME##*/}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("large prefix"),
            vec![b"main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME%/*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("small suffix"),
            vec![b"src/bin".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME%%/*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("large suffix"),
            vec![b"src".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME#\"src/\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted pattern"),
            vec![b"bin/main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED#*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("wildcard prefix"),
            vec![b"beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED##*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("largest wildcard prefix"),
            vec![b"gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%.*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("wildcard suffix"),
            vec![b"alpha.beta".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%%.*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("largest wildcard suffix"),
            vec![b"alpha".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED#\"*.\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted wildcard"),
            vec![b"alpha.beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("empty suffix pattern"),
            vec![b"alpha.beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${MISSING%%*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("unset value"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn arithmetic_parser_covers_more_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx, b"9 - 2 - 1").expect("subtract"), 6);
        assert_eq!(eval_arithmetic(&mut ctx, b"8 / 2").expect("divide"), 4);
        assert_eq!(eval_arithmetic(&mut ctx, b"9 % 4").expect("modulo"), 1);
        assert_eq!(eval_arithmetic(&mut ctx, b"(1 + 2)").expect("parens"), 3);
        assert_eq!(eval_arithmetic(&mut ctx, b"-5").expect("negation"), -5);

        let error = eval_arithmetic(&mut ctx, b"5 % 0").expect_err("mod zero");
        assert_eq!(&*error.message, b"division by zero".as_ref());

        let error = eval_arithmetic(&mut ctx, b"999999999999999999999999999999999999999")
            .expect_err("overflow");
        assert_eq!(&*error.message, b"invalid arithmetic operand".as_ref());
    }

    #[test]
    fn default_pathname_context_trait_impl() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(ctx.special_param(b'?'), None);
        assert_eq!(ctx.positional_param(0).as_deref(), Some(b"meiksh".as_ref()));
        assert_eq!(ctx.positional_param(1), None);
        assert!(ctx.positional_params().is_empty());
        assert!(ctx.home_dir_for_user(b"nobody").is_none());
        assert!(!ctx.nounset_enabled());
        ctx.set_var(b"NAME", b"value".to_vec()).expect("set var");
        assert_eq!(ctx.env_var(b"NAME").as_deref(), Some(b"value".as_ref()));
        assert_eq!(ctx.shell_name(), b"meiksh");
        assert_eq!(
            ctx.command_substitute(b"printf ok").expect("substitute"),
            b"printf ok\n"
        );
    }

    #[test]
    fn unmatched_glob_returns_pattern_literally() {
        let arena = ByteArena::new();
        run_trace(
            vec![t(
                "opendir",
                vec![ArgMatcher::Any],
                TraceResult::Err(crate::sys::ENOENT),
            )],
            || {
                let mut ctx = DefaultPathContext::new();
                assert_eq!(
                    expand_word(
                        &mut ctx,
                        &Word {
                            raw: b"*.definitely-no-match".as_ref().into(),
                            line: 0
                        },
                        &arena,
                    )
                    .expect("unmatched glob"),
                    vec![b"*.definitely-no-match".as_ref()]
                );
            },
        );
    }

    #[test]
    fn bracket_helpers_cover_missing_closer() {
        assert_eq!(match_bracket(Some(b'a'), b"[a", 0), None);
    }

    #[test]
    fn expands_here_documents_without_field_splitting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let expanded = expand_here_document(
            &mut ctx,
            b"hello $USER\n$(printf hi)\n$((1 + 2))\n",
            0,
            &arena,
        )
        .expect("expand heredoc");
        assert_eq!(expanded, b"hello meiksh\nprintf hi\n3\n");

        let escaped = expand_here_document(&mut ctx, b"\\$USER\nline\\\ncontinued\n", 0, &arena)
            .expect("expand heredoc");
        assert_eq!(escaped, b"$USER\nlinecontinued\n");

        let trailing = expand_here_document(&mut ctx, b"keep\\", 0, &arena).expect("expand heredoc");
        assert_eq!(trailing, b"keep\\");

        let literal = expand_here_document(&mut ctx, b"\\x", 0, &arena).expect("expand heredoc");
        assert_eq!(literal, b"\\x");

        let double_backslash = expand_here_document(&mut ctx, b"a\\\\b\n", 0, &arena)
            .expect("expand heredoc double backslash");
        assert_eq!(double_backslash, b"a\\b\n");
    }

    #[test]
    fn quoted_at_produces_separate_fields() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted at 3"),
            vec![b"a".as_ref(), b"b", b"c"]
        );

        ctx.positional = vec![b"one".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted at 1"),
            vec![b"one".as_ref()]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted at 0"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn quoted_at_with_prefix_and_suffix() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"pre$@suf\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("prefix suffix"),
            vec![b"prea".as_ref(), b"bsuf"]
        );

        ctx.positional = vec![b"only".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"[$@]\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("brackets one"),
            vec![b"[only]".as_ref()]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"pre$@suf\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("prefix empty"),
            vec![b"presuf".as_ref()]
        );
    }

    #[test]
    fn quoted_at_at_produces_merged_fields() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@$@\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("at at"),
            vec![b"a".as_ref(), b"ba", b"b"]
        );
    }

    #[test]
    fn unquoted_at_undergoes_field_splitting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a b".to_vec(), b"c".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$@".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("unquoted at"),
            vec![b"a".as_ref(), b"b", b"c"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$@".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("unquoted at empty"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn quoted_star_joins_with_ifs() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("star colon"),
            vec![b"a:b:c".as_ref()]
        );

        ctx.env.insert(b"IFS".to_vec(), Vec::new());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("star empty ifs"),
            vec![b"abc".as_ref()]
        );

        ctx.env.remove(b"IFS".as_ref());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("star unset ifs"),
            vec![b"a b c".as_ref()]
        );
    }

    #[test]
    fn backtick_command_substitution_in_expander() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"`echo hello`".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("backtick"),
            vec![b"echo".as_ref(), b"hello"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"`echo hello`\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted bt"),
            vec![b"echo hello".as_ref()]
        );
    }

    #[test]
    fn backtick_backslash_escapes() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"`echo \\$USER`".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escaped dollar"),
            b"echo $USER"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"\"`echo \\$USER`\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escaped dollar dq"),
            b"echo $USER"
        );
    }

    #[test]
    fn brace_scanning_respects_quotes_and_nesting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"VAR".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\"a}b\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted brace in default"),
            b"a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$(echo ok)}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("command sub in brace"),
            b"echo ok"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$((1+2))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("arith in brace"),
            b"3"
        );

        ctx.env.insert(b"INNER".to_vec(), b"val".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-${INNER}}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested brace"),
            b"val"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-`echo hi`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("backtick in brace"),
            b"echo hi"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-'a}b'}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("single quote in brace"),
            b"a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\\}}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escaped brace"),
            b"}"
        );
    }

    #[test]
    fn here_document_expands_at_sign() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
        let result = expand_here_document(&mut ctx, b"$@\n", 0, &arena).expect("heredoc at");
        assert_eq!(result, b"x y\n");
    }

    #[test]
    fn error_parameter_expansion_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?custom error}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon question");
        assert_eq!(&*error.message, b"custom error".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET?also error}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("question");
        assert_eq!(&*error.message, b"also error".as_ref());
    }

    #[test]
    fn segment_bytes_skips_at_break() {
        let segs = vec![
            Segment::Text(b"a".to_vec(), QuoteState::Expanded),
            Segment::AtBreak,
            Segment::Text(b"b".to_vec(), QuoteState::Quoted),
        ];
        let chars: Vec<_> = segment_bytes(&segs).collect();
        assert_eq!(
            chars,
            vec![(b'a', QuoteState::Expanded), (b'b', QuoteState::Quoted)]
        );
    }

    #[test]
    fn scan_to_closing_brace_error_on_unterminated() {
        let err = scan_to_closing_brace(b"${var", 2).expect_err("unterminated");
        assert_eq!(&*err.message, b"unterminated parameter expansion".as_ref());
    }

    #[test]
    fn expand_word_empty_quoted_at_with_other_quoted() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\"\"$@\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("empty at dq"),
            vec![b"".as_ref()]
        );
    }

    #[test]
    fn backtick_inside_double_quotes_with_buffer() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"hello `echo world`\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("bt dq buffer"),
            vec![b"hello echo world".as_ref()]
        );
    }

    #[test]
    fn scan_backtick_command_unterminated() {
        let mut index = 1usize;
        let err =
            scan_backtick_command(b"`unterminated", &mut index, false).expect_err("unterminated");
        assert_eq!(&*err.message, b"unterminated backquote".as_ref());
    }

    #[test]
    fn scan_backtick_command_escape_outside_dq() {
        let mut index = 1usize;
        let result = scan_backtick_command(b"`echo \\\\ok`", &mut index, false).expect("bt escape");
        assert_eq!(result, b"echo \\ok");
    }

    #[test]
    fn here_document_with_at_expansion() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        let result = expand_here_document(&mut ctx, b"args: $@\n", 0, &arena).expect("heredoc @");
        assert_eq!(result, b"args: a b\n");
    }

    #[test]
    fn brace_scanning_handles_complex_nesting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"VAR".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$((2+3))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("arith in brace scan"),
            b"5"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$(echo deep)}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("cmd sub in brace scan"),
            b"echo deep"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-`echo bt`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("backtick in brace scan"),
            b"echo bt"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\"inside\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("dq in brace scan with escape"),
            b"inside"
        );
    }

    #[test]
    fn error_parameter_expansion_with_null_or_not_set() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${EMPTY:?null or unset}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon question null");
        assert_eq!(&*err.message, b"null or unset".as_ref());

        let ok = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"${EMPTY?not an error}\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("question set but empty");
        assert_eq!(ok, vec![b"".as_ref()]);

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOEXIST?custom msg}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("question unset");
        assert_eq!(&*err.message, b"custom msg".as_ref());
    }

    #[test]
    fn field_splitting_empty_result_returns_empty_vec() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"WS".to_vec(), b"   ".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$WS".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("whitespace only"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn at_break_with_glob_in_at_fields() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.pathname_expansion_enabled = false;
        ctx.positional = vec![b"*.txt".to_vec(), b"b".to_vec()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("at with glob-like");
        assert_eq!(result, vec![b"*.txt".as_ref(), b"b"]);
    }

    #[test]
    fn flatten_expansion_covers_at_fields() {
        assert_eq!(flatten_expansion(Expansion::One(b"hello".to_vec())), b"hello");
        assert_eq!(
            flatten_expansion(Expansion::AtFields(vec![b"a".to_vec(), b"b".to_vec()])),
            b"a b"
        );
    }

    #[test]
    fn scan_backtick_non_special_escape_in_dquote() {
        let mut index = 1usize;
        let result =
            scan_backtick_command(b"`echo \\x`", &mut index, true).expect("non-special escape");
        assert_eq!(result, b"echo \\x");
    }

    #[test]
    fn at_empty_combined_with_at_break() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("at one param");
        assert_eq!(result, vec![b"x".as_ref()]);

        ctx.positional = Vec::new();
        let result2 = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("at empty");
        assert_eq!(result2, Vec::<&[u8]>::new());
    }

    #[test]
    fn brace_scanning_with_arith_and_cmd_sub_and_backtick() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-$((1+(2*3)))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested arith in scan"),
            b"7"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-$(echo (hi))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested cmd sub in scan"),
            b"echo (hi)"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-`echo \\\\x`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("bt escape in scan"),
            b"echo \\x"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-\"q\\}x\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("dq escape in scan"),
            b"q}x"
        );
    }

    #[test]
    fn colon_question_error_with_null_value() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"NULL".to_vec(), Vec::new());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NULL:?is null}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err(":? with null");
        assert_eq!(&*err.message, b"is null".as_ref());

        ctx.nounset_enabled = true;
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NULL:?$NOVAR}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err(":? nounset propagation");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOEXIST?$NOVAR}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("? nounset propagation");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());
    }

    #[test]
    fn question_error_with_unset_default_message() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOVAR?}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("? with unset");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());

        ctx.env.insert(b"SET".to_vec(), b"val".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${SET:?no error}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect(":? success"),
            b"val"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${SET?no error}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("? success"),
            b"val"
        );

        let err_colon = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOVAR:?}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err(":? with unset");
        assert_eq!(&*err_colon.message, b"NOVAR: parameter null or not set".as_ref());
    }

    #[test]
    fn dquote_backslash_preserves_literal_for_non_special_chars() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"\\a\\b\\c\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("dquote bs");
        assert_eq!(fields, vec![b"\\a\\b\\c".as_ref()]);
    }

    #[test]
    fn dquote_backslash_escapes_special_chars() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\$\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escape $"),
            vec![b"$".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\\\\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escape bs"),
            vec![b"\\".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\\"\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escape dq"),
            vec![b"\"".as_ref()],
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\`\"".as_ref().into(),
                    line: 0,
                },
                &arena,
            )
            .expect("escape bt"),
            vec![b"`".as_ref()]
        );
    }

    #[test]
    fn dquote_backslash_newline_is_line_continuation() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"ab\\\ncd\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("line continuation");
        assert_eq!(fields, vec![b"abcd".as_ref()]);
    }

    #[test]
    fn tilde_user_expansion() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~testuser/bin".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("tilde user");
        assert_eq!(fields, vec![b"/home/testuser/bin".as_ref()]);
    }

    #[test]
    fn tilde_unknown_user_preserved() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~nosuchuser/dir".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("tilde unknown");
        assert_eq!(fields, vec![b"~nosuchuser/dir".as_ref()]);
    }

    #[test]
    fn tilde_user_without_slash() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~testuser".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("tilde user no slash");
        assert_eq!(fields, vec![b"/home/testuser".as_ref()]);
    }

    #[test]
    fn tilde_after_colon_in_assignment() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: b"~/bin:~testuser/lib".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("tilde colon");
        assert_eq!(result, b"/tmp/home/bin:/home/testuser/lib");
    }

    #[test]
    fn arith_variable_reference() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"count".to_vec(), b"7".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((count + 3))".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("arith var");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_dollar_variable_reference() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"n".to_vec(), b"5".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$(($n * 2))".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("arith $var");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_comparison_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 < 5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((5 < 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 <= 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((5 > 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 >= 5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 == 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 != 5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
    }

    #[test]
    fn arith_bitwise_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((6 & 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"2".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((6 | 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"7".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((6 ^ 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"5".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((~0))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"-1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((1 << 4))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"16".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((16 >> 2))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"4".as_ref()]);
    }

    #[test]
    fn arith_logical_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((1 && 1))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((1 && 0))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0 || 1))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0 || 0))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((!0))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((!5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
    }

    #[test]
    fn arith_logical_and_short_circuits() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(&mut ctx, &Word { raw: b"$((0 && (x = 5)))".as_ref().into(), line: 0 }, &arena).unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_logical_or_short_circuits() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(&mut ctx, &Word { raw: b"$((1 || (x = 5)))".as_ref().into(), line: 0 }, &arena).unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_ternary_short_circuits() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(&mut ctx, &Word { raw: b"$((1 ? 10 : (x = 99)))".as_ref().into(), line: 0 }, &arena).unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_ternary_operator() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((1 ? 10 : 20))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"10".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0 ? 10 : 20))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"20".as_ref()]);
    }

    #[test]
    fn arith_assignment_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"10".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x = 5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"5".as_ref()]);
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"5");

        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x += 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"8".as_ref()]);
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"8");

        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x -= 2))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"6".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x *= 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"18".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x /= 6))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"3".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x %= 2))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);

        ctx.env.insert(b"x".to_vec(), b"4".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x <<= 2))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"16".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x >>= 1))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"8".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x &= 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);

        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x |= 2))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"7".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x ^= 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"4".as_ref()]);
    }

    #[test]
    fn arith_hex_and_octal_constants() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0xff))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"255".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0X1A))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"26".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((010))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"8".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((0))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
    }

    #[test]
    fn arith_unary_plus() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((+5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"5".as_ref()]);
    }

    #[test]
    fn arith_unset_variable_is_zero() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((nosuch))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
    }

    #[test]
    fn arith_nested_parens_and_precedence() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((2 + 3 * 4))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"14".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$(((2 + 3) * 4))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"20".as_ref()]);
    }

    #[test]
    fn arith_variable_in_hex_value() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"h".to_vec(), b"0xff".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((h))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"255".as_ref()]);
    }

    #[test]
    fn arith_variable_in_octal_value() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"o".to_vec(), b"010".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((o))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"8".as_ref()]);
    }

    #[test]
    fn split_colons_handles_quotes_and_backslash() {
        let parts = split_on_unquoted_colons(b"'b:c':d");
        assert_eq!(parts, vec![b"'b:c'".to_vec(), b"d".to_vec()]);

        let parts = split_on_unquoted_colons(b"\"b:c\":d");
        assert_eq!(parts, vec![b"\"b:c\"".to_vec(), b"d".to_vec()]);

        let parts = split_on_unquoted_colons(b"a\\:b:c");
        assert_eq!(parts, vec![b"a\\:b".to_vec(), b"c".to_vec()]);

        let parts = split_on_unquoted_colons(b"\"a\\\"b\":c");
        assert_eq!(parts, vec![b"\"a\\\"b\"".to_vec(), b"c".to_vec()]);

        let parts = split_on_unquoted_colons(b"${x:-a:b}:c");
        assert_eq!(parts, vec![b"${x:-a:b}".to_vec(), b"c".to_vec()]);

        let parts = split_on_unquoted_colons(b"$(echo a:b):c");
        assert_eq!(parts, vec![b"$(echo a:b)".to_vec(), b"c".to_vec()]);

        let parts = split_on_unquoted_colons(b"${a:-${b:-x:y}}:z");
        assert_eq!(parts, vec![b"${a:-${b:-x:y}}".to_vec(), b"z".to_vec()]);
    }

    #[test]
    fn dquote_trailing_backslash_is_literal() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"abc\\".as_ref().into(),
                line: 0,
            },
            &arena,
        );
        assert!(fields.is_err());
    }

    #[test]
    fn tilde_with_quoted_char_breaks_tilde_prefix() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~'user'".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("tilde quoted");
        assert_eq!(fields, vec![b"~user".as_ref()]);
    }

    #[test]
    fn arith_backtick_in_expression() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((`7` + 3))".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("arith backtick");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_not_equal_via_parse_unary() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((3 != 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
    }

    #[test]
    fn arith_compound_assign_div_by_zero() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        let err = expand_word(&mut ctx, &Word { raw: b"$((x /= 0))".as_ref().into(), line: 0 }, &arena).unwrap_err();
        assert_eq!(&*err.message, b"division by zero".as_ref());

        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        let err = expand_word(&mut ctx, &Word { raw: b"$((x %= 0))".as_ref().into(), line: 0 }, &arena).unwrap_err();
        assert_eq!(&*err.message, b"division by zero".as_ref());
    }

    #[test]
    fn tilde_colon_assignment_with_quotes() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: b"~/a:'literal:colon'".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("colon assign with quotes");
        assert_eq!(result, b"/tmp/home/a:literal:colon");
    }

    #[test]
    fn arith_equality_not_confused_with_assignment() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x == 5))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"1".as_ref()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: b"$((x == 3))".as_ref().into(), line: 0 }, &arena).unwrap(), vec![b"0".as_ref()]);
    }

    #[test]
    fn arith_ternary_missing_colon_error() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(&mut ctx, &Word { raw: b"$((1 ? 2 3))".as_ref().into(), line: 0 }, &arena).unwrap_err();
        assert!(err.message.windows(3).any(|w| w == b"':'"));
    }

    #[test]
    fn arith_invalid_hex_constant() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(&mut ctx, &Word { raw: b"$((0x))".as_ref().into(), line: 0 }, &arena).unwrap_err();
        assert!(err.message.windows(3).any(|w| w == b"hex"));
    }

    #[test]
    fn arith_at_fields_in_expression() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"3".to_vec()];
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$(($@ + 2))".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("at fields arith");
        assert_eq!(fields, vec![b"5".as_ref()]);
    }

    #[test]
    fn apply_compound_assign_unknown_op_returns_error() {
        let err = apply_compound_assign(b"??=", 1, 2).unwrap_err();
        assert!(err.message.windows(7).any(|w| w == b"unknown"));
    }

    #[test]
    fn expand_braced_parameter_pattern_removal_operators() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"FILE".to_vec(), b"archive.tar.gz".to_vec());

            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE%.*", false).unwrap(),
                Expansion::One(b"archive.tar".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE%%.*", false).unwrap(),
                Expansion::One(b"archive".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE#*.", false).unwrap(),
                Expansion::One(b"tar.gz".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE##*.", false).unwrap(),
                Expansion::One(b"gz".to_vec())
            );
        });
    }

    #[test]
    fn scan_to_closing_brace_skips_backslash() {
        assert_no_syscalls(|| {
            let pos = scan_to_closing_brace(b"a\\}b}", 0).unwrap();
            assert_eq!(pos, 4);
        });
    }

    #[test]
    fn expand_parameter_word_as_expansion_with_at_fields() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
            let result = expand_parameter_word_as_expansion(&mut ctx, b"\"$@\"", false).unwrap();
            assert_eq!(result, Expansion::AtFields(vec![b"x".to_vec(), b"y".to_vec()]));
        });
    }

    #[test]
    fn expand_word_quoted_null_adjacent_to_empty_at() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = Vec::new();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: b"''\"$@\"".as_ref().into(),
                        line: 0
                    },
                    &arena,
                )
                .unwrap(),
                vec![b"".as_ref()]
            );
        });
    }

    #[test]
    fn redirect_word_no_pathname_expansion() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"file_*.txt".as_ref().into(),
                    line: 0,
                },
                &arena,
            )
            .expect("redirect word");
            assert_eq!(result, b"file_*.txt");
        });
    }

    #[test]
    fn redirect_word_empty_expansion() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"$UNSET_VAR".as_ref().into(),
                    line: 0,
                },
                &arena,
            )
            .expect("redirect word empty");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn redirect_word_with_expanded_field_splitting() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"V".to_vec(), b"a b".to_vec());
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"$V".as_ref().into(),
                    line: 0,
                },
                &arena,
            )
            .expect("redirect word split");
            assert_eq!(result, b"a b");
        });
    }

    #[test]
    fn here_doc_backtick_substitution() {
        let arena = ByteArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_here_document(&mut ctx, b"`echo ok`\n", 0, &arena)
                .expect("here doc backtick");
            assert_eq!(result, b"echo ok\n");
        });
    }

    #[test]
    fn word_is_assignment_rejects_empty_and_non_identifier_prefix() {
        assert!(!word_is_assignment(b""));
        assert!(!word_is_assignment(b"a-b=c"));
    }

    #[test]
    fn fake_context_special_param_star_and_at() {
        let ctx = FakeContext::new();
        assert_eq!(ctx.special_param(b'*').as_deref(), Some(b"alpha beta".as_ref()));
        assert_eq!(ctx.special_param(b'@').as_deref(), Some(b"alpha beta".as_ref()));
    }

    #[test]
    fn newline_inside_single_quote_increments_lineno() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"'hello\nworld'".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("single quote newline");
        assert_eq!(fields, vec![b"hello\nworld".as_ref()]);
    }

    #[test]
    fn newline_inside_double_quote_increments_lineno() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"hello\nworld\"".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("double quote newline");
        assert_eq!(fields, vec![b"hello\nworld".as_ref()]);
    }

    #[test]
    fn backslash_newline_inside_double_quote() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"a\\\nb\"".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("backslash newline in dquote");
        assert_eq!(fields, vec![b"ab".as_ref()]);
    }

    #[test]
    fn backslash_escape_in_unquoted_context() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\\a\\b".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("backslash escape");
        assert_eq!(fields, vec![b"ab".as_ref()]);
    }

    #[test]
    fn backslash_newline_in_unquoted_context() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"a\\\nb".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("backslash newline unquoted");
        assert_eq!(fields.len(), 1);
        assert!(fields[0].contains(&b'\n'));
    }

    #[test]
    fn trailing_backslash_in_unquoted_context() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"a\\".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("trailing backslash");
        assert_eq!(fields, vec![b"a".as_ref()]);
    }

    #[test]
    fn bare_newline_in_unquoted_context() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"hello\nworld".as_ref().into(),
                line: 1,
            },
            &arena,
        )
        .expect("bare newline");
        assert_eq!(fields.len(), 1);
        assert!(fields[0].contains(&b'\n'));
    }

    #[test]
    fn arithmetic_nounset_rejects_unset_variable() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((nosuch_var))".as_ref().into(),
                line: 0,
            },
            &arena,
        );
        assert!(result.is_err());
    }
}
