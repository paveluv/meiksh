use crate::bstr;
use crate::syntax::ast::Word;
use crate::syntax::word_parts::WordPart;

use super::core::{Context, ExpandError};
use super::expand_parts::{ExpandOutput, expand_parts_into};
use super::model::{
    ExpandedWord, Expansion, QuoteState, Segment, flatten_segments, push_segment,
    push_segment_slice, render_pattern_from_segments,
};
use super::parameter::{expand_dollar, expand_parameter_dollar};
use super::scratch::ExpandScratch;
use crate::syntax::byte_class::{is_name_cont, is_name_start};

pub(crate) fn expand_words_into<C: Context>(
    ctx: &mut C,
    words: &[Word],
    argv: &mut Vec<Vec<u8>>,
) -> Result<(), ExpandError> {
    with_scratch(ctx, |ctx, scratch| {
        ensure_ifs_cached(ctx, scratch);
        for word in words {
            expand_word_with_scratch(ctx, word, scratch, argv)?;
        }
        Ok(())
    })
}

/// Take the context's `ExpandScratch` out of `ctx`, run `body` with it, and
/// always put it back (even on error). Nested expansion calls on the same
/// `ctx` during `body` will observe a default/empty scratch; this is
/// correct but loses pooling for that nesting level. In practice the only
/// re-entry path during word expansion is command substitution, which
/// `fork()`s into a child process with its own shell state.
fn with_scratch<C, R>(
    ctx: &mut C,
    body: impl FnOnce(&mut C, &mut ExpandScratch) -> Result<R, ExpandError>,
) -> Result<R, ExpandError>
where
    C: Context,
{
    let mut scratch = std::mem::take(ctx.expand_scratch_mut());
    let result = body(ctx, &mut scratch);
    *ctx.expand_scratch_mut() = scratch;
    result
}

/// Ensure `scratch.ifs_bytes` holds the current `$IFS`. Cached across
/// calls because IFS is read on every simple command but rarely mutated;
/// [`ExpandScratch::invalidate_ifs`] is called from `set_var` / `unset_var`
/// whenever `IFS` is touched.
fn ensure_ifs_cached<C: Context>(ctx: &C, scratch: &mut ExpandScratch) {
    if scratch.ifs_valid {
        return;
    }
    scratch.ifs_bytes.clear();
    match ctx.env_var(b"IFS") {
        Some(c) => scratch.ifs_bytes.extend_from_slice(&c),
        None => scratch.ifs_bytes.extend_from_slice(b" \t\n"),
    }
    scratch.ifs_valid = true;
}

fn expand_word_with_scratch<C: Context>(
    ctx: &mut C,
    word: &Word,
    scratch: &mut ExpandScratch,
    argv: &mut Vec<Vec<u8>>,
) -> Result<(), ExpandError> {
    let ExpandScratch {
        ifs_bytes, output, ..
    } = scratch;
    expand_word_into(ctx, word, ifs_bytes, output, argv)
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

fn expand_word_into<C: Context>(
    ctx: &mut C,
    word: &Word,
    ifs: &[u8],
    scratch: &mut ExpandOutput,
    argv: &mut Vec<Vec<u8>>,
) -> Result<(), ExpandError> {
    ctx.set_lineno(word.line);

    if word.parts.is_empty() {
        // Parser invariant: a non-empty `raw` always carries a non-empty
        // `parts` slice (see keyword-as-command recovery in
        // `syntax::ast`). Truly empty words contribute nothing to argv.
        debug_assert!(
            word.raw.is_empty(),
            "parser invariant violated: Word with empty parts and non-empty raw reached expand_word_into: {:?}",
            word.raw,
        );
        return Ok(());
    }

    // Fast path: a single literal WordPart that spans the full raw
    // word with no glob metacharacters and no embedded newlines is
    // the overwhelmingly common case for tokens like `[`, `-gt`,
    // `0`, `case`, `then`. Bypass ExpandOutput entirely and push the
    // single owned byte vector directly into argv.
    if let [
        WordPart::Literal {
            start: 0,
            end,
            has_glob: false,
            newlines: 0,
        },
    ] = &word.parts[..]
        && *end == word.raw.len()
    {
        if !word.raw.is_empty() {
            argv.push(word.raw.to_vec());
        }
        return Ok(());
    }

    scratch.clear();
    super::expand_parts::expand_parts_into(ctx, &word.raw, &word.parts, ifs, false, scratch)?;
    scratch.finish_into(ctx, argv)
}

pub(crate) fn expand_redirect_word<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);

    if !word.parts.is_empty() {
        return with_scratch(ctx, |ctx, scratch| {
            ensure_ifs_cached(ctx, scratch);
            let ExpandScratch {
                ifs_bytes, output, ..
            } = scratch;
            output.clear();
            expand_parts_into(ctx, &word.raw, &word.parts, ifs_bytes, false, output)?;
            let mut argv: Vec<Vec<u8>> = Vec::new();
            output.finish_into_no_glob(&mut argv)?;
            Ok(bstr::join_bstrings(&argv, b" "))
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
        return with_scratch(ctx, |ctx, scratch| {
            let output = &mut scratch.output;
            output.clear();
            expand_parts_into(ctx, &word.raw, &word.parts, b"", true, output)?;
            Ok(output.drain_single_vec())
        });
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
        return with_scratch(ctx, |ctx, scratch| {
            let output = &mut scratch.output;
            output.clear();
            expand_parts_into(ctx, &word.raw, &word.parts, b"", true, output)?;
            Ok(output.drain_single_vec())
        });
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

    while index < raw.len() {
        match raw[index] {
            b'\'' => {
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
                index += 1;
                let mut buffer = Vec::new();
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
                index += 1;
            }
            b'\\' => {
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

    Ok(ExpandedWord { segments })
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
    fn parsed_cmd_word(source: &[u8]) -> Word {
        let prog = crate::syntax::parse(source).expect("parse");
        let item = &prog.items[0];
        let cmd = &item.and_or.first.commands[0];
        match cmd {
            crate::syntax::ast::Command::Simple(sc) => sc.words[1].clone(),
            _ => panic!("expected simple command"),
        }
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
    fn expand_word_parsed_tilde_home_empty() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"".to_vec());
        let word = parsed_cmd_word(b"echo ~\n");
        assert!(!word.parts.is_empty());
        let text = expand_word_text(&mut ctx, &word).expect("text");
        assert_eq!(text, b"");
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
    fn expand_word_via_parts_tilde_home_empty() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"".to_vec());
        let word = parts_word(b"echo ~\n");
        let result = expand_word_text(&mut ctx, &word).expect("tilde empty home parts");
        assert_eq!(result, b"");
    }
    #[test]
    fn expand_redirect_word_static_expansion_via_parts() {
        let mut ctx = FakeContext::new();
        let word = parts_word(b"echo $?\n");
        let result = expand_redirect_word(&mut ctx, &word).expect("redirect static");
        assert_eq!(result, b"0");
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
    fn drain_single_vec_via_assignment_star() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = parts_word(b"echo $*\n");
            let result = expand_assignment_value(&mut ctx, &word).expect("assign star");
            assert_eq!(result, b"alpha beta");
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
    fn expand_raw_double_quote_escape_variants() {
        // Inside a double-quoted segment, expand_raw must:
        //   - collapse `\\<newline>`,
        //   - keep `\\z` (non-special) verbatim,
        //   - flush the buffer before `$` and `` ` ``,
        //   - preserve literal newlines.
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"X".to_vec(), b"val".to_vec());

        let cases: &[(&[u8], &[u8])] = &[
            (b"\"pre\\\nsuf\"", b"presuf"),
            (b"\"a\\zb\"", b"a\\zb"),
            (b"\"pre$X-suf\"", b"preval-suf"),
            (b"\"pre`cmd`suf\"", b"precmdsuf"),
            (b"\"a\nb\"", b"a\nb"),
        ];
        for (input, expected) in cases {
            let expanded = expand_raw(&mut ctx, input).expect("expand raw double-quote");
            assert_eq!(
                flatten_segments(&expanded.segments),
                *expected,
                "input={:?}",
                input,
            );
        }

        assert!(expand_raw(&mut ctx, b"\"unterminated").is_err());
        assert!(expand_raw(&mut ctx, b"'unterminated").is_err());
    }

    #[test]
    fn expand_raw_top_level_backslash_and_newline_branches() {
        // Top-level `\\<newline>` increments lineno (preserved); lone trailing
        // backslash at EOF is simply dropped; an unquoted literal newline is
        // kept as a literal segment.
        let mut ctx = FakeContext::new();

        let with_bs_nl = expand_raw(&mut ctx, b"a\\\nb").expect("backslash newline");
        assert_eq!(flatten_segments(&with_bs_nl.segments), b"a\nb");

        let trailing_bs = expand_raw(&mut ctx, b"abc\\").expect("trailing backslash");
        assert_eq!(flatten_segments(&trailing_bs.segments), b"abc");

        let literal_nl = expand_raw(&mut ctx, b"a\nb").expect("literal newline");
        assert_eq!(flatten_segments(&literal_nl.segments), b"a\nb");
    }

    #[test]
    fn expand_raw_tilde_expansion_branches() {
        // Exercises every branch of expand_raw's `~` handler:
        //   - `~` alone with HOME set (no trailing slash trim needed),
        //   - `~/...` with HOME ending in '/' (slash trimmed),
        //   - `~` with HOME unset → literal,
        //   - `~` with HOME set to "" → empty quoted segment,
        //   - `~user` (known) — and with trailing slash trim,
        //   - `~unknown` → literal `~` + user name,
        //   - `~'` (tilde followed by break char without slash) → literal `~`.
        let mut ctx = FakeContext::new();

        ctx.env.insert(b"HOME".to_vec(), b"/h/user".to_vec());
        let plain = expand_raw(&mut ctx, b"~").expect("plain tilde");
        assert_eq!(flatten_segments(&plain.segments), b"/h/user");

        ctx.env.insert(b"HOME".to_vec(), b"/h/".to_vec());
        let trim = expand_raw(&mut ctx, b"~/foo").expect("tilde slash");
        assert_eq!(flatten_segments(&trim.segments), b"/h/foo");

        ctx.env.remove(b"HOME".as_ref());
        let unset = expand_raw(&mut ctx, b"~").expect("tilde no home");
        assert_eq!(flatten_segments(&unset.segments), b"~");

        ctx.env.insert(b"HOME".to_vec(), Vec::new());
        let empty = expand_raw(&mut ctx, b"~").expect("tilde empty home");
        assert_eq!(flatten_segments(&empty.segments), b"");

        ctx.env.insert(b"HOME".to_vec(), b"/h".to_vec());
        let user = expand_raw(&mut ctx, b"~testuser").expect("tilde user");
        assert_eq!(flatten_segments(&user.segments), b"/home/testuser");
        let user_slash = expand_raw(&mut ctx, b"~slashuser/x").expect("tilde user slash");
        // `slashuser` resolves to `/home/slashuser/`; the trailing `/` is
        // trimmed when a `/` follows in the word.
        assert_eq!(flatten_segments(&user_slash.segments), b"/home/slashuser/x");

        let unknown = expand_raw(&mut ctx, b"~nobodyhere").expect("tilde unknown user");
        assert_eq!(flatten_segments(&unknown.segments), b"~nobodyhere");

        let broken = expand_raw(&mut ctx, b"~'abc'").expect("tilde break on quote");
        assert_eq!(flatten_segments(&broken.segments), b"~abc");
    }

    #[test]
    fn expand_raw_bare_dollar_yields_static_dollar() {
        // `$` with no follow byte (or followed by something that is not a
        // valid parameter start) must survive `expand_raw` as a literal `$`
        // via `Expansion::Static(b"$")` routed through `apply_expansion`'s
        // `Static` arm (word.rs apply_expansion).  The segment must be
        // produced with `QuoteState::Expanded` (unquoted context), which
        // `push_segment_slice` records without copying.
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();

            let bare = expand_raw(&mut ctx, b"$").expect("bare dollar");
            assert_eq!(flatten_segments(&bare.segments), b"$");
            assert_eq!(
                bare.segments.as_slice(),
                &[Segment::Text(b"$".to_vec(), QuoteState::Expanded)]
            );

            let trailing = expand_raw(&mut ctx, b"$ ").expect("dollar then space");
            assert_eq!(flatten_segments(&trailing.segments), b"$ ");

            let quoted_bare = expand_raw(&mut ctx, b"\"$\"").expect("bare dollar quoted");
            assert_eq!(flatten_segments(&quoted_bare.segments), b"$");
            assert_eq!(
                quoted_bare.segments.as_slice(),
                &[Segment::Text(b"$".to_vec(), QuoteState::Quoted)]
            );
        });
    }

    #[test]
    fn expand_raw_single_quote_preserves_embedded_newlines() {
        // A literal newline inside `'...'` is part of the quoted text.  The
        // inner loop of `expand_raw`'s `'` branch is the only code path that
        // pushes `\n` through the single-quote state, so this exercises it.
        // The newline-handling branch also calls `ctx.inc_lineno()` — the
        // lineno stays observable-externally-zero through `FakeContext`
        // because we deliberately do not expose line tracking there, so we
        // only assert on the byte result.
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let expanded = expand_raw(&mut ctx, b"'line1\nline2\nline3'").expect("multiline sq");
            assert_eq!(flatten_segments(&expanded.segments), b"line1\nline2\nline3",);
            assert_eq!(
                expanded.segments.as_slice(),
                &[Segment::Text(
                    b"line1\nline2\nline3".to_vec(),
                    QuoteState::Quoted,
                )],
            );
        });
    }

    #[test]
    fn expand_raw_double_quote_trailing_backslash_at_eof_errors() {
        // Raw buffer ending in `"...\\` (no closing quote) exercises the
        // double-quote backslash branch where `index + 1 >= raw.len()`:
        // the byte is appended to the internal buffer, `index` advances
        // past the end of the buffer, and the enclosing loop exits into
        // the unterminated-quote diagnostic.  We assert on the resulting
        // error message to distinguish this from other failure modes.
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let err = expand_raw(&mut ctx, b"\"ab\\").expect_err("no closing quote");
            assert_eq!(&*err.message, b"unterminated double quote");
        });
    }

    #[test]
    fn expand_words_into_skips_truly_empty_word() {
        // `expand_word_into`'s `word.parts.is_empty() && word.raw.is_empty()`
        // early-return must not push anything (the caller relies on this for
        // the "no command words" case).  Driving it through the public
        // `expand_words_into` entry keeps the test free of `pub(super)`
        // plumbing while still exercising the empty-word branch.
        use crate::syntax::word_parts::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let empty = Word {
                raw: Box::from(&b""[..]),
                parts: Box::<[WordPart]>::from(Vec::new()),
                line: 7,
            };
            let populated = Word {
                raw: Box::from(&b"keep"[..]),
                parts: Box::from([WordPart::Literal {
                    start: 0,
                    end: 4,
                    has_glob: false,
                    newlines: 0,
                }]),
                line: 7,
            };
            let mut argv: Vec<Vec<u8>> = Vec::new();
            expand_words_into(&mut ctx, &[empty, populated], &mut argv)
                .expect("mixed empty + populated");
            assert_eq!(argv, vec![b"keep".to_vec()]);
        });
    }
}
