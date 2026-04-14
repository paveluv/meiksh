use std::borrow::Cow;

use crate::arena::ByteArena;
use crate::bstr;
use crate::syntax::Word;

use super::core::{Context, ExpandError};
use super::model::{
    ExpandedWord, Expansion, Field, QuoteState, Segment, flatten_segments, is_glob_byte,
    push_segment, push_segment_slice, render_pattern_from_segments, split_fields_from_segments,
};
use super::parameter::{expand_dollar, expand_parameter_dollar};
use super::pathname::expand_pathname;

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

pub(super) fn word_assignment_value(raw: &[u8]) -> Option<&[u8]> {
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

    Ok(arena.intern_vec(bstr::join_bstrings(
        &fields.into_iter().map(|f| f.text).collect::<Vec<_>>(),
        b" ",
    )))
}

pub(super) fn expand_word_with_at_fields(
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

pub(super) fn expand_word_text_assignment<'a, C: Context>(
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

pub(super) fn split_on_unquoted_colons(raw: &[u8]) -> Vec<Vec<u8>> {
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

pub(super) fn expand_parameter_text_owned<C: Context>(
    ctx: &mut C,
    raw: &[u8],
) -> Result<Vec<u8>, ExpandError> {
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

pub(super) fn flatten_expansion(expansion: Expansion) -> Vec<u8> {
    match expansion {
        Expansion::One(s) => s,
        Expansion::AtFields(fields) => bstr::join_bstrings(&fields, b" "),
    }
}

pub(super) fn apply_expansion(
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

pub(super) fn expand_raw<C: Context>(ctx: &mut C, raw: &[u8]) -> Result<ExpandedWord, ExpandError> {
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
                push_segment_slice(&mut segments, &raw[index..index + 1], QuoteState::Literal);
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

pub(super) fn trim_trailing_newlines(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    while end > 0 && s[end - 1] == b'\n' {
        end -= 1;
    }
    &s[..end]
}

pub(super) fn scan_backtick_command(
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

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::arena::ByteArena;
    use crate::bstr;
    use crate::expand::arithmetic::*;
    use crate::expand::core::{Context, ExpandError};
    use crate::expand::glob::*;
    use crate::expand::model::*;
    use crate::expand::parameter::*;
    use crate::expand::pathname::*;
    use crate::expand::test_support::*;
    use crate::expand::word::*;
    use crate::syntax::Word;

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
    fn rejects_bad_arithmetic() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        for raw in [b"$((1 / 0))".as_ref(), b"$((1 + ))", b"$((1 1))"] {
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
    fn helper_paths_cover_remaining_branches() {
        let ctx = FakeContext::new();
        assert_eq!(lookup_param(&ctx, b"?").as_deref(), Some(b"0".as_ref()));
        assert_eq!(
            lookup_param(&ctx, b"0").as_deref(),
            Some(b"meiksh".as_ref())
        );
        assert_eq!(
            lookup_param(&ctx, b"X").as_deref(),
            Some(b"fallback".as_ref())
        );
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
    fn arithmetic_parser_covers_more_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            eval_arithmetic(&mut ctx, b"9 - 2 - 1").expect("subtract"),
            6
        );
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
    fn here_document_expands_at_sign() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
        let result = expand_here_document(&mut ctx, b"$@\n", 0, &arena).expect("heredoc at");
        assert_eq!(result, b"x y\n");
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
    fn scan_backtick_non_special_escape_in_dquote() {
        let mut index = 1usize;
        let result =
            scan_backtick_command(b"`echo \\x`", &mut index, true).expect("non-special escape");
        assert_eq!(result, b"echo \\x");
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
        assert_eq!(
            &*err_colon.message,
            b"NOVAR: parameter null or not set".as_ref()
        );
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
        assert_eq!(
            ctx.special_param(b'*').as_deref(),
            Some(b"alpha beta".as_ref())
        );
        assert_eq!(
            ctx.special_param(b'@').as_deref(),
            Some(b"alpha beta".as_ref())
        );
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
