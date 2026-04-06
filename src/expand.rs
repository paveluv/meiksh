use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::arena::StringArena;
use crate::syntax::Word;
use crate::sys;

#[derive(Debug)]
pub struct ExpandError {
    pub message: Box<str>,
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExpandError {}

fn char_at(s: &str, i: usize) -> char {
    s.as_bytes().get(i).map_or('\0', |&b| {
        if b < 0x80 {
            b as char
        } else {
            s[i..].chars().next().unwrap_or('\0')
        }
    })
}

fn char_len(s: &str, i: usize) -> usize {
    let b = s.as_bytes()[i];
    if b < 0x80 {
        1
    } else {
        s[i..].chars().next().map_or(1, |c| c.len_utf8())
    }
}

pub trait Context {
    fn env_var(&self, name: &str) -> Option<Cow<'_, str>>;
    fn special_param(&self, name: char) -> Option<Cow<'_, str>>;
    fn positional_param(&self, index: usize) -> Option<Cow<'_, str>>;
    fn positional_params(&self) -> &[String];
    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &str;
    fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError>;
    fn home_dir_for_user(&self, name: &str) -> Option<Cow<'_, str>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    Quoted,
    Literal,
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String, QuoteState),
    AtBreak,
    AtEmpty,
}

#[derive(Debug, PartialEq, Eq)]
enum Expansion {
    One(String),
    AtFields(Vec<String>),
}

pub fn expand_words<'a, C: Context>(
    ctx: &mut C,
    words: &[Word],
    arena: &'a StringArena,
) -> Result<Vec<&'a str>, ExpandError> {
    let mut result = Vec::new();
    for word in words {
        result.extend(expand_word(ctx, word, arena)?);
    }
    Ok(result)
}

pub fn expand_word_as_declaration_assignment<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    let value_word = Word {
        raw: word_assignment_value(word.raw).unwrap_or(word.raw),
    };
    let name = &word.raw[..word.raw.len() - value_word.raw.len()];
    let expanded_value = expand_word_text_assignment(ctx, &value_word, true, arena)?;
    Ok(arena.intern(format!("{name}{expanded_value}")))
}

pub fn word_is_assignment(raw: &str) -> bool {
    word_assignment_value(raw).is_some()
}

fn word_assignment_value(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let first = bytes[0];
    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
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
    arena: &'a StringArena,
) -> Result<Vec<&'a str>, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;

    if expanded.has_at_expansion {
        let fields = expand_word_with_at_fields(&expanded, expanded.had_quoted_null_outside_at)?;
        return Ok(fields.into_iter().map(|s| arena.intern(s)).collect());
    }

    if expanded.segments.is_empty() {
        if expanded.had_quoted_content {
            return Ok(vec![arena.intern(String::new())]);
        }
        return Ok(Vec::new());
    }

    let has_expanded = expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, QuoteState::Expanded)));

    let ifs_cow = ctx.env_var("IFS").unwrap_or(Cow::Borrowed(" \t\n"));
    let fields = if has_expanded {
        split_fields_from_segments(&expanded.segments, &ifs_cow)
    } else {
        let has_glob = expanded.segments.iter().any(|seg| {
            matches!(seg, Segment::Text(text, QuoteState::Literal) if text.chars().any(is_glob_char))
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
                result.push(arena.intern(field.text));
            } else {
                for m in matches {
                    result.push(arena.intern(m));
                }
            }
        } else {
            result.push(arena.intern(field.text));
        }
    }
    Ok(result)
}

pub fn expand_redirect_word<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;

    if expanded.segments.is_empty() {
        return Ok(arena.intern(String::new()));
    }

    let has_expanded = expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, QuoteState::Expanded)));

    let ifs_cow = ctx.env_var("IFS").unwrap_or(Cow::Borrowed(" \t\n"));
    let fields = if has_expanded {
        split_fields_from_segments(&expanded.segments, &ifs_cow)
    } else {
        vec![Field {
            text: flatten_segments(&expanded.segments),
            has_unquoted_glob: false,
        }]
    };

    Ok(arena.intern(
        fields
            .into_iter()
            .map(|f| f.text)
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn expand_word_with_at_fields(
    expanded: &ExpandedWord,
    had_quoted_null_outside_at: bool,
) -> Result<Vec<String>, ExpandError> {
    let has_at_empty = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtEmpty));
    let has_at_break = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak));

    if has_at_empty && !has_at_break {
        let mut text = String::new();
        for seg in &expanded.segments {
            if let Segment::Text(t, _) = seg {
                text.push_str(t);
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
    let mut current = String::new();

    for seg in &expanded.segments {
        if let Segment::Text(text, _) = seg {
            current.push_str(text);
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
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    expand_word_text_assignment(ctx, word, false, arena)
}

pub fn expand_word_pattern<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(arena.intern(render_pattern_from_segments(&expanded.segments)))
}

pub fn expand_assignment_value<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    expand_word_text_assignment(ctx, word, true, arena)
}

fn expand_word_text_assignment<'a, C: Context>(
    ctx: &mut C,
    word: &Word,
    assignment_rhs: bool,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    if !assignment_rhs {
        let expanded = expand_raw(ctx, &word.raw)?;
        return Ok(arena.intern(flatten_segments(&expanded.segments)));
    }
    let raw = &word.raw;
    let mut result = String::new();
    let mut first = true;
    for part in split_on_unquoted_colons(raw) {
        if !first {
            result.push(':');
        }
        first = false;
        let expanded = expand_raw(ctx, &part)?;
        result.push_str(&flatten_segments(&expanded.segments));
    }
    Ok(arena.intern(result))
}

fn split_on_unquoted_colons(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    while i < raw.len() {
        match raw.as_bytes()[i] {
            b'\'' if brace_depth == 0 && paren_depth == 0 => {
                current.push('\'');
                i += 1;
                while i < raw.len() && raw.as_bytes()[i] != b'\'' {
                    let clen = char_len(raw, i);
                    current.push_str(&raw[i..i + clen]);
                    i += clen;
                }
                if i < raw.len() {
                    current.push('\'');
                    i += 1;
                }
            }
            b'"' if brace_depth == 0 && paren_depth == 0 => {
                current.push('"');
                i += 1;
                while i < raw.len() && raw.as_bytes()[i] != b'"' {
                    if raw.as_bytes()[i] == b'\\' && i + 1 < raw.len() {
                        current.push('\\');
                        i += 1;
                    }
                    let clen = char_len(raw, i);
                    current.push_str(&raw[i..i + clen]);
                    i += clen;
                }
                if i < raw.len() {
                    current.push('"');
                    i += 1;
                }
            }
            b'\\' => {
                current.push('\\');
                i += 1;
                if i < raw.len() {
                    let clen = char_len(raw, i);
                    current.push_str(&raw[i..i + clen]);
                    i += clen;
                }
            }
            b'$' if i + 1 < raw.len() && raw.as_bytes()[i + 1] == b'{' => {
                current.push_str("${");
                brace_depth += 1;
                i += 2;
            }
            b'}' if brace_depth > 0 => {
                brace_depth -= 1;
                current.push('}');
                i += 1;
            }
            b'$' if i + 1 < raw.len() && raw.as_bytes()[i + 1] == b'(' => {
                current.push_str("$(");
                paren_depth += 1;
                i += 2;
            }
            b')' if paren_depth > 0 => {
                paren_depth -= 1;
                current.push(')');
                i += 1;
            }
            b':' if brace_depth == 0 && paren_depth == 0 => {
                parts.push(std::mem::take(&mut current));
                i += 1;
            }
            _ => {
                let clen = char_len(raw, i);
                current.push_str(&raw[i..i + clen]);
                i += clen;
            }
        }
    }
    parts.push(current);
    parts
}

pub fn expand_parameter_text<'a, C: Context>(
    ctx: &mut C,
    raw: &str,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    Ok(arena.intern(expand_parameter_text_owned(ctx, raw)?))
}

fn expand_parameter_text_owned<C: Context>(ctx: &mut C, raw: &str) -> Result<String, ExpandError> {
    let mut result = String::new();
    let mut index = 0usize;

    while index < raw.len() {
        if raw.as_bytes()[index] == b'$' {
            let (value, consumed) = expand_parameter_dollar(ctx, &raw[index..])?;
            result.push_str(&value);
            index += consumed;
        } else {
            let clen = char_len(raw, index);
            result.push_str(&raw[index..index + clen]);
            index += clen;
        }
    }

    Ok(result)
}

fn flatten_expansion(expansion: Expansion) -> String {
    match expansion {
        Expansion::One(s) => s,
        Expansion::AtFields(fields) => fields.join(" "),
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

fn expand_raw<C: Context>(ctx: &mut C, raw: &str) -> Result<ExpandedWord, ExpandError> {
    let mut index = 0usize;
    let mut segments = Vec::new();
    let mut had_quoted_content = false;
    let mut had_quoted_null_outside_at = false;
    let mut has_at_expansion = false;

    while index < raw.len() {
        match raw.as_bytes()[index] {
            b'\'' => {
                had_quoted_content = true;
                had_quoted_null_outside_at = true;
                index += 1;
                let start = index;
                while index < raw.len() && raw.as_bytes()[index] != b'\'' {
                    index += char_len(raw, index);
                }
                if index >= raw.len() {
                    return Err(ExpandError {
                        message: "unterminated single quote".into(),
                    });
                }
                push_segment_str(&mut segments, &raw[start..index], QuoteState::Quoted);
                index += 1;
            }
            b'"' => {
                had_quoted_content = true;
                index += 1;
                let mut buffer = String::new();
                let at_before = has_at_expansion;
                while index < raw.len() && raw.as_bytes()[index] != b'"' {
                    match raw.as_bytes()[index] {
                        b'\\' => {
                            if index + 1 < raw.len() {
                                let next = raw.as_bytes()[index + 1] as char;
                                if matches!(next, '$' | '`' | '"' | '\\' | '\n' | '}') {
                                    index += 1;
                                    if next != '\n' {
                                        buffer.push(next);
                                    }
                                    index += 1;
                                } else {
                                    buffer.push('\\');
                                    index += 1;
                                }
                            } else {
                                buffer.push('\\');
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
                            let trimmed = output.trim_end_matches('\n').to_string();
                            push_segment(&mut segments, trimmed, QuoteState::Quoted);
                        }
                        _ => {
                            let clen = char_len(raw, index);
                            buffer.push_str(&raw[index..index + clen]);
                            index += clen;
                        }
                    }
                }
                if index >= raw.len() {
                    return Err(ExpandError {
                        message: "unterminated double quote".into(),
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
                    let clen = char_len(raw, index);
                    push_segment_str(&mut segments, &raw[index..index + clen], QuoteState::Quoted);
                    index += clen;
                }
            }
            b'$' => {
                let dollar_single_quotes = raw.as_bytes().get(index + 1) == Some(&b'\'');
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
                let trimmed = output.trim_end_matches('\n').to_string();
                push_segment(&mut segments, trimmed, QuoteState::Expanded);
            }
            b'~' if index == 0 => {
                index += 1;
                let mut user = String::new();
                let at_start = index;
                while index < raw.len() && raw.as_bytes()[index] != b'/' {
                    let b = raw.as_bytes()[index];
                    if b == b'\'' || b == b'"' || b == b'\\' || b == b'$' || b == b'`' {
                        break;
                    }
                    let clen = char_len(raw, index);
                    user.push_str(&raw[index..index + clen]);
                    index += clen;
                }
                let broke_on_non_login =
                    index == at_start && index < raw.len() && raw.as_bytes()[index] != b'/';
                if broke_on_non_login {
                    push_segment_str(&mut segments, "~", QuoteState::Literal);
                } else if user.is_empty() {
                    match ctx.env_var("HOME") {
                        Some(home) if !home.is_empty() => {
                            push_segment(&mut segments, home.into_owned(), QuoteState::Quoted);
                        }
                        Some(_) => {
                            segments.push(Segment::Text(String::new(), QuoteState::Quoted));
                        }
                        None => {
                            push_segment_str(&mut segments, "~", QuoteState::Literal);
                        }
                    }
                } else if let Some(dir) = ctx.home_dir_for_user(&user) {
                    push_segment(&mut segments, dir.into_owned(), QuoteState::Quoted);
                } else {
                    let mut literal = String::from('~');
                    literal.push_str(&user);
                    push_segment(&mut segments, literal, QuoteState::Literal);
                }
            }
            _ => {
                let clen = char_len(raw, index);
                push_segment_str(
                    &mut segments,
                    &raw[index..index + clen],
                    QuoteState::Literal,
                );
                index += clen;
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

fn scan_backtick_command(
    source: &str,
    index: &mut usize,
    in_double_quotes: bool,
) -> Result<String, ExpandError> {
    let mut command = String::new();
    while *index < source.len() {
        let ch = source.as_bytes()[*index];
        if ch == b'`' {
            *index += 1;
            return Ok(command);
        }
        if ch == b'\\' && *index + 1 < source.len() {
            let next = source.as_bytes()[*index + 1] as char;
            let special = if in_double_quotes {
                matches!(next, '$' | '`' | '\\' | '"' | '\n')
            } else {
                matches!(next, '$' | '`' | '\\')
            };
            if special {
                command.push(next);
                *index += 2;
                continue;
            }
        }
        command.push(ch as char);
        *index += 1;
    }
    Err(ExpandError {
        message: "unterminated backquote".into(),
    })
}

pub fn expand_here_document<'a, C: Context>(
    ctx: &mut C,
    text: &str,
    arena: &'a StringArena,
) -> Result<&'a str, ExpandError> {
    let mut result = String::new();
    let mut index = 0usize;

    while index < text.len() {
        match text.as_bytes()[index] {
            b'\\' => {
                index += 1;
                if index >= text.len() {
                    result.push('\\');
                    break;
                }
                match text.as_bytes()[index] {
                    b'$' | b'\\' => {
                        result.push(text.as_bytes()[index] as char);
                        index += 1;
                    }
                    b'\n' => {
                        index += 1;
                    }
                    _ => {
                        result.push('\\');
                        let clen = char_len(text, index);
                        result.push_str(&text[index..index + clen]);
                        index += clen;
                    }
                }
            }
            b'$' => {
                let (expansion, consumed) = expand_dollar(ctx, &text[index..], false)?;
                result.push_str(&flatten_expansion(expansion));
                index += consumed;
            }
            b'`' => {
                index += 1;
                let command = scan_backtick_command(text, &mut index, true)?;
                let output = ctx.command_substitute(&command)?;
                result.push_str(output.trim_end_matches('\n'));
            }
            _ => {
                let clen = char_len(text, index);
                result.push_str(&text[index..index + clen]);
                index += clen;
            }
        }
    }

    Ok(arena.intern(result))
}

fn expand_dollar<C: Context>(
    ctx: &mut C,
    source: &str,
    quoted: bool,
) -> Result<(Expansion, usize), ExpandError> {
    if source.len() < 2 {
        return Ok((Expansion::One("$".to_string()), 1));
    }

    let c1 = source.as_bytes()[1] as char;
    match c1 {
        '\'' if !quoted => {
            let (s, n) = parse_dollar_single_quoted(source)?;
            Ok((Expansion::One(s), n))
        }
        '{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let expansion = expand_braced_parameter(ctx, expr, quoted)?;
            Ok((expansion, end + 1))
        }
        '(' => {
            if source.as_bytes().get(2) == Some(&b'(') {
                let mut index = 3usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source.as_bytes()[index] as char;
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        if depth == 1 && source.as_bytes().get(index + 1) == Some(&b')') {
                            let expression = source[3..index].to_string();
                            let pre_expanded = expand_arithmetic_expression(ctx, &expression)?;
                            let value = eval_arithmetic(ctx, &pre_expanded)?;
                            return Ok((Expansion::One(value.to_string()), index + 2));
                        }
                        depth = depth.saturating_sub(1);
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: "unterminated arithmetic expansion".into(),
                })
            } else {
                let mut index = 2usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source.as_bytes()[index] as char;
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth == 0 {
                            let command = source[2..index].to_string();
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = output.trim_end_matches('\n').to_string();
                            return Ok((Expansion::One(trimmed), index + 1));
                        }
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: "unterminated command substitution".into(),
                })
            }
        }
        '@' => {
            if quoted {
                let params = ctx.positional_params().to_vec();
                Ok((Expansion::AtFields(params), 2))
            } else {
                let joined = Cow::Owned(ctx.positional_params().join(" "));
                let value = require_set_parameter(ctx, "@", Some(joined))?;
                Ok((Expansion::One(value), 2))
            }
        }
        '*' => {
            let ifs = ctx.env_var("IFS");
            let sep = match ifs.as_deref() {
                None => " ".to_string(),
                Some("") => String::new(),
                Some(s) => s.chars().next().unwrap().to_string(),
            };
            let value = ctx.positional_params().join(&sep);
            Ok((Expansion::One(value), 2))
        }
        '?' | '$' | '!' | '#' | '-' | '0' => {
            let ch_str = &source[1..2];
            let value = if c1 == '0' {
                require_set_parameter(ctx, "0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_str, ctx.special_param(c1))?
            };
            Ok((Expansion::One(value), 2))
        }
        next if next.is_ascii_digit() => Ok((
            Expansion::One(require_set_parameter(
                ctx,
                &source[1..2],
                ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize),
            )?),
            2,
        )),
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source.as_bytes()[index];
                if b == b'_' || (b as char).is_ascii_alphanumeric() {
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
        _ => Ok((Expansion::One("$".to_string()), 1)),
    }
}

fn expand_parameter_dollar<C: Context>(
    ctx: &mut C,
    source: &str,
) -> Result<(String, usize), ExpandError> {
    if source.len() < 2 {
        return Ok(("$".to_string(), 1));
    }

    let c1 = source.as_bytes()[1] as char;
    match c1 {
        '\'' => parse_dollar_single_quoted(source),
        '{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let value = expand_braced_parameter_text(ctx, expr)?;
            Ok((value, end + 1))
        }
        '?' | '$' | '!' | '#' | '*' | '@' | '-' | '0' => {
            let ch_str = &source[1..2];
            let value = if c1 == '0' {
                require_set_parameter(ctx, "0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_str, ctx.special_param(c1))?
            };
            Ok((value, 2))
        }
        next if next.is_ascii_digit() => {
            let value = ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize);
            Ok((require_set_parameter(ctx, &source[1..2], value)?, 2))
        }
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source.as_bytes()[index];
                if b == b'_' || (b as char).is_ascii_alphanumeric() {
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
        _ => Ok(("$".to_string(), 1)),
    }
}

fn parse_dollar_single_quoted(source: &str) -> Result<(String, usize), ExpandError> {
    let mut index = 2usize;
    let mut result = String::new();
    while index < source.len() {
        match source.as_bytes()[index] {
            b'\'' => return Ok((result, index + 1)),
            b'\\' => {
                index += 1;
                if index >= source.len() {
                    return Err(ExpandError {
                        message: "unterminated dollar-single-quotes".into(),
                    });
                }
                let ch = source.as_bytes()[index] as char;
                match ch {
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    '\\' => result.push('\\'),
                    'a' => result.push('\u{0007}'),
                    'b' => result.push('\u{0008}'),
                    'e' => result.push('\u{001b}'),
                    'f' => result.push('\u{000c}'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'v' => result.push('\u{000b}'),
                    'c' => {
                        index += 1;
                        if index >= source.len() {
                            return Err(ExpandError {
                                message: "unterminated dollar-single-quotes".into(),
                            });
                        }
                        if source.as_bytes()[index] == b'\\' && index + 1 < source.len() {
                            index += 1;
                            result.push(control_escape(source.as_bytes()[index] as char));
                        } else {
                            result.push(control_escape(source.as_bytes()[index] as char));
                        }
                    }
                    'x' => {
                        let (value, consumed) =
                            parse_variable_base_escape(&source[(index + 1)..], 16, 2);
                        if consumed == 0 {
                            result.push('x');
                        } else {
                            result.push(char::from(value));
                            index += consumed;
                        }
                    }
                    '0'..='7' => {
                        let mut digits = String::from(ch);
                        let mut consumed = 0usize;
                        while consumed < 2
                            && index + 1 + consumed < source.len()
                            && matches!(source.as_bytes()[index + 1 + consumed], b'0'..=b'7')
                        {
                            digits.push(source.as_bytes()[index + 1 + consumed] as char);
                            consumed += 1;
                        }
                        let value = u8::from_str_radix(&digits, 8).unwrap_or_default();
                        result.push(char::from(value));
                        index += consumed;
                    }
                    other => result.push(other),
                }
                index += 1;
            }
            _ => {
                let clen = char_len(source, index);
                result.push_str(&source[index..index + clen]);
                index += clen;
            }
        }
    }
    Err(ExpandError {
        message: "unterminated dollar-single-quotes".into(),
    })
}

fn scan_to_closing_brace(source: &str, start: usize) -> Result<usize, ExpandError> {
    let mut index = start;
    while index < source.len() {
        match source.as_bytes()[index] {
            b'}' => return Ok(index),
            b'\\' => {
                index += 2;
            }
            b'\'' => {
                index += 1;
                while index < source.len() && source.as_bytes()[index] != b'\'' {
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'"' => {
                index += 1;
                while index < source.len() && source.as_bytes()[index] != b'"' {
                    if source.as_bytes()[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'$' if source.as_bytes().get(index + 1) == Some(&b'{') => {
                index += 2;
                let inner = scan_to_closing_brace(source, index)?;
                index = inner + 1;
            }
            b'$' if source.as_bytes().get(index + 1) == Some(&b'(') => {
                if source.as_bytes().get(index + 2) == Some(&b'(') {
                    index += 3;
                    let mut depth = 1usize;
                    while index < source.len() {
                        if source.as_bytes()[index] == b'(' {
                            depth += 1;
                        } else if source.as_bytes()[index] == b')' {
                            if depth == 1 && source.as_bytes().get(index + 1) == Some(&b')') {
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
                        if source.as_bytes()[index] == b'(' {
                            depth += 1;
                        } else if source.as_bytes()[index] == b')' {
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
                while index < source.len() && source.as_bytes()[index] != b'`' {
                    if source.as_bytes()[index] == b'\\' {
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
        message: "unterminated parameter expansion".into(),
    })
}

fn control_escape(ch: char) -> char {
    match ch {
        '\\' => '\u{001c}',
        '?' => '\u{007f}',
        other => char::from((other as u8) & 0x1f),
    }
}

fn parse_variable_base_escape(source: &str, base: u32, max_digits: usize) -> (u8, usize) {
    let mut consumed = 0usize;
    while consumed < max_digits
        && consumed < source.len()
        && (source.as_bytes()[consumed] as char).is_digit(base)
    {
        consumed += 1;
    }
    if consumed == 0 {
        return (0, 0);
    }
    (
        u8::from_str_radix(&source[..consumed], base).unwrap_or_default(),
        consumed,
    )
}

fn expand_braced_parameter<C: Context>(
    ctx: &mut C,
    expr: &str,
    quoted: bool,
) -> Result<Expansion, ExpandError> {
    if expr == "#" {
        return Ok(Expansion::One(
            lookup_param(ctx, "#")
                .map(|c| c.into_owned())
                .unwrap_or_default(),
        ));
    }
    if let Some(name) = expr.strip_prefix('#') {
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(Expansion::One(value.chars().count().to_string()));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    match op {
        None => Ok(Expansion::One(require_set_parameter(ctx, name, value)?)),
        Some(":-") => {
            if !is_set || is_null {
                expand_parameter_word_as_expansion(ctx, word.unwrap_or_default(), quoted)
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some("-") => {
            if !is_set {
                expand_parameter_word_as_expansion(ctx, word.unwrap_or_default(), quoted)
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some(":=") => {
            if !is_set || is_null {
                let val = assign_parameter(ctx, name, word.unwrap_or_default(), quoted)?;
                Ok(Expansion::One(val))
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some("=") => {
            if !is_set {
                let val = assign_parameter(ctx, name, word.unwrap_or_default(), quoted)?;
                Ok(Expansion::One(val))
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some(":?") => {
            if !is_set || is_null {
                let default_msg = format!("{name}: parameter null or not set");
                let raw = match word {
                    Some(w) if !w.is_empty() => w,
                    _ => &default_msg,
                };
                let message = expand_parameter_word(ctx, raw, quoted)?;
                Err(ExpandError {
                    message: message.into(),
                })
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some("?") => {
            if !is_set {
                let default_msg = format!("{name}: parameter not set");
                let raw = match word {
                    Some(w) if !w.is_empty() => w,
                    _ => &default_msg,
                };
                let message = expand_parameter_word(ctx, raw, quoted)?;
                Err(ExpandError {
                    message: message.into(),
                })
            } else {
                Ok(Expansion::One(
                    value.map(|c| c.into_owned()).unwrap_or_default(),
                ))
            }
        }
        Some(":+") => {
            if is_set && !is_null {
                expand_parameter_word_as_expansion(ctx, word.unwrap_or_default(), quoted)
            } else {
                Ok(Expansion::One(String::new()))
            }
        }
        Some("+") => {
            if is_set {
                expand_parameter_word_as_expansion(ctx, word.unwrap_or_default(), quoted)
            } else {
                Ok(Expansion::One(String::new()))
            }
        }
        Some("%" | "%%" | "#" | "##") => {
            let val = require_set_parameter(ctx, name, value)?;
            let pat = expand_parameter_pattern_word(ctx, word.unwrap_or_default())?;
            let mode = match op.unwrap() {
                "%" => PatternRemoval::SmallestSuffix,
                "%%" => PatternRemoval::LargestSuffix,
                "#" => PatternRemoval::SmallestPrefix,
                _ => PatternRemoval::LargestPrefix,
            };
            Ok(Expansion::One(remove_parameter_pattern(val, &pat, mode)?))
        }
        Some(_) => Err(ExpandError {
            message: "unsupported parameter expansion".into(),
        }),
    }
}

fn expand_braced_parameter_text<C: Context>(
    ctx: &mut C,
    expr: &str,
) -> Result<String, ExpandError> {
    if expr == "#" {
        return Ok(lookup_param(ctx, "#")
            .map(|c| c.into_owned())
            .unwrap_or_default());
    }
    if let Some(name) = expr.strip_prefix('#') {
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(value.chars().count().to_string());
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    match op {
        None => require_set_parameter(ctx, name, value),
        Some(":-") => {
            if !is_set || is_null {
                expand_parameter_text_owned(ctx, word.unwrap_or_default())
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some("-") => {
            if !is_set {
                expand_parameter_text_owned(ctx, word.unwrap_or_default())
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some(":=") => {
            if !is_set || is_null {
                assign_parameter_text(ctx, name, word.unwrap_or_default())
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some("=") => {
            if !is_set {
                assign_parameter_text(ctx, name, word.unwrap_or_default())
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some(":?") => {
            if !is_set || is_null {
                let message =
                    expand_parameter_error_text(ctx, name, word, "parameter null or not set")?;
                Err(ExpandError {
                    message: message.into(),
                })
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some("?") => {
            if !is_set {
                let message = expand_parameter_error_text(ctx, name, word, "parameter not set")?;
                Err(ExpandError {
                    message: message.into(),
                })
            } else {
                Ok(value.map(|c| c.into_owned()).unwrap_or_default())
            }
        }
        Some(":+") => {
            if is_set && !is_null {
                expand_parameter_text_owned(ctx, word.unwrap_or_default())
            } else {
                Ok(String::new())
            }
        }
        Some("+") => {
            if is_set {
                expand_parameter_text_owned(ctx, word.unwrap_or_default())
            } else {
                Ok(String::new())
            }
        }
        Some("%") => remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, word.unwrap_or_default())?,
            PatternRemoval::SmallestSuffix,
        ),
        Some("%%") => remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, word.unwrap_or_default())?,
            PatternRemoval::LargestSuffix,
        ),
        Some("#") => remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, word.unwrap_or_default())?,
            PatternRemoval::SmallestPrefix,
        ),
        Some("##") => remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, word.unwrap_or_default())?,
            PatternRemoval::LargestPrefix,
        ),
        Some(_) => Err(ExpandError {
            message: "unsupported parameter expansion".into(),
        }),
    }
}

fn assign_parameter<C: Context>(
    ctx: &mut C,
    name: &str,
    raw_word: &str,
    quoted: bool,
) -> Result<String, ExpandError> {
    if !is_name(name) {
        return Err(ExpandError {
            message: format!("{name}: cannot assign in parameter expansion").into(),
        });
    }
    let value = expand_parameter_word(ctx, raw_word, quoted)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn assign_parameter_text<C: Context>(
    ctx: &mut C,
    name: &str,
    raw_word: &str,
) -> Result<String, ExpandError> {
    if !is_name(name) {
        return Err(ExpandError {
            message: format!("{name}: cannot assign in parameter expansion").into(),
        });
    }
    let value = expand_parameter_text_owned(ctx, raw_word)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn expand_parameter_error_text<C: Context>(
    ctx: &mut C,
    name: &str,
    word: Option<&str>,
    default_message: &str,
) -> Result<String, ExpandError> {
    let owned;
    let raw = match word {
        Some(w) if !w.is_empty() => w,
        _ => {
            owned = format!("{name}: {default_message}");
            &owned
        }
    };
    expand_parameter_text_owned(ctx, raw)
}

fn expand_parameter_word<C: Context>(
    ctx: &mut C,
    raw: &str,
    _quoted: bool,
) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_segments(&expanded.segments))
}

fn expand_parameter_word_as_expansion<C: Context>(
    ctx: &mut C,
    raw: &str,
    _quoted: bool,
) -> Result<Expansion, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    let has_at = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak | Segment::AtEmpty));
    if has_at {
        let mut fields = Vec::new();
        let mut current = String::new();
        for seg in &expanded.segments {
            match seg {
                Segment::Text(s, _) => current.push_str(s),
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
    raw: &str,
) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
}

fn parse_parameter_expression(
    expr: &str,
) -> Result<(&str, Option<&str>, Option<&str>), ExpandError> {
    if expr.is_empty() {
        return Err(ExpandError {
            message: "empty parameter expansion".into(),
        });
    }
    let mut index = 0usize;
    let b0 = expr.as_bytes()[0];
    let name: &str = if b0.is_ascii_digit() {
        while index < expr.len() && expr.as_bytes()[index].is_ascii_digit() {
            index += 1;
        }
        &expr[..index]
    } else if matches!(b0, b'?' | b'$' | b'!' | b'#' | b'*' | b'@') {
        index = 1;
        &expr[..index]
    } else if b0 == b'_' || b0.is_ascii_alphabetic() {
        while index < expr.len()
            && (expr.as_bytes()[index] == b'_' || expr.as_bytes()[index].is_ascii_alphanumeric())
        {
            index += 1;
        }
        &expr[..index]
    } else {
        return Err(ExpandError {
            message: "invalid parameter expansion".into(),
        });
    };

    if index == expr.len() {
        return Ok((name, None, None));
    }

    let rest = &expr[index..];
    let bytes = rest.as_bytes();
    let (op, word) = match bytes[0] {
        b':' if bytes.len() > 1 => match bytes[1] {
            b'-' => (":-", &rest[2..]),
            b'=' => (":=", &rest[2..]),
            b'?' => (":?", &rest[2..]),
            b'+' => (":+", &rest[2..]),
            _ => (&rest[..1], &rest[1..]),
        },
        b'%' if bytes.len() > 1 && bytes[1] == b'%' => ("%%", &rest[2..]),
        b'#' if bytes.len() > 1 && bytes[1] == b'#' => ("##", &rest[2..]),
        b'-' => ("-", &rest[1..]),
        b'=' => ("=", &rest[1..]),
        b'?' => ("?", &rest[1..]),
        b'+' => ("+", &rest[1..]),
        b'%' => ("%", &rest[1..]),
        b'#' => ("#", &rest[1..]),
        _ => (&rest[..1], &rest[1..]),
    };
    Ok((name, Some(op), Some(word)))
}

fn lookup_param<'a, C: Context>(ctx: &'a C, name: &str) -> Option<Cow<'a, str>> {
    if name == "0" {
        return Some(Cow::Borrowed(ctx.shell_name()));
    }
    if !name.is_empty() && name.as_bytes().iter().all(|b| b.is_ascii_digit()) {
        return name
            .parse::<usize>()
            .ok()
            .and_then(|index| ctx.positional_param(index));
    }
    let mut chars = name.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        if let Some(value) = ctx.special_param(ch) {
            return Some(value);
        }
    }
    ctx.env_var(name)
}

fn require_set_parameter<C: Context>(
    ctx: &C,
    name: &str,
    value: Option<Cow<'_, str>>,
) -> Result<String, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != "@" && name != "*" {
        return Err(ExpandError {
            message: format!("{name}: parameter not set").into(),
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
    text: String,
    has_unquoted_glob: bool,
}

fn segment_chars(segments: &[Segment]) -> impl Iterator<Item = (char, QuoteState)> + '_ {
    segments
        .iter()
        .flat_map(|seg| match seg {
            Segment::Text(text, state) => {
                let s = *state;
                Some(text.chars().map(move |ch| (ch, s)))
            }
            _ => None,
        })
        .flatten()
}

fn split_fields_from_segments(segments: &[Segment], ifs: &str) -> Vec<Field> {
    if ifs.is_empty() {
        return vec![Field {
            text: flatten_segments(segments),
            has_unquoted_glob: segments.iter().any(|seg| {
                matches!(seg, Segment::Text(text, state) if *state != QuoteState::Quoted && text.chars().any(is_glob_char))
            }),
        }];
    }

    let ifs_ws: Vec<char> = ifs
        .chars()
        .filter(|ch| matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let ifs_other: Vec<char> = ifs
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let chars: Vec<(char, QuoteState)> = segment_chars(segments).collect();

    let mut fields = Vec::new();
    let mut current = String::new();
    let mut current_glob = false;
    let mut index = 0usize;

    while index < chars.len() {
        let (ch, state) = chars[index];
        let splittable = state == QuoteState::Expanded;
        if splittable && ifs_other.contains(&ch) {
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
        if splittable && ifs_ws.contains(&ch) {
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
        current_glob |= state != QuoteState::Quoted && is_glob_char(ch);
        current.push(ch);
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

fn push_segment(segments: &mut Vec<Segment>, text: String, state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.push_str(&text);
            return;
        }
    }
    segments.push(Segment::Text(text, state));
}

fn push_segment_str(segments: &mut Vec<Segment>, text: &str, state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.push_str(text);
            return;
        }
    }
    segments.push(Segment::Text(text.to_string(), state));
}

fn flatten_segments(segments: &[Segment]) -> String {
    let mut result = String::new();
    for seg in segments {
        if let Segment::Text(part, _) = seg {
            result.push_str(part);
        }
    }
    result
}

fn render_pattern_from_segments(segments: &[Segment]) -> String {
    let mut pattern = String::new();
    for seg in segments {
        if let Segment::Text(text, state) = seg {
            if *state == QuoteState::Quoted {
                for ch in text.chars() {
                    pattern.push('\\');
                    pattern.push(ch);
                }
            } else {
                pattern.push_str(text);
            }
        }
    }
    pattern
}

fn is_glob_char(ch: char) -> bool {
    matches!(ch, '*' | '?' | '[')
}

#[derive(Clone, Copy)]
enum PatternRemoval {
    SmallestSuffix,
    LargestSuffix,
    SmallestPrefix,
    LargestPrefix,
}

fn remove_parameter_pattern(
    value: String,
    pattern: &str,
    mode: PatternRemoval,
) -> Result<String, ExpandError> {
    let boundaries: Vec<usize> = {
        let mut v = Vec::new();
        let mut i = 0;
        while i < value.len() {
            v.push(i);
            i += char_len(&value, i);
        }
        v.push(value.len());
        v
    };
    match mode {
        PatternRemoval::SmallestPrefix => {
            for &end in &boundaries {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_string());
                }
            }
        }
        PatternRemoval::LargestPrefix => {
            for &end in boundaries.iter().rev() {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_string());
                }
            }
        }
        PatternRemoval::SmallestSuffix => {
            for &start in boundaries.iter().rev() {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_string());
                }
            }
        }
        PatternRemoval::LargestSuffix => {
            for &start in &boundaries {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_string());
                }
            }
        }
    }
    Ok(value)
}

fn expand_pathname(pattern: &str) -> Vec<String> {
    if !pattern.chars().any(is_glob_char) {
        return vec![pattern.to_string()];
    }
    let absolute = pattern.starts_with('/');
    let segments: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let base = if absolute {
        PathBuf::from("/")
    } else {
        PathBuf::from(".")
    };
    let mut matches = Vec::new();
    expand_path_segments(&base, &segments, 0, absolute, &mut matches);
    matches.sort();
    matches
}

fn expand_path_segments(
    base: &Path,
    segments: &[&str],
    index: usize,
    absolute: bool,
    matches: &mut Vec<String>,
) {
    if index == segments.len() {
        let text = if absolute {
            base.display().to_string()
        } else {
            base.strip_prefix(".").unwrap_or(base).display().to_string()
        };
        matches.push(if text.is_empty() {
            ".".to_string()
        } else {
            text
        });
        return;
    }

    let segment = segments[index];
    if !segment.chars().any(is_glob_char) {
        let next = base.join(segment);
        if sys::file_exists(&next.display().to_string()) {
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
        return;
    }

    let Ok(mut names) = sys::read_dir_entries(&base.display().to_string()) else {
        return;
    };
    names.sort();
    for name in names {
        if name.starts_with('.') && !segment.starts_with('.') {
            continue;
        }
        if pattern_matches(&name, segment) {
            expand_path_segments(&base.join(&name), segments, index + 1, absolute, matches);
        }
    }
}

pub fn pattern_matches(text: &str, pattern: &str) -> bool {
    pattern_matches_inner(text, 0, pattern, 0)
}

fn pattern_matches_inner(text: &str, ti: usize, pattern: &str, pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    let pc = char_at(pattern, pi);
    let pclen = char_len(pattern, pi);
    match pc {
        '*' => {
            let mut pos = ti;
            loop {
                if pattern_matches_inner(text, pos, pattern, pi + 1) {
                    return true;
                }
                if pos == text.len() {
                    break;
                }
                pos += char_len(text, pos);
            }
            false
        }
        '?' => {
            ti < text.len() && pattern_matches_inner(text, ti + char_len(text, ti), pattern, pi + 1)
        }
        '[' => {
            let tc = if ti < text.len() {
                Some(char_at(text, ti))
            } else {
                None
            };
            match match_bracket(tc, pattern, pi) {
                Some((matched, next_pi)) => {
                    matched
                        && ti < text.len()
                        && pattern_matches_inner(text, ti + char_len(text, ti), pattern, next_pi)
                }
                None => {
                    ti < text.len()
                        && char_at(text, ti) == '['
                        && pattern_matches_inner(text, ti + char_len(text, ti), pattern, pi + 1)
                }
            }
        }
        '\\' if pi + pclen < pattern.len() => {
            let escaped = char_at(pattern, pi + pclen);
            let eclen = char_len(pattern, pi + pclen);
            ti < text.len()
                && char_at(text, ti) == escaped
                && pattern_matches_inner(text, ti + char_len(text, ti), pattern, pi + pclen + eclen)
        }
        ch => {
            ti < text.len()
                && char_at(text, ti) == ch
                && pattern_matches_inner(text, ti + char_len(text, ti), pattern, pi + pclen)
        }
    }
}

fn match_charclass(class: &str, ch: char) -> bool {
    crate::sys::classify_char(class, ch)
}

fn match_bracket(current: Option<char>, pattern: &str, start: usize) -> Option<(bool, usize)> {
    let current = current?;
    let mut index = start + 1;
    if index >= pattern.len() {
        return None;
    }

    let mut negate = false;
    if index < pattern.len() && matches!(pattern.as_bytes()[index], b'!' | b'^') {
        negate = true;
        index += 1;
    }

    let mut matched = false;
    let mut saw_closer = false;
    let mut first_elem = true;
    while index < pattern.len() {
        let pc = char_at(pattern, index);
        if pc == ']' && !first_elem {
            saw_closer = true;
            index += 1;
            break;
        }

        first_elem = false;

        if pc == '[' && index + 1 < pattern.len() && pattern.as_bytes()[index + 1] == b':' {
            let class_start = index + 2;
            let mut found_end = None;
            let mut ci = class_start;
            while ci + 1 < pattern.len() {
                if pattern.as_bytes()[ci] == b':' && pattern.as_bytes()[ci + 1] == b']' {
                    found_end = Some(ci);
                    break;
                }
                ci += 1;
            }
            if let Some(end) = found_end {
                let class_name = pattern[class_start..end].to_string();
                matched |= match_charclass(&class_name, current);
                index = end + 2;
                continue;
            }
        }

        let first = if pc == '\\' && index + 1 < pattern.len() {
            index += 1;
            char_at(pattern, index)
        } else {
            pc
        };
        let first_clen = char_len(pattern, index);
        if index + first_clen + 1 < pattern.len()
            && pattern.as_bytes()[index + first_clen] == b'-'
            && char_at(pattern, index + first_clen + 1) != ']'
        {
            let last = char_at(pattern, index + first_clen + 1);
            let last_clen = char_len(pattern, index + first_clen + 1);
            matched |= first <= current && current <= last;
            index += first_clen + 1 + last_clen;
        } else {
            matched |= current == first;
            index += first_clen;
        }
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn expand_arithmetic_expression<C: Context>(
    ctx: &mut C,
    expression: &str,
) -> Result<String, ExpandError> {
    let mut result = String::new();
    let mut i = 0;
    while i < expression.len() {
        if expression.as_bytes()[i] == b'$' {
            let (expansion, consumed) = expand_dollar(ctx, &expression[i..], true)?;
            match expansion {
                Expansion::One(s) => result.push_str(&s),
                Expansion::AtFields(fields) => {
                    result.push_str(&fields.join(" "));
                }
            }
            i += consumed;
        } else if expression.as_bytes()[i] == b'`' {
            i += 1;
            let command = scan_backtick_command(expression, &mut i, true)?;
            let output = ctx.command_substitute(&command)?;
            result.push_str(output.trim_end_matches('\n'));
        } else {
            let clen = char_len(expression, i);
            result.push_str(&expression[i..i + clen]);
            i += clen;
        }
    }
    Ok(result)
}

fn eval_arithmetic<C: Context>(ctx: &mut C, expression: &str) -> Result<i64, ExpandError> {
    let mut parser = ArithmeticParser::new(ctx, expression);
    let value = parser.parse_assignment()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ExpandError {
            message: "unexpected trailing arithmetic tokens".into(),
        });
    }
    Ok(value)
}

struct ArithmeticParser<'a, 'src, C> {
    source: &'src str,
    index: usize,
    ctx: &'a mut C,
}

fn arith_err(msg: &str) -> ExpandError {
    ExpandError {
        message: msg.into(),
    }
}

impl<'a, 'src, C: Context> ArithmeticParser<'a, 'src, C> {
    fn new(ctx: &'a mut C, raw: &'src str) -> Self {
        Self {
            source: raw,
            index: 0,
            ctx,
        }
    }

    fn parse_assignment(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let save = self.index;
        if let Some(name) = self.try_scan_name() {
            self.skip_ws();
            if let Some(op) = self.try_consume_assign_op() {
                let rhs = self.parse_assignment()?;
                let value = if op == "=" {
                    rhs
                } else {
                    let lhs = self.resolve_var(&name)?;
                    apply_compound_assign(&op, lhs, rhs)?
                };
                self.ctx
                    .set_var(&name, value.to_string())
                    .map_err(|e| arith_err(&e.message))?;
                return Ok(value);
            }
            self.index = save;
        }
        self.parse_ternary()
    }

    fn try_consume_assign_op(&mut self) -> Option<String> {
        let remaining = &self.source[self.index..];
        for op in &[
            "<<=", ">>=", "&=", "^=", "|=", "*=", "/=", "%=", "+=", "-=", "=",
        ] {
            if remaining.starts_with(op) {
                if *op == "=" && remaining.starts_with("==") {
                    return None;
                }
                self.index += op.len();
                return Some(op.to_string());
            }
        }
        None
    }

    fn parse_ternary(&mut self) -> Result<i64, ExpandError> {
        let cond = self.parse_logical_or()?;
        self.skip_ws();
        if self.consume('?') {
            let then_val = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(':') {
                return Err(arith_err("expected ':' in ternary expression"));
            }
            let else_val = self.parse_assignment()?;
            Ok(if cond != 0 { then_val } else { else_val })
        } else {
            Ok(cond)
        }
    }

    fn parse_logical_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_logical_and()?;
        loop {
            self.skip_ws();
            if self.consume_str("||") {
                let rhs = self.parse_logical_and()?;
                value = i64::from(value != 0 || rhs != 0);
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
            if self.consume_str("&&") {
                let rhs = self.parse_bitwise_or()?;
                value = i64::from(value != 0 && rhs != 0);
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
            if self.peek() == Some('|')
                && self.peek_at(1) != Some('|')
                && self.peek_at(1) != Some('=')
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
            if self.peek() == Some('^') && self.peek_at(1) != Some('=') {
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
            if self.peek() == Some('&')
                && self.peek_at(1) != Some('&')
                && self.peek_at(1) != Some('=')
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
            if self.consume_str("==") {
                value = i64::from(value == self.parse_relational()?);
            } else if self.consume_str("!=") {
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
            if self.consume_str("<=") {
                value = i64::from(value <= self.parse_shift()?);
            } else if self.consume_str(">=") {
                value = i64::from(value >= self.parse_shift()?);
            } else if self.peek() == Some('<') && self.peek_at(1) != Some('<') {
                self.index += 1;
                value = i64::from(value < self.parse_shift()?);
            } else if self.peek() == Some('>') && self.peek_at(1) != Some('>') {
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
            if self.peek() == Some('<')
                && self.peek_at(1) == Some('<')
                && self.peek_at(2) != Some('=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shl(rhs as u32);
            } else if self.peek() == Some('>')
                && self.peek_at(1) == Some('>')
                && self.peek_at(2) != Some('=')
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
            if self.peek() == Some('+') && self.peek_at(1) != Some('=') {
                self.index += 1;
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.peek() == Some('-') && self.peek_at(1) != Some('=') {
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
            if self.peek() == Some('*') && self.peek_at(1) != Some('=') {
                self.index += 1;
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.peek() == Some('/') && self.peek_at(1) != Some('=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(arith_err("division by zero"));
                }
                value /= rhs;
            } else if self.peek() == Some('%') && self.peek_at(1) != Some('=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(arith_err("division by zero"));
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
        if self.consume('+') {
            return self.parse_unary();
        }
        if self.consume('-') {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.consume('~') {
            return Ok(!self.parse_unary()?);
        }
        if self.peek() == Some('!') && self.peek_at(1) != Some('=') {
            self.index += 1;
            return Ok(i64::from(self.parse_unary()? == 0));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume('(') {
            let value = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(')') {
                return Err(arith_err("missing ')'"));
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
        if self.peek() == Some('0') {
            self.index += 1;
            if self.peek() == Some('x') || self.peek() == Some('X') {
                self.index += 1;
                let hex_start = self.index;
                while self.index < self.source.len()
                    && self.source.as_bytes()[self.index].is_ascii_hexdigit()
                {
                    self.index += 1;
                }
                if self.index == hex_start {
                    return Err(arith_err("invalid hex constant"));
                }
                return i64::from_str_radix(&self.source[hex_start..self.index], 16)
                    .map_err(|_| arith_err("invalid hex constant"));
            }
            if self.peek().map_or(false, |c| c.is_ascii_digit()) {
                while self.index < self.source.len()
                    && self.source.as_bytes()[self.index].is_ascii_digit()
                {
                    self.index += 1;
                }
                return i64::from_str_radix(&self.source[start + 1..self.index], 8)
                    .map_err(|_| arith_err("invalid octal constant"));
            }
            return Ok(0);
        }

        while self.index < self.source.len() && self.source.as_bytes()[self.index].is_ascii_digit()
        {
            self.index += 1;
        }
        if start == self.index {
            return Err(arith_err("expected arithmetic operand"));
        }
        self.source[start..self.index]
            .parse::<i64>()
            .map_err(|_| arith_err("invalid arithmetic operand"))
    }

    fn try_scan_name(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.index;
        if self.index < self.source.len() {
            let b = self.source.as_bytes()[self.index];
            if b.is_ascii_alphabetic() || b == b'_' {
                self.index += 1;
                while self.index < self.source.len() {
                    let b2 = self.source.as_bytes()[self.index];
                    if b2.is_ascii_alphanumeric() || b2 == b'_' {
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                return Some(self.source[start..self.index].to_string());
            }
        }
        None
    }

    fn resolve_var(&mut self, name: &str) -> Result<i64, ExpandError> {
        let val_str = self.ctx.env_var(name).unwrap_or_default();
        if val_str.is_empty() {
            return Ok(0);
        }
        let trimmed = val_str.trim();
        if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            i64::from_str_radix(&trimmed[2..], 16)
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        } else if trimmed.starts_with('0')
            && trimmed.len() > 1
            && trimmed[1..].chars().all(|c| c.is_ascii_digit())
        {
            i64::from_str_radix(&trimmed[1..], 8)
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        } else {
            trimmed
                .parse::<i64>()
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        }
    }

    fn skip_ws(&mut self) {
        while self.index < self.source.len()
            && self.source.as_bytes()[self.index].is_ascii_whitespace()
        {
            self.index += 1;
        }
    }

    fn consume(&mut self, ch: char) -> bool {
        if self.source.as_bytes().get(self.index) == Some(&(ch as u8)) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, s: &str) -> bool {
        if self.source[self.index..].starts_with(s) {
            self.index += s.len();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.as_bytes().get(self.index).map(|&b| b as char)
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.source
            .as_bytes()
            .get(self.index + offset)
            .map(|&b| b as char)
    }

    fn is_eof(&self) -> bool {
        self.index >= self.source.len()
    }
}

fn apply_compound_assign(op: &str, lhs: i64, rhs: i64) -> Result<i64, ExpandError> {
    match op {
        "+=" => Ok(lhs.wrapping_add(rhs)),
        "-=" => Ok(lhs.wrapping_sub(rhs)),
        "*=" => Ok(lhs.wrapping_mul(rhs)),
        "/=" => {
            if rhs == 0 {
                return Err(arith_err("division by zero"));
            }
            Ok(lhs / rhs)
        }
        "%=" => {
            if rhs == 0 {
                return Err(arith_err("division by zero"));
            }
            Ok(lhs % rhs)
        }
        "<<=" => Ok(lhs.wrapping_shl(rhs as u32)),
        ">>=" => Ok(lhs.wrapping_shr(rhs as u32)),
        "&=" => Ok(lhs & rhs),
        "^=" => Ok(lhs ^ rhs),
        "|=" => Ok(lhs | rhs),
        _ => Err(arith_err(&format!("unknown assignment operator '{op}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use std::collections::HashMap;

    struct FakeContext {
        env: HashMap<String, String>,
        positional: Vec<String>,
        pathname_expansion_enabled: bool,
        nounset_enabled: bool,
    }

    impl FakeContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert("HOME".into(), "/tmp/home".into());
            env.insert("USER".into(), "meiksh".into());
            env.insert("IFS".into(), " \t\n,".into());
            env.insert("WORDS".into(), "one,two three".into());
            env.insert("DELIMS".into(), ",,,".into());
            env.insert("EMPTY".into(), String::new());
            env.insert("X".into(), "fallback".into());
            Self {
                env,
                positional: vec!["alpha".into(), "beta".into()],
                pathname_expansion_enabled: true,
                nounset_enabled: false,
            }
        }
    }

    impl Context for FakeContext {
        fn env_var(&self, name: &str) -> Option<Cow<'_, str>> {
            self.env.get(name).map(|v| Cow::Borrowed(v.as_str()))
        }

        fn special_param(&self, name: char) -> Option<Cow<'_, str>> {
            match name {
                '?' => Some(Cow::Owned("0".to_string())),
                '#' => Some(Cow::Owned(self.positional.len().to_string())),
                '-' => Some(Cow::Owned("aC".to_string())),
                '*' | '@' => Some(Cow::Owned(self.positional.join(" "))),
                _ => None,
            }
        }

        fn positional_param(&self, index: usize) -> Option<Cow<'_, str>> {
            if index == 0 {
                Some(Cow::Owned("meiksh".to_string()))
            } else {
                self.positional
                    .get(index - 1)
                    .map(|v| Cow::Borrowed(v.as_str()))
            }
        }

        fn positional_params(&self) -> &[String] {
            &self.positional
        }

        fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
            self.env.insert(name.to_string(), value);
            Ok(())
        }

        fn pathname_expansion_enabled(&self) -> bool {
            self.pathname_expansion_enabled
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &str {
            "meiksh"
        }

        fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError> {
            Ok(format!("{command}\n"))
        }

        fn home_dir_for_user(&self, name: &str) -> Option<Cow<'_, str>> {
            match name {
                "testuser" => Some(Cow::Owned("/home/testuser".to_string())),
                _ => None,
            }
        }
    }

    #[test]
    fn expands_home_and_params() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~/$USER".into(),
            },
            &arena,
        )
        .expect("expand");
        assert_eq!(fields, vec!["/tmp/home/meiksh".to_string()]);
    }

    #[test]
    fn expands_arithmetic_expressions() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 + 2 * 3))".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["7".to_string()]
        );
    }

    #[test]
    fn expands_command_substitution() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_words(
                &mut ctx,
                &[
                    Word {
                        raw: "$WORDS".into()
                    },
                    Word {
                        raw: "$(printf hi)".into()
                    },
                ],
                &arena,
            )
            .expect("expand"),
            vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
                "printf".to_string(),
                "hi".to_string(),
            ]
        );
    }

    #[test]
    fn preserves_quoted_and_escaped_characters() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$0 $1\"".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["meiksh alpha".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\\$HOME".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["$HOME".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "a\\ b".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "'literal text'".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["literal text".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"cost:\\$USER\"".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["cost:$USER".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$'a b'".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$'line\\nnext'".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["line\nnext".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$'a b'\"".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["$'a b'".to_string()]
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "$'tab\\tstop'", &arena).expect("parameter text"),
            "tab\tstop".to_string()
        );
    }

    #[test]
    fn rejects_unterminated_quotes_and_expansions() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        for raw in ["'oops", "\"oops", "${USER", "$(echo", "$((1 + 2)", "$'oops"] {
            let error = expand_word(&mut ctx, &Word { raw }, &arena).expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn dollar_single_quote_helpers_cover_escape_matrix() {
        let input = "$'\\\"\\'\\\\\\a\\b\\e\\f\\n\\r\\t\\v\\cA\\c\\\\\\x41\\101Z'";
        let (value, consumed) = parse_dollar_single_quoted(input).expect("parse");
        assert_eq!(consumed, input.len());
        assert_eq!(
            value,
            format!(
                "\"'\\{}\u{0008}\u{001b}\u{000c}\n\r\t\u{000b}\u{0001}\u{001c}AAZ",
                '\u{0007}'
            )
        );

        assert!(parse_dollar_single_quoted("$'\\").is_err());

        assert!(parse_dollar_single_quoted("$'\\c").is_err());

        let (value, _) = parse_dollar_single_quoted("$'\\xZ'").expect("parse no hex");
        assert_eq!(value, "xZ");

        let (value, _) = parse_dollar_single_quoted("$'\\x41'").expect("parse hex");
        assert_eq!(value, "A");

        let (value, _) = parse_dollar_single_quoted("$'\\z'").expect("parse unspecified");
        assert_eq!(value, "z");

        assert_eq!(control_escape('\\'), '\u{001c}');
        assert_eq!(control_escape('?'), '\u{007f}');
        assert_eq!(control_escape('A'), '\u{0001}');
        assert_eq!(parse_variable_base_escape("412", 16, 2), (0x41, 2));
        assert_eq!(parse_variable_base_escape("1017", 8, 3), (0o101, 3));
        assert_eq!(parse_variable_base_escape("Z", 16, 2), (0, 0));
    }

    #[test]
    fn rejects_bad_arithmetic() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        for raw in ["$((1 / 0))", "$((1 + ))", "$((1 1))"] {
            let error = expand_word(&mut ctx, &Word { raw }, &arena).expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn supports_parameter_operators_and_positionals() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "e".into(),
            "f".into(),
            "g".into(),
            "h".into(),
            "i".into(),
            "j".into(),
        ];

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${10}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["j".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$10".into() }, &arena).expect("expand"),
            vec!["a0".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${#10}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["1".to_string()]
        );
        ctx.env.insert("IFS".into(), ":".into());
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$*".into() }, &arena).expect("expand"),
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string(),
                "g".to_string(),
                "h".to_string(),
                "i".to_string(),
                "j".to_string()
            ]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["a:b:c:d:e:f:g:h:i:j".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET-word}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET:-word}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${EMPTY-word}".into()
                },
                &arena,
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${EMPTY:-word}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${USER:+alt}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["alt".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET+alt}".into()
                },
                &arena,
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${NEW:=value}".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["value".to_string()]
        );
        assert_eq!(ctx.env.get("NEW").map(String::as_str), Some("value"));
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${#}".into() }, &arena).expect("expand"),
            vec!["10".to_string()]
        );

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?boom}".into(),
            },
            &arena,
        )
        .expect_err("unset error");
        assert_eq!(&*error.message, "boom");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?$'unterminated}".into(),
            },
            &arena,
        )
        .expect_err("colon-question word expansion error");
        assert!(!error.message.is_empty());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${MISSING?$'unterminated}".into(),
            },
            &arena,
        )
        .expect_err("plain-question word expansion error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn performs_field_splitting_more_like_posix() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$WORDS".into()
                },
                &arena,
            )
            .expect("expand"),
            vec!["one".to_string(), "two".to_string(), "three".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$DELIMS".into()
                },
                &arena,
            )
            .expect("expand"),
            vec![String::new(), String::new(), String::new()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$EMPTY".into()
                },
                &arena,
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert!(split_fields_from_segments(&[], " \t\n").is_empty());
    }

    #[test]
    fn expands_text_without_field_splitting_or_pathname_expansion() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("WORDS".into(), "one two".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "$WORDS".into()
                },
                &arena,
            )
            .expect("expand"),
            "one two"
        );
        assert_eq!(
            expand_word_text(&mut ctx, &Word { raw: "*".into() }, &arena).expect("expand"),
            "*"
        );
    }

    #[test]
    fn performs_pathname_expansion() {
        let arena = StringArena::new();
        let dir_entries = || {
            vec![
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry("a.txt".into()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry("b.txt".into()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry(".hidden.txt".into()),
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
                        raw: "/testdir/*.txt".into()
                    },
                    &arena,
                )
                .expect("glob"),
                vec!["/testdir/a.txt".to_string(), "/testdir/b.txt".to_string()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "\\*.txt".into()
                    },
                    &arena,
                )
                .expect("escaped glob"),
                vec!["*.txt".to_string()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "/testdir/.*.txt".into()
                    },
                    &arena,
                )
                .expect("hidden glob"),
                vec!["/testdir/.hidden.txt".to_string()]
            );
        });
    }

    #[test]
    fn can_disable_pathname_expansion_via_context() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let pattern = "/testdir/*.txt";
            assert_eq!(
                expand_word(&mut ctx, &Word { raw: pattern }, &arena,).expect("noglob"),
                vec![pattern]
            );
        });
    }

    #[test]
    fn helper_paths_cover_remaining_branches() {
        let ctx = FakeContext::new();
        assert_eq!(lookup_param(&ctx, "?").as_deref(), Some("0"));
        assert_eq!(lookup_param(&ctx, "0").as_deref(), Some("meiksh"));
        assert_eq!(lookup_param(&ctx, "X").as_deref(), Some("fallback"));
        assert_eq!(lookup_param(&ctx, "99"), None);
        assert_eq!(
            ctx.positional_params(),
            &["alpha".to_string(), "beta".to_string()][..]
        );
        assert_eq!(ctx.positional_param(0).as_deref(), Some("meiksh"));

        let mut segs = Vec::new();
        push_segment(&mut segs, "a".into(), QuoteState::Expanded);
        push_segment(&mut segs, String::new(), QuoteState::Expanded);
        push_segment(&mut segs, "b".into(), QuoteState::Expanded);
        push_segment(&mut segs, "c".into(), QuoteState::Quoted);
        assert_eq!(
            segs,
            vec![
                Segment::Text("ab".to_string(), QuoteState::Expanded),
                Segment::Text("c".to_string(), QuoteState::Quoted)
            ]
        );

        assert_eq!(flatten_segments(&segs), "abc".to_string());
        assert!(pattern_matches("beta", "b*"));
        assert!(!pattern_matches("beta", "a*"));
        let mut ctx2 = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx2, "42").expect("direct eval"), 42);
        assert!(eval_arithmetic(&mut ctx2, "(1 + 2").is_err());

        let arith_bt =
            expand_arithmetic_expression(&mut ctx2, "`printf 5`").expect("backtick in arith");
        assert_eq!(arith_bt, "printf 5");

        let mut parser = ArithmeticParser::new(&mut ctx2, "9");
        parser.index = 99;
        assert!(parser.is_eof());
    }

    #[test]
    fn nounset_option_rejects_plain_unset_parameter_expansions() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "$UNSET".into(),
            },
            &arena,
        )
        .expect_err("nounset variable");
        assert_eq!(&*error.message, "UNSET: parameter not set");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET}".into(),
            },
            &arena,
        )
        .expect_err("nounset braced");
        assert_eq!(&*error.message, "UNSET: parameter not set");

        let error = expand_word(&mut ctx, &Word { raw: "$9".into() }, &arena)
            .expect_err("nounset positional");
        assert_eq!(&*error.message, "9: parameter not set");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET-word}".into()
                },
                &arena,
            )
            .expect("default still works"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                },
                &arena,
            )
            .expect("star exempt"),
            vec!["alpha beta".to_string()]
        );
    }

    struct DefaultPathContext {
        env: HashMap<String, String>,
        nounset_enabled: bool,
    }

    impl DefaultPathContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert("HOME".into(), "/tmp/home".into());
            Self {
                env,
                nounset_enabled: false,
            }
        }
    }

    impl Context for DefaultPathContext {
        fn env_var(&self, name: &str) -> Option<Cow<'_, str>> {
            self.env.get(name).map(|v| Cow::Borrowed(v.as_str()))
        }

        fn special_param(&self, _name: char) -> Option<Cow<'_, str>> {
            None
        }

        fn positional_param(&self, index: usize) -> Option<Cow<'_, str>> {
            if index == 0 {
                Some(Cow::Owned("meiksh".to_string()))
            } else {
                None
            }
        }

        fn positional_params(&self) -> &[String] {
            &[]
        }

        fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
            self.env.insert(name.to_string(), value);
            Ok(())
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &str {
            "meiksh"
        }

        fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError> {
            Ok(format!("{command}\n"))
        }

        fn home_dir_for_user(&self, _name: &str) -> Option<Cow<'_, str>> {
            None
        }
    }

    fn expect_one(result: Result<(Expansion, usize), ExpandError>) -> (String, usize) {
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
            expect_one(expand_dollar(&mut ctx, "$", false)),
            ("$".to_string(), 1)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, "$-", false)),
            ("aC".to_string(), 2)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, "$$", false)),
            ("".to_string(), 2)
        );

        let (at_expansion, at_consumed) = expand_dollar(&mut ctx, "$@", true).expect("quoted at");
        assert_eq!(at_consumed, 2);
        let Expansion::AtFields(fields) = at_expansion else {
            panic!("expected AtFields for quoted $@")
        };
        assert_eq!(fields, vec!["alpha".to_string(), "beta".to_string()]);

        let arithmetic_input = "$((1 + (2 * 3)))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, arithmetic_input, false)),
            ("7".to_string(), arithmetic_input.len())
        );

        let command_input = "$(printf (hi))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, command_input, false)),
            ("printf (hi)".to_string(), command_input.len())
        );
    }

    #[test]
    fn parameter_text_expansion_avoids_command_substitution() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("EMPTY".into(), String::new());

        assert_eq!(
            expand_parameter_text(&mut ctx, "${HOME:-/fallback}/.shrc", &arena)
                .expect("parameter text"),
            "/tmp/home/.shrc"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "${EMPTY:-$HOME}/nested", &arena)
                .expect("nested default"),
            "/tmp/home/nested"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "$(printf nope)${HOME}", &arena)
                .expect("literal command"),
            "$(printf nope)/tmp/home"
        );
    }

    #[test]
    fn parameter_text_dollar_helpers_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$").expect("single"),
            ("$".to_string(), 1)
        );
        assert!(expand_parameter_dollar(&mut ctx, "${HOME").is_err());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$0").expect("zero"),
            ("meiksh".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$?").expect("special"),
            ("0".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$1").expect("positional"),
            ("alpha".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$HOME").expect("name"),
            ("/tmp/home".to_string(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$HOME+rest").expect("name stops at +"),
            ("/tmp/home".to_string(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, "$-").expect("dash"),
            ("aC".to_string(), 2)
        );
    }

    #[test]
    fn parameter_text_assignment_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "#").expect("hash"),
            "2"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "#HOME").expect("length"),
            "9"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME-word").expect("dash set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "UNSET-word").expect("dash unset"),
            "word"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:=value").expect("colon assign set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "UNSET:=value").expect("assign unset"),
            "value"
        );
        assert_eq!(ctx.env.get("UNSET").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING3=value").expect("assign equals unset"),
            "value"
        );
        assert_eq!(ctx.env.get("MISSING3").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME=value").expect("assign set"),
            "/tmp/home"
        );
        assert!(assign_parameter_text(&mut ctx, "1", "value").is_err());

        let err = expand_braced_parameter_text(&mut ctx, "MISSING4?").expect_err("? no word");
        assert_eq!(&*err.message, "MISSING4: parameter not set");
        let text =
            expand_parameter_error_text(&mut ctx, "X", Some(""), "my default").expect("empty word");
        assert_eq!(text, "X: my default");
    }

    #[test]
    fn nounset_option_rejects_length_and_pattern_expansions_of_unset_parameters() {
        let mut ctx = DefaultPathContext::new();
        ctx.nounset_enabled = true;

        let error = expand_braced_parameter_text(&mut ctx, "#UNSET").expect_err("nounset length");
        assert_eq!(&*error.message, "UNSET: parameter not set");

        let error =
            expand_braced_parameter_text(&mut ctx, "UNSET%.*").expect_err("nounset pattern");
        assert_eq!(&*error.message, "UNSET: parameter not set");
    }

    #[test]
    fn parameter_text_question_operator_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("EMPTY".into(), String::new());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:?boom").expect("colon question set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME?boom").expect("question set"),
            "/tmp/home"
        );
        let colon_question = expand_braced_parameter_text(&mut ctx, "EMPTY:?boom")
            .expect_err("colon question unset");
        assert_eq!(&*colon_question.message, "boom");
        let question =
            expand_braced_parameter_text(&mut ctx, "MISSING?boom").expect_err("question unset");
        assert_eq!(&*question.message, "boom");
        let colon_default =
            expand_braced_parameter_text(&mut ctx, "EMPTY:?").expect_err("colon default");
        assert_eq!(&*colon_default.message, "EMPTY: parameter null or not set");
        let question_default =
            expand_braced_parameter_text(&mut ctx, "MISSING?").expect_err("question default");
        assert_eq!(&*question_default.message, "MISSING: parameter not set");
    }

    #[test]
    fn parameter_text_question_propagates_word_expansion_error() {
        let mut ctx = FakeContext::new();
        let err = expand_braced_parameter_text(&mut ctx, "MISSING:?$'unterminated")
            .expect_err("colon-question text expansion error");
        assert!(!err.message.is_empty());
        let err = expand_braced_parameter_text(&mut ctx, "MISSING?$'unterminated")
            .expect_err("plain-question text expansion error");
        assert!(!err.message.is_empty());
    }

    #[test]
    fn parameter_text_plus_and_pattern_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta".into());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:+alt").expect("colon plus"),
            "alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING2:+alt").expect("colon plus unset"),
            ""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME+alt").expect("plus set"),
            "alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING2+alt").expect("plus unset"),
            ""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED%.*").expect("suffix"),
            "alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED%%.*").expect("largest suffix"),
            "alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED#*.").expect("prefix"),
            "beta"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED##*.").expect("largest prefix"),
            "beta"
        );
        assert!(expand_braced_parameter_text(&mut ctx, "HOME::word").is_err());
    }

    #[test]
    fn parameter_helpers_cover_more_edge_cases() {
        let mut ctx = FakeContext::new();

        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:-word", false).expect("default set"),
            Expansion::One("meiksh".into())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:=word", false).expect("assign set"),
            Expansion::One("meiksh".into())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "MISSING=value", false).expect("assign unset"),
            Expansion::One("value".into())
        );
        assert_eq!(ctx.env.get("MISSING").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER=value", false).expect("assign set"),
            Expansion::One("meiksh".into())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER?boom", false).expect("question set"),
            Expansion::One("meiksh".into())
        );
        let error =
            expand_braced_parameter(&mut ctx, "UNSET?boom", false).expect_err("question unset");
        assert_eq!(&*error.message, "boom");
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:?boom", false).expect("colon question set"),
            Expansion::One("meiksh".into())
        );

        let error = assign_parameter(&mut ctx, "1", "value", false).expect_err("invalid assign");
        assert_eq!(&*error.message, "1: cannot assign in parameter expansion");

        let parsed = parse_parameter_expression("@").expect("special name");
        assert_eq!(parsed, ("@", None, None));

        let error = parse_parameter_expression("").expect_err("empty expr");
        assert_eq!(&*error.message, "empty parameter expansion");

        let error = parse_parameter_expression("%oops").expect_err("invalid expr");
        assert_eq!(&*error.message, "invalid parameter expansion");
        assert_eq!(
            parse_parameter_expression("USER%%tail").expect("largest suffix"),
            ("USER", Some("%%"), Some("tail"))
        );
        assert_eq!(
            parse_parameter_expression("USER/tail").expect("unknown operator"),
            ("USER", Some("/"), Some("tail"))
        );

        let error =
            expand_braced_parameter(&mut ctx, "USER/tail", false).expect_err("unsupported expr");
        assert_eq!(&*error.message, "unsupported parameter expansion");
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
                let segs = vec![Segment::Text("*.txt".to_string(), QuoteState::Expanded)];
                assert_eq!(
                    split_fields_from_segments(&segs, ""),
                    vec![Field {
                        text: "*.txt".to_string(),
                        has_unquoted_glob: true,
                    }]
                );

                assert_eq!(
                    split_fields_from_segments(
                        &[Segment::Text(
                            "alpha,  beta".to_string(),
                            QuoteState::Expanded
                        )],
                        " ,"
                    ),
                    vec![
                        Field {
                            text: "alpha".to_string(),
                            has_unquoted_glob: false,
                        },
                        Field {
                            text: "beta".to_string(),
                            has_unquoted_glob: false,
                        },
                    ]
                );

                assert_eq!(expand_pathname("plain.txt"), vec!["plain.txt".to_string()]);

                let mut matches = Vec::new();
                expand_path_segments(
                    Path::new("/definitely/not/a/real/dir"),
                    &["*.txt"],
                    0,
                    false,
                    &mut matches,
                );
                assert!(matches.is_empty());

                let mut matches = Vec::new();
                expand_path_segments(Path::new("."), &[], 0, false, &mut matches);
                assert_eq!(matches, vec![".".to_string()]);

                assert!(pattern_matches("x", "?"));
                assert!(pattern_matches("[", "["));
                assert!(pattern_matches("]", r"\]"));
                assert!(pattern_matches("b", "[a-c]"));
                assert!(pattern_matches("d", "[!a-c]"));

                assert!(pattern_matches("a", "[[:alpha:]]"));
                assert!(pattern_matches("Z", "[[:alpha:]]"));
                assert!(!pattern_matches("5", "[[:alpha:]]"));
                assert!(pattern_matches("3", "[[:alnum:]]"));
                assert!(pattern_matches("z", "[[:alnum:]]"));
                assert!(!pattern_matches("!", "[[:alnum:]]"));
                assert!(pattern_matches(" ", "[[:blank:]]"));
                assert!(pattern_matches("\t", "[[:blank:]]"));
                assert!(!pattern_matches("a", "[[:blank:]]"));
                assert!(pattern_matches("\x01", "[[:cntrl:]]"));
                assert!(!pattern_matches("a", "[[:cntrl:]]"));
                assert!(pattern_matches("9", "[[:digit:]]"));
                assert!(!pattern_matches("a", "[[:digit:]]"));
                assert!(pattern_matches("!", "[[:graph:]]"));
                assert!(!pattern_matches(" ", "[[:graph:]]"));
                assert!(pattern_matches("a", "[[:lower:]]"));
                assert!(!pattern_matches("A", "[[:lower:]]"));
                assert!(pattern_matches(" ", "[[:print:]]"));
                assert!(pattern_matches("a", "[[:print:]]"));
                assert!(!pattern_matches("\x01", "[[:print:]]"));
                assert!(pattern_matches(".", "[[:punct:]]"));
                assert!(!pattern_matches("a", "[[:punct:]]"));
                assert!(pattern_matches("\n", "[[:space:]]"));
                assert!(!pattern_matches("a", "[[:space:]]"));
                assert!(pattern_matches("A", "[[:upper:]]"));
                assert!(!pattern_matches("a", "[[:upper:]]"));
                assert!(pattern_matches("f", "[[:xdigit:]]"));
                assert!(pattern_matches("F", "[[:xdigit:]]"));
                assert!(!pattern_matches("g", "[[:xdigit:]]"));
                assert!(!pattern_matches("a", "[[:bogus:]]"));
                assert!(pattern_matches("x", "[[:x]"));
                assert!(!pattern_matches("", "[a-z]"));

                assert_eq!(match_bracket(None, "[a]", 0), None);
                assert_eq!(match_bracket(Some('a'), "[", 0), None);
                assert_eq!(match_bracket(Some(']'), "[\\]]", 0), Some((true, 4)));
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        "*".to_string(),
                        QuoteState::Quoted
                    )]),
                    "\\*".to_string()
                );
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        "ab".to_string(),
                        QuoteState::Literal
                    )]),
                    "ab".to_string()
                );
                assert_eq!(
                    render_pattern_from_segments(&[
                        Segment::Text("x".to_string(), QuoteState::Literal),
                        Segment::AtBreak,
                        Segment::Text("y".to_string(), QuoteState::Expanded),
                    ]),
                    "xy".to_string()
                );
            },
        );
    }

    #[test]
    fn supports_pattern_removal_parameter_expansions() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("PATHNAME".into(), "src/bin/main.rs".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta.gamma".into());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME#*/}".into()
                },
                &arena,
            )
            .expect("small prefix"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME##*/}".into()
                },
                &arena,
            )
            .expect("large prefix"),
            vec!["main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME%/*}".into()
                },
                &arena,
            )
            .expect("small suffix"),
            vec!["src/bin".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME%%/*}".into()
                },
                &arena,
            )
            .expect("large suffix"),
            vec!["src".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME#\"src/\"}".into()
                },
                &arena,
            )
            .expect("quoted pattern"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED#*.}".into()
                },
                &arena,
            )
            .expect("wildcard prefix"),
            vec!["beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED##*.}".into()
                },
                &arena,
            )
            .expect("largest wildcard prefix"),
            vec!["gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%.*}".into()
                },
                &arena,
            )
            .expect("wildcard suffix"),
            vec!["alpha.beta".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%%.*}".into()
                },
                &arena,
            )
            .expect("largest wildcard suffix"),
            vec!["alpha".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED#\"*.\"}".into()
                },
                &arena,
            )
            .expect("quoted wildcard"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%}".into()
                },
                &arena,
            )
            .expect("empty suffix pattern"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${MISSING%%*.}".into()
                },
                &arena,
            )
            .expect("unset value"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn arithmetic_parser_covers_more_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx, "9 - 2 - 1").expect("subtract"), 6);
        assert_eq!(eval_arithmetic(&mut ctx, "8 / 2").expect("divide"), 4);
        assert_eq!(eval_arithmetic(&mut ctx, "9 % 4").expect("modulo"), 1);
        assert_eq!(eval_arithmetic(&mut ctx, "(1 + 2)").expect("parens"), 3);
        assert_eq!(eval_arithmetic(&mut ctx, "-5").expect("negation"), -5);

        let error = eval_arithmetic(&mut ctx, "5 % 0").expect_err("mod zero");
        assert_eq!(&*error.message, "division by zero");

        let error = eval_arithmetic(&mut ctx, "999999999999999999999999999999999999999")
            .expect_err("overflow");
        assert_eq!(&*error.message, "invalid arithmetic operand");
    }

    #[test]
    fn default_pathname_context_trait_impl() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(ctx.special_param('?'), None);
        assert_eq!(ctx.positional_param(0).as_deref(), Some("meiksh"));
        assert_eq!(ctx.positional_param(1), None);
        assert!(ctx.positional_params().is_empty());
        assert!(ctx.home_dir_for_user("nobody").is_none());
        assert!(!ctx.nounset_enabled());
        ctx.set_var("NAME", "value".to_string()).expect("set var");
        assert_eq!(ctx.env_var("NAME").as_deref(), Some("value"));
        assert_eq!(ctx.shell_name(), "meiksh");
        assert_eq!(
            ctx.command_substitute("printf ok").expect("substitute"),
            "printf ok\n"
        );
    }

    #[test]
    fn unmatched_glob_returns_pattern_literally() {
        let arena = StringArena::new();
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
                            raw: "*.definitely-no-match".into()
                        },
                        &arena,
                    )
                    .expect("unmatched glob"),
                    vec!["*.definitely-no-match".to_string()]
                );
            },
        );
    }

    #[test]
    fn bracket_helpers_cover_missing_closer() {
        assert_eq!(match_bracket(Some('a'), "[a", 0), None);
    }

    #[test]
    fn expands_here_documents_without_field_splitting() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let expanded =
            expand_here_document(&mut ctx, "hello $USER\n$(printf hi)\n$((1 + 2))\n", &arena)
                .expect("expand heredoc");
        assert_eq!(expanded, "hello meiksh\nprintf hi\n3\n");

        let escaped = expand_here_document(&mut ctx, "\\$USER\nline\\\ncontinued\n", &arena)
            .expect("expand heredoc");
        assert_eq!(escaped, "$USER\nlinecontinued\n");

        let trailing = expand_here_document(&mut ctx, "keep\\", &arena).expect("expand heredoc");
        assert_eq!(trailing, "keep\\");

        let literal = expand_here_document(&mut ctx, "\\x", &arena).expect("expand heredoc");
        assert_eq!(literal, "\\x");

        let double_backslash = expand_here_document(&mut ctx, "a\\\\b\n", &arena)
            .expect("expand heredoc double backslash");
        assert_eq!(double_backslash, "a\\b\n");
    }

    #[test]
    fn quoted_at_produces_separate_fields() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                },
                &arena,
            )
            .expect("quoted at 3"),
            vec!["a", "b", "c"]
        );

        ctx.positional = vec!["one".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                },
                &arena,
            )
            .expect("quoted at 1"),
            vec!["one"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                },
                &arena,
            )
            .expect("quoted at 0"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quoted_at_with_prefix_and_suffix() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"pre$@suf\"".into()
                },
                &arena,
            )
            .expect("prefix suffix"),
            vec!["prea", "bsuf"]
        );

        ctx.positional = vec!["only".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"[$@]\"".into()
                },
                &arena,
            )
            .expect("brackets one"),
            vec!["[only]"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"pre$@suf\"".into()
                },
                &arena,
            )
            .expect("prefix empty"),
            vec!["presuf"]
        );
    }

    #[test]
    fn quoted_at_at_produces_merged_fields() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@$@\"".into()
                },
                &arena,
            )
            .expect("at at"),
            vec!["a", "ba", "b"]
        );
    }

    #[test]
    fn unquoted_at_undergoes_field_splitting() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a b".into(), "c".into()];
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$@".into() }, &arena).expect("unquoted at"),
            vec!["a", "b", "c"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$@".into() }, &arena).expect("unquoted at empty"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quoted_star_joins_with_ifs() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into(), "c".into()];
        ctx.env.insert("IFS".into(), ":".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                },
                &arena,
            )
            .expect("star colon"),
            vec!["a:b:c"]
        );

        ctx.env.insert("IFS".into(), String::new());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                },
                &arena,
            )
            .expect("star empty ifs"),
            vec!["abc"]
        );

        ctx.env.remove("IFS");
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                },
                &arena,
            )
            .expect("star unset ifs"),
            vec!["a b c"]
        );
    }

    #[test]
    fn backtick_command_substitution_in_expander() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "`echo hello`".into()
                },
                &arena,
            )
            .expect("backtick"),
            vec!["echo", "hello"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"`echo hello`\"".into()
                },
                &arena,
            )
            .expect("quoted bt"),
            vec!["echo hello"]
        );
    }

    #[test]
    fn backtick_backslash_escapes() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "`echo \\$USER`".into()
                },
                &arena,
            )
            .expect("escaped dollar"),
            "echo $USER"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "\"`echo \\$USER`\"".into()
                },
                &arena,
            )
            .expect("escaped dollar dq"),
            "echo $USER"
        );
    }

    #[test]
    fn brace_scanning_respects_quotes_and_nesting() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("VAR".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\"a}b\"}".into()
                },
                &arena,
            )
            .expect("quoted brace in default"),
            "a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$(echo ok)}".into()
                },
                &arena,
            )
            .expect("command sub in brace"),
            "echo ok"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$((1+2))}".into()
                },
                &arena,
            )
            .expect("arith in brace"),
            "3"
        );

        ctx.env.insert("INNER".into(), "val".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-${INNER}}".into()
                },
                &arena,
            )
            .expect("nested brace"),
            "val"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-`echo hi`}".into()
                },
                &arena,
            )
            .expect("backtick in brace"),
            "echo hi"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-'a}b'}".into()
                },
                &arena,
            )
            .expect("single quote in brace"),
            "a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\\}}".into()
                },
                &arena,
            )
            .expect("escaped brace"),
            "}"
        );
    }

    #[test]
    fn here_document_expands_at_sign() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["x".into(), "y".into()];
        let result = expand_here_document(&mut ctx, "$@\n", &arena).expect("heredoc at");
        assert_eq!(result, "x y\n");
    }

    #[test]
    fn error_parameter_expansion_operators() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?custom error}".into(),
            },
            &arena,
        )
        .expect_err("colon question");
        assert_eq!(&*error.message, "custom error");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET?also error}".into(),
            },
            &arena,
        )
        .expect_err("question");
        assert_eq!(&*error.message, "also error");
    }

    #[test]
    fn segment_chars_skips_at_break() {
        let segs = vec![
            Segment::Text("a".into(), QuoteState::Expanded),
            Segment::AtBreak,
            Segment::Text("b".into(), QuoteState::Quoted),
        ];
        let chars: Vec<_> = segment_chars(&segs).collect();
        assert_eq!(
            chars,
            vec![('a', QuoteState::Expanded), ('b', QuoteState::Quoted)]
        );
    }

    #[test]
    fn scan_to_closing_brace_error_on_unterminated() {
        let err = scan_to_closing_brace("${var", 2).expect_err("unterminated");
        assert_eq!(&*err.message, "unterminated parameter expansion");
    }

    #[test]
    fn expand_word_empty_quoted_at_with_other_quoted() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"\"\"$@\"".into()
                },
                &arena,
            )
            .expect("empty at dq"),
            vec!["".to_string()]
        );
    }

    #[test]
    fn backtick_inside_double_quotes_with_buffer() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"hello `echo world`\"".into()
                },
                &arena,
            )
            .expect("bt dq buffer"),
            vec!["hello echo world"]
        );
    }

    #[test]
    fn scan_backtick_command_unterminated() {
        let mut index = 1usize;
        let err =
            scan_backtick_command("`unterminated", &mut index, false).expect_err("unterminated");
        assert_eq!(&*err.message, "unterminated backquote");
    }

    #[test]
    fn scan_backtick_command_escape_outside_dq() {
        let mut index = 1usize;
        let result = scan_backtick_command("`echo \\\\ok`", &mut index, false).expect("bt escape");
        assert_eq!(result, "echo \\ok");
    }

    #[test]
    fn here_document_with_at_expansion() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        let result = expand_here_document(&mut ctx, "args: $@\n", &arena).expect("heredoc @");
        assert_eq!(result, "args: a b\n");
    }

    #[test]
    fn brace_scanning_handles_complex_nesting() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("VAR".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$((2+3))}".into()
                },
                &arena,
            )
            .expect("arith in brace scan"),
            "5"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$(echo deep)}".into()
                },
                &arena,
            )
            .expect("cmd sub in brace scan"),
            "echo deep"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-`echo bt`}".into()
                },
                &arena,
            )
            .expect("backtick in brace scan"),
            "echo bt"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\"inside\"}".into()
                },
                &arena,
            )
            .expect("dq in brace scan with escape"),
            "inside"
        );
    }

    #[test]
    fn error_parameter_expansion_with_null_or_not_set() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("EMPTY".into(), String::new());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${EMPTY:?null or unset}".into(),
            },
            &arena,
        )
        .expect_err("colon question null");
        assert_eq!(&*err.message, "null or unset");

        let ok = expand_word(
            &mut ctx,
            &Word {
                raw: "\"${EMPTY?not an error}\"".into(),
            },
            &arena,
        )
        .expect("question set but empty");
        assert_eq!(ok, vec![String::new()]);

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOEXIST?custom msg}".into(),
            },
            &arena,
        )
        .expect_err("question unset");
        assert_eq!(&*err.message, "custom msg");
    }

    #[test]
    fn field_splitting_empty_result_returns_empty_vec() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("WS".into(), "   ".into());
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$WS".into() }, &arena).expect("whitespace only"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn at_break_with_glob_in_at_fields() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.pathname_expansion_enabled = false;
        ctx.positional = vec!["*.txt".into(), "b".into()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
            &arena,
        )
        .expect("at with glob-like");
        assert_eq!(result, vec!["*.txt", "b"]);
    }

    #[test]
    fn flatten_expansion_covers_at_fields() {
        assert_eq!(flatten_expansion(Expansion::One("hello".into())), "hello");
        assert_eq!(
            flatten_expansion(Expansion::AtFields(vec!["a".into(), "b".into()])),
            "a b"
        );
    }

    #[test]
    fn scan_backtick_non_special_escape_in_dquote() {
        let mut index = 1usize;
        let result =
            scan_backtick_command("`echo \\x`", &mut index, true).expect("non-special escape");
        assert_eq!(result, "echo \\x");
    }

    #[test]
    fn at_empty_combined_with_at_break() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["x".into()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
            &arena,
        )
        .expect("at one param");
        assert_eq!(result, vec!["x"]);

        ctx.positional = Vec::new();
        let result2 = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
            &arena,
        )
        .expect("at empty");
        assert_eq!(result2, Vec::<String>::new());
    }

    #[test]
    fn brace_scanning_with_arith_and_cmd_sub_and_backtick() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("V".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-$((1+(2*3)))}".into()
                },
                &arena,
            )
            .expect("nested arith in scan"),
            "7"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-$(echo (hi))}".into()
                },
                &arena,
            )
            .expect("nested cmd sub in scan"),
            "echo (hi)"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-`echo \\\\x`}".into()
                },
                &arena,
            )
            .expect("bt escape in scan"),
            "echo \\x"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-\"q\\}x\"}".into()
                },
                &arena,
            )
            .expect("dq escape in scan"),
            "q}x"
        );
    }

    #[test]
    fn colon_question_error_with_null_value() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("NULL".into(), String::new());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NULL:?is null}".into(),
            },
            &arena,
        )
        .expect_err(":? with null");
        assert_eq!(&*err.message, "is null");

        ctx.nounset_enabled = true;
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NULL:?$NOVAR}".into(),
            },
            &arena,
        )
        .expect_err(":? nounset propagation");
        assert_eq!(&*err.message, "NOVAR: parameter not set");

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOEXIST?$NOVAR}".into(),
            },
            &arena,
        )
        .expect_err("? nounset propagation");
        assert_eq!(&*err.message, "NOVAR: parameter not set");
    }

    #[test]
    fn question_error_with_unset_default_message() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOVAR?}".into(),
            },
            &arena,
        )
        .expect_err("? with unset");
        assert_eq!(&*err.message, "NOVAR: parameter not set");

        ctx.env.insert("SET".into(), "val".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${SET:?no error}".into()
                },
                &arena,
            )
            .expect(":? success"),
            "val"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${SET?no error}".into()
                },
                &arena,
            )
            .expect("? success"),
            "val"
        );

        let err_colon = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOVAR:?}".into(),
            },
            &arena,
        )
        .expect_err(":? with unset");
        assert_eq!(&*err_colon.message, "NOVAR: parameter null or not set");
    }

    #[test]
    fn dquote_backslash_preserves_literal_for_non_special_chars() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: r#""\a\b\c""#.into(),
            },
            &arena,
        )
        .expect("dquote bs");
        assert_eq!(fields, vec![r"\a\b\c"]);
    }

    #[test]
    fn dquote_backslash_escapes_special_chars() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\$""#.into()
                },
                &arena,
            )
            .expect("escape $"),
            vec!["$"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\\""#.into()
                },
                &arena,
            )
            .expect("escape bs"),
            vec!["\\"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\"""#.into()
                },
                &arena,
            )
            .expect("escape dq"),
            vec!["\""],
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"\\`\"".into()
                },
                &arena,
            )
            .expect("escape bt"),
            vec!["`"]
        );
    }

    #[test]
    fn dquote_backslash_newline_is_line_continuation() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "\"ab\\\ncd\"".into(),
            },
            &arena,
        )
        .expect("line continuation");
        assert_eq!(fields, vec!["abcd"]);
    }

    #[test]
    fn tilde_user_expansion() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~testuser/bin".into(),
            },
            &arena,
        )
        .expect("tilde user");
        assert_eq!(fields, vec!["/home/testuser/bin"]);
    }

    #[test]
    fn tilde_unknown_user_preserved() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~nosuchuser/dir".into(),
            },
            &arena,
        )
        .expect("tilde unknown");
        assert_eq!(fields, vec!["~nosuchuser/dir"]);
    }

    #[test]
    fn tilde_user_without_slash() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~testuser".into(),
            },
            &arena,
        )
        .expect("tilde user no slash");
        assert_eq!(fields, vec!["/home/testuser"]);
    }

    #[test]
    fn tilde_after_colon_in_assignment() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: "~/bin:~testuser/lib".into(),
            },
            &arena,
        )
        .expect("tilde colon");
        assert_eq!(result, "/tmp/home/bin:/home/testuser/lib");
    }

    #[test]
    fn arith_variable_reference() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("count".into(), "7".into());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$((count + 3))".into(),
            },
            &arena,
        )
        .expect("arith var");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_dollar_variable_reference() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("n".into(), "5".into());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$(($n * 2))".into(),
            },
            &arena,
        )
        .expect("arith $var");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_comparison_operators() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 < 5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((5 < 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 <= 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((5 > 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 >= 5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 == 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 != 5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
    }

    #[test]
    fn arith_bitwise_operators() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 & 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["2"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 | 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["7"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 ^ 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["5"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((~0))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["-1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 << 4))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["16"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((16 >> 2))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["4"]
        );
    }

    #[test]
    fn arith_logical_operators() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 && 1))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 && 0))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 || 1))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 || 0))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((!0))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((!5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_ternary_operator() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 ? 10 : 20))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["10"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 ? 10 : 20))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["20"]
        );
    }

    #[test]
    fn arith_assignment_operators() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "10".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x = 5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["5"]
        );
        assert_eq!(ctx.env.get("x").unwrap(), "5");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x += 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["8"]
        );
        assert_eq!(ctx.env.get("x").unwrap(), "8");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x -= 2))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["6"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x *= 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["18"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x /= 6))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["3"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x %= 2))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );

        ctx.env.insert("x".into(), "4".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x <<= 2))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["16"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x >>= 1))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["8"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x &= 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );

        ctx.env.insert("x".into(), "5".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x |= 2))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["7"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x ^= 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["4"]
        );
    }

    #[test]
    fn arith_hex_and_octal_constants() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0xff))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["255"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0X1A))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["26"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((010))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["8"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_unary_plus() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((+5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["5"]
        );
    }

    #[test]
    fn arith_unset_variable_is_zero() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((nosuch))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_nested_parens_and_precedence() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((2 + 3 * 4))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["14"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$(((2 + 3) * 4))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["20"]
        );
    }

    #[test]
    fn arith_variable_in_hex_value() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("h".into(), "0xff".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((h))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["255"]
        );
    }

    #[test]
    fn arith_variable_in_octal_value() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("o".into(), "010".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((o))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["8"]
        );
    }

    #[test]
    fn split_colons_handles_quotes_and_backslash() {
        let parts = split_on_unquoted_colons("'b:c':d");
        assert_eq!(parts, vec!["'b:c'", "d"]);

        let parts = split_on_unquoted_colons(r#""b:c":d"#);
        assert_eq!(parts, vec![r#""b:c""#, "d"]);

        let parts = split_on_unquoted_colons(r"a\:b:c");
        assert_eq!(parts, vec![r"a\:b", "c"]);

        let parts = split_on_unquoted_colons(r#""a\"b":c"#);
        assert_eq!(parts, vec![r#""a\"b""#, "c"]);

        let parts = split_on_unquoted_colons("${x:-a:b}:c");
        assert_eq!(parts, vec!["${x:-a:b}", "c"]);

        let parts = split_on_unquoted_colons("$(echo a:b):c");
        assert_eq!(parts, vec!["$(echo a:b)", "c"]);

        let parts = split_on_unquoted_colons("${a:-${b:-x:y}}:z");
        assert_eq!(parts, vec!["${a:-${b:-x:y}}", "z"]);
    }

    #[test]
    fn dquote_trailing_backslash_is_literal() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: r#""abc\"#.into(),
            },
            &arena,
        );
        assert!(fields.is_err());
    }

    #[test]
    fn tilde_with_quoted_char_breaks_tilde_prefix() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~'user'".into(),
            },
            &arena,
        )
        .expect("tilde quoted");
        assert_eq!(fields, vec!["~user"]);
    }

    #[test]
    fn arith_backtick_in_expression() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$((`7` + 3))".into(),
            },
            &arena,
        )
        .expect("arith backtick");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_not_equal_via_parse_unary() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 != 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_compound_assign_div_by_zero() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "5".into());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((x /= 0))".into(),
            },
            &arena,
        )
        .unwrap_err();
        assert_eq!(&*err.message, "division by zero");

        ctx.env.insert("x".into(), "5".into());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((x %= 0))".into(),
            },
            &arena,
        )
        .unwrap_err();
        assert_eq!(&*err.message, "division by zero");
    }

    #[test]
    fn tilde_colon_assignment_with_quotes() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: "~/a:'literal:colon'".into(),
            },
            &arena,
        )
        .expect("colon assign with quotes");
        assert_eq!(result, "/tmp/home/a:literal:colon");
    }

    #[test]
    fn arith_equality_not_confused_with_assignment() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "5".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x == 5))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x == 3))".into()
                },
                &arena,
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_ternary_missing_colon_error() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((1 ? 2 3))".into(),
            },
            &arena,
        )
        .unwrap_err();
        assert!(err.message.contains("':'"));
    }

    #[test]
    fn arith_invalid_hex_constant() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((0x))".into(),
            },
            &arena,
        )
        .unwrap_err();
        assert!(err.message.contains("hex"));
    }

    #[test]
    fn arith_at_fields_in_expression() {
        let arena = StringArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["3".into()];
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$(($@ + 2))".into(),
            },
            &arena,
        )
        .expect("at fields arith");
        assert_eq!(fields, vec!["5"]);
    }

    #[test]
    fn apply_compound_assign_unknown_op_returns_error() {
        let err = apply_compound_assign("??=", 1, 2).unwrap_err();
        assert!(err.message.contains("unknown"));
    }

    #[test]
    fn expand_braced_parameter_pattern_removal_operators() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert("FILE".into(), "archive.tar.gz".into());

            assert_eq!(
                expand_braced_parameter(&mut ctx, "FILE%.*", false).unwrap(),
                Expansion::One("archive.tar".into())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, "FILE%%.*", false).unwrap(),
                Expansion::One("archive".into())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, "FILE#*.", false).unwrap(),
                Expansion::One("tar.gz".into())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, "FILE##*.", false).unwrap(),
                Expansion::One("gz".into())
            );
        });
    }

    #[test]
    fn scan_to_closing_brace_skips_backslash() {
        assert_no_syscalls(|| {
            let pos = scan_to_closing_brace("a\\}b}", 0).unwrap();
            assert_eq!(pos, 4);
        });
    }

    #[test]
    fn expand_parameter_word_as_expansion_with_at_fields() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec!["x".into(), "y".into()];
            let result = expand_parameter_word_as_expansion(&mut ctx, "\"$@\"", false).unwrap();
            assert_eq!(result, Expansion::AtFields(vec!["x".into(), "y".into()]));
        });
    }

    #[test]
    fn expand_word_quoted_null_adjacent_to_empty_at() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = Vec::new();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "''\"$@\"".into()
                    },
                    &arena,
                )
                .unwrap(),
                vec!["".to_string()]
            );
        });
    }

    #[test]
    fn redirect_word_no_pathname_expansion() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: "file_*.txt".into(),
                },
                &arena,
            )
            .expect("redirect word");
            assert_eq!(result, "file_*.txt");
        });
    }

    #[test]
    fn redirect_word_empty_expansion() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: "$UNSET_VAR".into(),
                },
                &arena,
            )
            .expect("redirect word empty");
            assert_eq!(result, "");
        });
    }

    #[test]
    fn redirect_word_with_expanded_field_splitting() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert("V".into(), "a b".into());
            let result = expand_redirect_word(&mut ctx, &Word { raw: "$V".into() }, &arena)
                .expect("redirect word split");
            assert_eq!(result, "a b");
        });
    }

    #[test]
    fn here_doc_backtick_substitution() {
        let arena = StringArena::new();
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result =
                expand_here_document(&mut ctx, "`echo ok`\n", &arena).expect("here doc backtick");
            assert_eq!(result, "echo ok\n");
        });
    }

    #[test]
    fn char_at_and_char_len_handle_multibyte() {
        assert_eq!(char_at("café", 3), 'é');
        assert_eq!(char_len("café", 3), 2);
        assert_eq!(char_at("日本", 0), '日');
        assert_eq!(char_len("日本", 0), 3);
    }

    #[test]
    fn word_is_assignment_rejects_empty_and_non_identifier_prefix() {
        assert!(!word_is_assignment(""));
        assert!(!word_is_assignment("a-b=c"));
    }

    #[test]
    fn fake_context_special_param_star_and_at() {
        let ctx = FakeContext::new();
        assert_eq!(ctx.special_param('*').as_deref(), Some("alpha beta"));
        assert_eq!(ctx.special_param('@').as_deref(), Some("alpha beta"));
    }
}
