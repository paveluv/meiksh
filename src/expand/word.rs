use std::borrow::Cow;

use crate::bstr;
use crate::syntax::ast::Word;

use super::core::{Context, ExpandError};
use super::expand_parts::{ExpandOutput, ExpandResult, expand_parts_into_new};
use super::model::{
    ExpandedWord, Expansion, QuoteState, Segment, flatten_segments, push_segment,
    push_segment_slice, render_pattern_from_segments,
};
use super::parameter::{expand_dollar, expand_parameter_dollar};
use super::pathname::expand_pathname;
use crate::syntax::byte_class::{is_glob_char, is_name_cont, is_name_start};

pub(crate) fn expand_words<C: Context>(
    ctx: &mut C,
    words: &[Word],
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let ifs = resolve_ifs(ctx);
    let mut result = Vec::new();
    let mut scratch = ExpandOutput::new();
    for word in words {
        result.extend(expand_word_reuse(ctx, word, &ifs, &mut scratch)?);
    }
    Ok(result)
}

fn resolve_ifs<C: Context>(ctx: &C) -> Cow<'static, [u8]> {
    match ctx.env_var(b"IFS") {
        Some(c) => Cow::Owned(c.into_owned()),
        None => Cow::Borrowed(b" \t\n"),
    }
}

pub(crate) fn expand_word_as_declaration_assignment<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    let value_raw = word_assignment_value(&word.raw).unwrap_or(&word.raw);
    let name = &word.raw[..word.raw.len() - value_raw.len()];
    let value_word = Word {
        raw: value_raw.into(),
        parts: Box::new([]),
        line: word.line,
    };
    let expanded_value = expand_word_text_assignment(ctx, &value_word, true)?;
    let mut combined = Vec::with_capacity(name.len() + expanded_value.len());
    combined.extend_from_slice(name);
    combined.extend_from_slice(&expanded_value);
    Ok(combined)
}

pub(crate) fn word_is_assignment(raw: &[u8]) -> bool {
    word_assignment_value(raw).is_some()
}

pub(super) fn word_assignment_value(raw: &[u8]) -> Option<&[u8]> {
    if raw.is_empty() {
        return None;
    }
    let first = raw[0];
    if !is_name_start(first) {
        return None;
    }
    let mut i = 1;
    while i < raw.len() {
        let b = raw[i];
        if b == b'=' {
            return Some(&raw[i + 1..]);
        }
        if !is_name_cont(b) {
            return None;
        }
        i += 1;
    }
    None
}

pub(crate) fn expand_word<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let ifs = resolve_ifs(ctx);
    let mut scratch = ExpandOutput::new();
    expand_word_reuse(ctx, word, &ifs, &mut scratch)
}

fn expand_word_reuse<C: Context>(
    ctx: &mut C,
    word: &Word,
    ifs: &[u8],
    scratch: &mut ExpandOutput,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    ctx.set_lineno(word.line);

    if !word.parts.is_empty() {
        scratch.clear();
        super::expand_parts::expand_parts_into(ctx, &word.raw, &word.parts, ifs, false, scratch)?;
        let result = scratch.finish();
        return match result {
            ExpandResult::Fields(fields) => Ok(fields),
            ExpandResult::FieldsWithGlob(entries) => {
                let mut result = Vec::new();
                for entry in entries {
                    if entry.has_glob && ctx.pathname_expansion_enabled() {
                        let matches = expand_pathname(&entry.text);
                        if matches.is_empty() {
                            result.push(entry.text);
                        } else {
                            result.extend(matches);
                        }
                    } else {
                        result.push(entry.text);
                    }
                }
                Ok(result)
            }
        };
    }

    // Fallback for Words constructed without pre-parsed parts (e.g. in tests).
    // The parser always populates word.parts, so this path is never reached
    // during normal execution.
    expand_word_raw_fallback(ctx, &word.raw, ifs)
}

fn expand_word_raw_fallback<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    ifs: &[u8],
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;

    let has_at_expansion = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak | Segment::AtEmpty));

    if has_at_expansion {
        let has_at_empty = expanded
            .segments
            .iter()
            .any(|s| matches!(s, Segment::AtEmpty));
        let has_at_break = expanded
            .segments
            .iter()
            .any(|s| matches!(s, Segment::AtBreak));

        if has_at_empty && !has_at_break {
            let text = flatten_segments(&expanded.segments);
            if !text.is_empty() || expanded.had_quoted_null_outside_at {
                return Ok(vec![text]);
            }
            return Ok(Vec::new());
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
        return Ok(fields);
    }

    if expanded.segments.is_empty() {
        if expanded.had_quoted_content {
            return Ok(vec![Vec::new()]);
        }
        return Ok(Vec::new());
    }

    let has_expanded = expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, QuoteState::Expanded)));

    let fields = if has_expanded {
        split_fields_raw(&expanded.segments, ifs)
    } else {
        let text = flatten_segments(&expanded.segments);
        debug_assert!(!text.is_empty());
        let has_glob = expanded.segments.iter().any(|seg| {
            matches!(seg, Segment::Text(text, QuoteState::Literal) if text.iter().any(|&b| is_glob_char(b)))
        });
        if has_glob && ctx.pathname_expansion_enabled() {
            let matches = expand_pathname(&text);
            if matches.is_empty() {
                vec![text]
            } else {
                matches
            }
        } else {
            vec![text]
        }
    };
    Ok(fields)
}

fn split_fields_raw(segments: &[Segment], ifs: &[u8]) -> Vec<Vec<u8>> {
    if ifs.is_empty() {
        return vec![flatten_segments(segments)];
    }
    let mut fields = Vec::new();
    let mut current = Vec::new();

    let ifs_chars = super::expand_parts::decompose_ifs(ifs);

    for seg in segments {
        #[rustfmt::skip]
        let Segment::Text(text, state) = seg else { continue };
        if *state != QuoteState::Expanded {
            current.extend_from_slice(text);
            continue;
        }
        let mut i = 0;
        while i < text.len() {
            if let Some((_, byte_seq, is_ws)) =
                super::expand_parts::find_ifs_char_at(&ifs_chars, &text[i..])
            {
                if is_ws {
                    if !current.is_empty() {
                        fields.push(std::mem::take(&mut current));
                    }
                } else {
                    fields.push(std::mem::take(&mut current));
                }
                i += byte_seq.len();
            } else {
                current.push(text[i]);
                i += 1;
            }
        }
    }
    if !current.is_empty() {
        fields.push(current);
    }
    fields
}

pub(crate) fn expand_redirect_word<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    let ifs = resolve_ifs(ctx);

    if !word.parts.is_empty() {
        let mut output = expand_parts_into_new(ctx, &word.raw, &word.parts, &ifs, false)?;
        let result = output.finish();
        return Ok(match result {
            ExpandResult::Fields(fields) => bstr::join_bstrings(&fields, b" "),
            ExpandResult::FieldsWithGlob(entries) => bstr::join_bstrings(
                &entries.into_iter().map(|e| e.text).collect::<Vec<_>>(),
                b" ",
            ),
        });
    }

    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(flatten_segments(&expanded.segments))
}

pub(crate) fn expand_word_text<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);

    if !word.parts.is_empty() {
        let mut output = ExpandOutput::new();
        super::expand_parts::expand_parts_into(
            ctx,
            &word.raw,
            &word.parts,
            b"",
            true,
            &mut output,
        )?;
        return Ok(output.drain_single_vec());
    }

    expand_word_text_assignment(ctx, word, false)
}

pub(crate) fn expand_word_pattern<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
}

pub(crate) fn expand_assignment_value<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);

    if !word.parts.is_empty() {
        let mut output = ExpandOutput::new();
        #[rustfmt::skip]
        super::expand_parts::expand_parts_into(ctx, &word.raw, &word.parts, b"", true, &mut output)?;
        return Ok(output.drain_single_vec());
    }
    expand_word_text_assignment(ctx, word, true)
}

pub(super) fn expand_word_text_assignment<C: Context>(
    ctx: &mut C,
    word: &Word,
    assignment_rhs: bool,
) -> Result<Vec<u8>, ExpandError> {
    if !assignment_rhs {
        let expanded = expand_raw(ctx, &word.raw)?;
        return Ok(flatten_segments(&expanded.segments));
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
    Ok(result)
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

pub(crate) fn expand_parameter_text<C: Context>(
    ctx: &mut C,
    raw: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    expand_parameter_text_owned(ctx, raw)
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
        Expansion::Static(s) => s.to_vec(),
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
        Expansion::Static(s) => push_segment_slice(segments, s, state),
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
    let mut has_at_expansion = false;
    let mut had_quoted_content = false;
    let mut had_quoted_null_outside_at = false;

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
                let _at_before = has_at_expansion;
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
                            let output = ctx.command_substitute_raw(&command)?;
                            let trimmed = trim_trailing_newlines(&output);
                            push_segment_slice(&mut segments, trimmed, QuoteState::Quoted);
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
                if !has_at_expansion || _at_before == has_at_expansion {
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
                let output = ctx.command_substitute_raw(&command)?;
                let trimmed = trim_trailing_newlines(&output);
                push_segment_slice(&mut segments, trimmed, QuoteState::Expanded);
            }
            b'~' if index == 0 => {
                index += 1;
                let at_start = index;
                while index < raw.len() && raw[index] != b'/' {
                    let b = raw[index];
                    if b == b'\'' || b == b'"' || b == b'\\' || b == b'$' || b == b'`' {
                        break;
                    }
                    index += 1;
                }
                let user = &raw[at_start..index];
                let broke_on_non_login =
                    index == at_start && index < raw.len() && raw[index] != b'/';
                let slash_follows = index < raw.len() && raw[index] == b'/';
                if broke_on_non_login {
                    push_segment_slice(&mut segments, b"~", QuoteState::Literal);
                } else if user.is_empty() {
                    match ctx.env_var(b"HOME") {
                        Some(home) if !home.is_empty() => {
                            let mut h = home.into_owned();
                            if slash_follows && h.ends_with(b"/") {
                                h.pop();
                            }
                            push_segment(&mut segments, h, QuoteState::Quoted);
                        }
                        Some(_) => {
                            segments.push(Segment::Text(Vec::new(), QuoteState::Quoted));
                        }
                        None => {
                            push_segment_slice(&mut segments, b"~", QuoteState::Literal);
                        }
                    }
                } else if let Some(dir) = ctx.home_dir_for_user(user) {
                    let mut d = dir.into_owned();
                    if slash_follows && d.ends_with(b"/") {
                        d.pop();
                    }
                    push_segment(&mut segments, d, QuoteState::Quoted);
                } else {
                    push_segment_slice(&mut segments, b"~", QuoteState::Literal);
                    push_segment_slice(&mut segments, user, QuoteState::Literal);
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

pub(crate) fn expand_here_document<C: Context>(
    ctx: &mut C,
    text: &[u8],
    body_line: usize,
) -> Result<Vec<u8>, ExpandError> {
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
                let output = ctx.command_substitute_raw(&command)?;
                result.extend_from_slice(trim_trailing_newlines(&output));
            }
            _ => {
                result.push(text[index]);
                index += 1;
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::arithmetic::{
        ArithmeticParser, eval_arithmetic, expand_arithmetic_expression,
    };
    use crate::expand::core::Context;
    use crate::expand::glob::pattern_matches;
    use crate::expand::model::{QuoteState, Segment, flatten_segments, push_segment};
    use crate::expand::parameter::lookup_param;
    use crate::expand::test_support::FakeContext;
    use crate::syntax::ast::Word;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn expands_home_and_params() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~/$USER".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("expand");
        assert_eq!(fields, vec![b"/tmp/home/meiksh".to_vec()]);
    }

    #[test]
    fn expands_arithmetic_expressions() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 + 2 * 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"7".to_vec()]
        );
    }

    #[test]
    fn expands_command_substitution() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_words(
                &mut ctx,
                &[
                    Word {
                        raw: b"$WORDS".as_ref().into(),
                        parts: Box::new([]),
                        line: 0
                    },
                    Word {
                        raw: b"$(printf hi)".as_ref().into(),
                        parts: Box::new([]),
                        line: 0
                    },
                ],
            )
            .expect("expand"),
            vec![
                b"one".to_vec(),
                b"two".to_vec(),
                b"three".to_vec(),
                b"printf".to_vec(),
                b"hi".to_vec(),
            ]
        );
    }

    #[test]
    fn preserves_quoted_and_escaped_characters() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$0 $1\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"meiksh alpha".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\\$HOME".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"$HOME".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"a\\ b".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"a b".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"'literal text'".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"literal text".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"cost:\\$USER\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"cost:$USER".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$'a b'".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"a b".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$'line\\nnext'".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"line\nnext".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$'a b'\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"$'a b'".to_vec()]
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"$'tab\\tstop'").expect("parameter text"),
            b"tab\tstop".as_ref()
        );
    }

    #[test]
    fn rejects_unterminated_quotes_and_expansions() {
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
                    parts: Box::new([]),
                    line: 0,
                },
            )
            .expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn rejects_bad_arithmetic() {
        let mut ctx = FakeContext::new();
        for raw in [b"$((1 / 0))".as_ref(), b"$((1 + ))", b"$((1 1))"] {
            let error = expand_word(
                &mut ctx,
                &Word {
                    raw: raw.into(),
                    parts: Box::new([]),
                    line: 0,
                },
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
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("quoted at 3"),
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
        );

        ctx.positional = vec![b"one".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("quoted at 1"),
            vec![b"one".to_vec()]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("quoted at 0"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn quoted_at_with_prefix_and_suffix() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"pre$@suf\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("prefix suffix"),
            vec![b"prea".to_vec(), b"bsuf".to_vec()]
        );

        ctx.positional = vec![b"only".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"[$@]\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("brackets one"),
            vec![b"[only]".to_vec()]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"pre$@suf\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("prefix empty"),
            vec![b"presuf".to_vec()]
        );
    }

    #[test]
    fn quoted_at_at_produces_merged_fields() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$@$@\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("at at"),
            vec![b"a".to_vec(), b"ba".to_vec(), b"b".to_vec()]
        );
    }

    #[test]
    fn quoted_star_joins_with_ifs() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("star colon"),
            vec![b"a:b:c".to_vec()]
        );

        ctx.env.insert(b"IFS".to_vec(), Vec::new());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("star empty ifs"),
            vec![b"abc".to_vec()]
        );

        ctx.env.remove(b"IFS".as_ref());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("star unset ifs"),
            vec![b"a b c".to_vec()]
        );
    }

    #[test]
    fn backtick_command_substitution_in_expander() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"`echo hello`".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("backtick"),
            vec![b"echo".to_vec(), b"hello".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"`echo hello`\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("quoted bt"),
            vec![b"echo hello".to_vec()]
        );
    }

    #[test]
    fn backtick_backslash_escapes() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"`echo \\$USER`".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("escaped dollar"),
            b"echo $USER"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"\"`echo \\$USER`\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("escaped dollar dq"),
            b"echo $USER"
        );
    }

    #[test]
    fn here_document_expands_at_sign() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
        let result = expand_here_document(&mut ctx, b"$@\n", 0).expect("heredoc at");
        assert_eq!(result, b"x y\n");
    }

    #[test]
    fn expand_word_empty_quoted_at_with_other_quoted() {
        let mut ctx = FakeContext::new();
        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\"\"$@\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("empty at dq"),
            vec![b"".to_vec()]
        );
    }

    #[test]
    fn backtick_inside_double_quotes_with_buffer() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"hello `echo world`\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("bt dq buffer"),
            vec![b"hello echo world".to_vec()]
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
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        let result = expand_here_document(&mut ctx, b"args: $@\n", 0).expect("heredoc @");
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
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"NULL".to_vec(), Vec::new());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NULL:?is null}".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect_err(":? with null");
        assert_eq!(&*err.message, b"is null".as_ref());

        ctx.nounset_enabled = true;
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NULL:?$NOVAR}".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect_err(":? nounset propagation");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOEXIST?$NOVAR}".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect_err("? nounset propagation");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());
    }

    #[test]
    fn question_error_with_unset_default_message() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOVAR?}".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect_err("? with unset");
        assert_eq!(&*err.message, b"NOVAR: parameter not set".as_ref());

        ctx.env.insert(b"SET".to_vec(), b"val".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${SET:?no error}".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect(":? success"),
            b"val"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${SET?no error}".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("? success"),
            b"val"
        );

        let err_colon = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOVAR:?}".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect_err(":? with unset");
        assert_eq!(
            &*err_colon.message,
            b"NOVAR: parameter null or not set".as_ref()
        );
    }

    #[test]
    fn dquote_backslash_preserves_literal_for_non_special_chars() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"\\a\\b\\c\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("dquote bs");
        assert_eq!(fields, vec![b"\\a\\b\\c".to_vec()]);
    }

    #[test]
    fn dquote_backslash_escapes_special_chars() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\$\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("escape $"),
            vec![b"$".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\\\\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("escape bs"),
            vec![b"\\".to_vec()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\\"\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("escape dq"),
            vec![b"\"".to_vec()],
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"\\`\"".as_ref().into(),
                    parts: Box::new([]),
                    line: 0,
                },
            )
            .expect("escape bt"),
            vec![b"`".to_vec()]
        );
    }

    #[test]
    fn dquote_backslash_newline_is_line_continuation() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"ab\\\ncd\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("line continuation");
        assert_eq!(fields, vec![b"abcd".to_vec()]);
    }

    #[test]
    fn tilde_user_expansion() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~testuser/bin".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde user");
        assert_eq!(fields, vec![b"/home/testuser/bin".to_vec()]);
    }

    #[test]
    fn tilde_unknown_user_preserved() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~nosuchuser/dir".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde unknown");
        assert_eq!(fields, vec![b"~nosuchuser/dir".to_vec()]);
    }

    #[test]
    fn tilde_user_without_slash() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~testuser".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde user no slash");
        assert_eq!(fields, vec![b"/home/testuser".to_vec()]);
    }

    #[test]
    fn tilde_after_colon_in_assignment() {
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: b"~/bin:~testuser/lib".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
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
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"abc\\".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        );
        assert!(fields.is_err());
    }

    #[test]
    fn tilde_with_quoted_char_breaks_tilde_prefix() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~'user'".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde quoted");
        assert_eq!(fields, vec![b"~user".to_vec()]);
    }

    #[test]
    fn tilde_colon_assignment_with_quotes() {
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: b"~/a:'literal:colon'".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("colon assign with quotes");
        assert_eq!(result, b"/tmp/home/a:literal:colon");
    }

    #[test]
    fn expand_word_quoted_null_adjacent_to_empty_at() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = Vec::new();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: b"''\"$@\"".as_ref().into(),
                        parts: Box::new([]),
                        line: 0
                    },
                )
                .unwrap(),
                vec![b"".to_vec()]
            );
        });
    }

    #[test]
    fn redirect_word_empty_expansion() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"$UNSET_VAR".as_ref().into(),
                    parts: Box::new([]),
                    line: 0,
                },
            )
            .expect("redirect word empty");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn here_doc_backtick_substitution() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result =
                expand_here_document(&mut ctx, b"`echo ok`\n", 0).expect("here doc backtick");
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
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"'hello\nworld'".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("single quote newline");
        assert_eq!(fields, vec![b"hello\nworld".to_vec()]);
    }

    #[test]
    fn newline_inside_double_quote_increments_lineno() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"hello\nworld\"".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("double quote newline");
        assert_eq!(fields, vec![b"hello\nworld".to_vec()]);
    }

    #[test]
    fn backslash_newline_inside_double_quote() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"a\\\nb\"".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("backslash newline in dquote");
        assert_eq!(fields, vec![b"ab".to_vec()]);
    }

    #[test]
    fn backslash_escape_in_unquoted_context() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\\a\\b".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("backslash escape");
        assert_eq!(fields, vec![b"ab".to_vec()]);
    }

    #[test]
    fn backslash_newline_in_unquoted_context() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"a\\\nb".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("backslash newline unquoted");
        assert_eq!(fields.len(), 1);
        assert!(fields[0].contains(&b'\n'));
    }

    #[test]
    fn trailing_backslash_in_unquoted_context() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"a\\".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("trailing backslash");
        assert_eq!(fields, vec![b"a".to_vec()]);
    }

    #[test]
    fn bare_newline_in_unquoted_context() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"hello\nworld".as_ref().into(),
                parts: Box::new([]),
                line: 1,
            },
        )
        .expect("bare newline");
        assert_eq!(fields.len(), 1);
        assert!(fields[0].contains(&b'\n'));
    }

    #[test]
    fn arithmetic_nounset_rejects_unset_variable() {
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((nosuch_var))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        );
        assert!(result.is_err());
    }

    fn parsed_cmd_word(source: &[u8]) -> Word {
        let prog = crate::syntax::parse(source).expect("parse");
        let item = &prog.items[0];
        let cmd = &item.and_or.first.commands[0];
        match cmd {
            crate::syntax::ast::Command::Simple(sc) => sc.words[1].clone(),
            _ => panic!("expected simple command"),
        }
    }

    #[test]
    fn expand_word_via_parts_simple_var() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"X".to_vec(), b"hello".to_vec());
        let word = parsed_cmd_word(b"echo $X\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("fields");
        assert_eq!(fields, vec![b"hello".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_quoted_at_empty() {
        let mut ctx = FakeContext::new();
        ctx.positional.clear();
        let word = parsed_cmd_word(b"echo \"$@\"\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("fields");
        assert_eq!(fields, Vec::<Vec<u8>>::new());
    }

    fn parts_word(source: &[u8]) -> Word {
        let w = parsed_cmd_word(source);
        assert!(
            !w.parts.is_empty(),
            "expected parts for {:?}",
            std::str::from_utf8(&w.raw)
        );
        w
    }

    #[test]
    fn expand_assignment_value_via_parts() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"Y".to_vec(), b"world".to_vec());
        let word = parts_word(b"echo hello${Y}\n");
        let result = expand_assignment_value(&mut ctx, &word).expect("assign");
        assert_eq!(result, b"helloworld");
    }

    #[test]
    fn expand_assignment_value_via_parts_with_at() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        let word = parts_word(b"echo $@\n");
        let result = expand_assignment_value(&mut ctx, &word).expect("assign at");
        assert_eq!(result, b"ab");
    }

    #[test]
    fn expand_redirect_word_via_parts() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"F".to_vec(), b"out.txt".to_vec());
        let word = parsed_cmd_word(b"echo $F\n");
        assert!(!word.parts.is_empty());
        let result = expand_redirect_word(&mut ctx, &word).expect("redir");
        assert_eq!(result, b"out.txt");
    }

    #[test]
    fn expand_word_text_via_parts() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"world".to_vec());
        let word = parsed_cmd_word(b"echo \"hello $V\"\n");
        assert!(!word.parts.is_empty());
        let result = expand_word_text(&mut ctx, &word).expect("text");
        assert_eq!(result, b"hello world");
    }

    #[test]
    fn expand_word_via_parts_with_ifs_split() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a b c".to_vec());
        let word = parsed_cmd_word(b"echo $V\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("fields");
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_empty_ifs() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a b c".to_vec());
        ctx.env.insert(b"IFS".to_vec(), b"".to_vec());
        let word = parsed_cmd_word(b"echo $V\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("fields");
        assert_eq!(fields, vec![b"a b c".to_vec()]);
    }

    #[test]
    fn expand_word_parsed_tilde_home_empty() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"".to_vec());
        let word = parsed_cmd_word(b"echo ~\n");
        assert!(!word.parts.is_empty());
        let text = expand_word_text(&mut ctx, &word).expect("text");
        assert_eq!(text, b"");
    }

    #[test]
    fn expand_word_parsed_tilde_home_unset() {
        let mut ctx = FakeContext::new();
        ctx.env.remove(b"HOME".as_ref());
        let word = parsed_cmd_word(b"echo ~\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("fields");
        assert_eq!(fields, vec![b"~".to_vec()]);
    }

    #[test]
    fn expand_redirect_word_via_parts_static_expansion() {
        let mut ctx = FakeContext::new();
        let word = parsed_cmd_word(b"echo $?\n");
        assert!(!word.parts.is_empty());
        let result = expand_redirect_word(&mut ctx, &word).expect("redir static");
        assert_eq!(result, b"0");
    }

    #[test]
    fn expand_arith_literal_via_parts() {
        let mut ctx = FakeContext::new();
        let word = parsed_cmd_word(b"echo $((42))\n");
        assert!(!word.parts.is_empty());
        let fields = expand_word(&mut ctx, &word).expect("arith");
        assert_eq!(fields, vec![b"42".to_vec()]);
    }

    #[test]
    fn tilde_home_empty_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"".to_vec());
        let result = expand_word_text(
            &mut ctx,
            &Word {
                raw: b"~".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde empty home");
        assert_eq!(result, b"");
    }

    #[test]
    fn tilde_home_unset_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.remove(b"HOME".as_ref());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde unset home");
        assert_eq!(fields, vec![b"~".to_vec()]);
    }

    #[test]
    fn tilde_user_trailing_slash_raw_path() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"~testuser/sub".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("tilde user");
        assert_eq!(fields, vec![b"/home/testuser/sub".to_vec()]);
    }

    #[test]
    fn redirect_word_with_expansion_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"F".to_vec(), b"a b".to_vec());
        let result = expand_redirect_word(
            &mut ctx,
            &Word {
                raw: b"$F".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("redirect fields");
        assert_eq!(result, b"a b");
    }

    #[test]
    fn expand_word_empty_ifs_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a b c".to_vec());
        ctx.env.insert(b"IFS".to_vec(), b"".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$V".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("empty ifs");
        assert_eq!(fields, vec![b"a b c".to_vec()]);
    }

    #[test]
    fn expand_word_other_ifs_split_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a:b:c".to_vec());
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$V".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("ifs split");
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn expand_assignment_value_raw_path() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"Y".to_vec(), b"world".to_vec());
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: b"hello$Y".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("assign raw");
        assert_eq!(result, b"helloworld");
    }

    #[test]
    fn expand_word_empty_non_expanded_raw_path() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("empty quoted");
        assert_eq!(fields, vec![b"".to_vec()]);
    }

    #[test]
    fn flatten_expansion_static_variant() {
        let result = flatten_expansion(Expansion::Static(b"test"));
        assert_eq!(result, b"test");
    }

    #[test]
    fn expand_redirect_word_via_parts_multiple_fields() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"F".to_vec(), b"a b".to_vec());
        let word = parts_word(b"echo $F\n");
        let result = expand_redirect_word(&mut ctx, &word).expect("redirect");
        assert_eq!(result, b"a b");
    }

    #[test]
    fn expand_word_via_parts_tilde_user_trailing_slash() {
        let mut ctx = FakeContext::new();
        let word = parts_word(b"echo ~testuser/sub\n");
        let fields = expand_word(&mut ctx, &word).expect("tilde user parts");
        assert_eq!(fields, vec![b"/home/testuser/sub".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_tilde_home_empty() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"".to_vec());
        let word = parts_word(b"echo ~\n");
        let result = expand_word_text(&mut ctx, &word).expect("tilde empty home parts");
        assert_eq!(result, b"");
    }

    #[test]
    fn expand_word_via_parts_tilde_home_unset() {
        let mut ctx = FakeContext::new();
        ctx.env.remove(b"HOME".as_ref());
        let word = parts_word(b"echo ~\n");
        let fields = expand_word(&mut ctx, &word).expect("tilde unset home parts");
        assert_eq!(fields, vec![b"~".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_ifs_other_split() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a:b:c".to_vec());
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        let word = parts_word(b"echo $V\n");
        let fields = expand_word(&mut ctx, &word).expect("ifs other split");
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_empty_ifs_no_split() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), b"a b c".to_vec());
        ctx.env.insert(b"IFS".to_vec(), b"".to_vec());
        let word = parts_word(b"echo $V\n");
        let fields = expand_word(&mut ctx, &word).expect("empty ifs");
        assert_eq!(fields, vec![b"a b c".to_vec()]);
    }

    #[test]
    fn expand_word_via_parts_literal_with_newlines() {
        let mut ctx = FakeContext::new();
        let word = parts_word(b"echo line1\necho line2\n");
        let fields = expand_word(&mut ctx, &word).expect("word with newlines");
        assert!(!fields.is_empty());
    }

    #[test]
    fn expand_word_via_parts_tilde_trailing_slash_home() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/root/".to_vec());
        let word = parts_word(b"echo ~/foo\n");
        let fields = expand_word(&mut ctx, &word).expect("tilde trailing slash");
        assert_eq!(fields, vec![b"/root/foo".to_vec()]);
    }

    #[test]
    fn expand_redirect_word_static_expansion_via_parts() {
        let mut ctx = FakeContext::new();
        let word = parts_word(b"echo $?\n");
        let result = expand_redirect_word(&mut ctx, &word).expect("redirect static");
        assert_eq!(result, b"0");
    }

    #[test]
    fn push_literal_with_glob_char_via_unknown_tilde() {
        use crate::syntax::word_parts::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let word = Word {
                raw: b"~unkn*wn".as_ref().into(),
                parts: Box::new([WordPart::TildeLiteral {
                    tilde_pos: 0,
                    user_end: 8,
                    end: 8,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("tilde unknown with glob");
            assert_eq!(fields, vec![b"~unkn*wn".to_vec()]);
        });
    }

    #[test]
    fn expand_word_via_parts_arith_with_special_param() {
        use crate::syntax::word_parts::{ExpansionKind, WordPart};
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let word = Word {
                raw: b"$(($?+1))".as_ref().into(),
                parts: Box::new([WordPart::Expansion {
                    kind: ExpansionKind::Arithmetic {
                        parts: Box::new([
                            WordPart::Expansion {
                                kind: ExpansionKind::SpecialVar { ch: b'?' },
                                quoted: false,
                            },
                            WordPart::Literal {
                                start: 5,
                                end: 7,
                                has_glob: false,
                                newlines: 0,
                            },
                        ]),
                    },
                    quoted: false,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("arith");
            assert_eq!(fields, vec![b"1".to_vec()]);
        });
    }

    #[test]
    fn expand_word_at_empty_in_braced_default() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional.clear();
            let word = parts_word(b"echo ${x:-\"$@\"}\n");
            let result = expand_word_text(&mut ctx, &word).expect("at empty braced");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn literal_with_newlines_increments_lineno() {
        use crate::syntax::word_parts::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = Word {
                raw: b"ab\ncd".as_ref().into(),
                parts: Box::new([WordPart::Literal {
                    start: 0,
                    end: 5,
                    has_glob: false,
                    newlines: 1,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("literal newlines");
            assert_eq!(fields, vec![b"ab\ncd".to_vec()]);
        });
    }

    #[test]
    fn tilde_user_trailing_slash_in_homedir_via_parts() {
        use crate::syntax::word_parts::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let raw = b"~slashuser/sub";
            let word = Word {
                raw: raw.as_ref().into(),
                parts: Box::new([WordPart::TildeLiteral {
                    tilde_pos: 0,
                    user_end: 10,
                    end: 14,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("tilde slash user");
            assert_eq!(fields, vec![b"/home/slashuser/sub".to_vec()]);
        });
    }

    #[test]
    fn tilde_literal_noop_in_arithmetic_parts() {
        use crate::syntax::word_parts::{ExpansionKind, WordPart};
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let word = Word {
                raw: b"$((1+2))".as_ref().into(),
                parts: Box::new([WordPart::Expansion {
                    kind: ExpansionKind::Arithmetic {
                        parts: Box::new([
                            WordPart::Literal {
                                start: 3,
                                end: 6,
                                has_glob: false,
                                newlines: 0,
                            },
                            WordPart::TildeLiteral {
                                tilde_pos: 0,
                                user_end: 0,
                                end: 0,
                            },
                        ]),
                    },
                    quoted: false,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("arith tilde noop");
            assert_eq!(fields, vec![b"3".to_vec()]);
        });
    }

    #[test]
    fn tilde_literal_noop_in_pattern_parts() {
        use crate::syntax::word_parts::{BracedName, BracedOp, ExpansionKind, WordPart};
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"V".to_vec(), b"hello_world".to_vec());
            let raw = b"${V%_*}";
            let word = Word {
                raw: raw.as_ref().into(),
                parts: Box::new([WordPart::Expansion {
                    kind: ExpansionKind::Braced {
                        name: BracedName::Var { start: 2, end: 3 },
                        op: BracedOp::TrimSuffix,
                        parts: Box::new([
                            WordPart::Literal {
                                start: 4,
                                end: 6,
                                has_glob: true,
                                newlines: 0,
                            },
                            WordPart::TildeLiteral {
                                tilde_pos: 0,
                                user_end: 0,
                                end: 0,
                            },
                        ]),
                    },
                    quoted: false,
                }]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("pattern tilde noop");
            assert_eq!(fields, vec![b"hello".to_vec()]);
        });
    }

    #[test]
    fn drain_single_vec_via_assignment_star() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = parts_word(b"echo $*\n");
            let result = expand_assignment_value(&mut ctx, &word).expect("assign star");
            assert_eq!(result, b"alpha beta");
        });
    }

    #[test]
    fn expand_word_at_single_positional_no_break() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec![b"only".to_vec()];
            let word = Word {
                raw: b"\"$@\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("single at");
            assert_eq!(fields, vec![b"only".to_vec()]);
        });
    }

    #[test]
    fn expand_word_at_empty_no_break() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional.clear();
            let word = Word {
                raw: b"\"$@\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("empty at");
            assert!(fields.is_empty());
        });
    }

    #[test]
    fn expand_redirect_word_at_expansion_via_parts() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec![b"file.txt".to_vec()];
            let word = parts_word(b"echo \"$@\"\n");
            let result = expand_redirect_word(&mut ctx, &word).expect("redirect at");
            assert_eq!(result, b"file.txt");
        });
    }

    #[test]
    fn expand_redirect_word_empty_quoted_via_parts() {
        use crate::syntax::word_parts::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = Word {
                raw: b"\"\"".as_ref().into(),
                parts: Box::new([WordPart::QuotedLiteral {
                    bytes: Box::from(&b""[..]),
                    newlines: 0,
                }]),
                line: 0,
            };
            let result = expand_redirect_word(&mut ctx, &word).expect("redirect empty quoted");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn tilde_user_trailing_slash_raw_path_with_slash_user() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let word = Word {
                raw: b"~slashuser/sub".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("tilde slash raw");
            assert_eq!(fields, vec![b"/home/slashuser/sub".to_vec()]);
        });
    }

    #[test]
    fn expand_word_empty_text_non_expanded_raw() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = Word {
                raw: b"''".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            };
            let fields = expand_word(&mut ctx, &word).expect("empty quoted");
            assert_eq!(fields, vec![Vec::<u8>::new()]);
        });
    }
}
