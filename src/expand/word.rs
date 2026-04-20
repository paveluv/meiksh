use crate::bstr;
use crate::syntax::ast::Word;
use crate::syntax::word_part::WordPart;

use super::core::{Context, ExpandError};
use super::expand_parts::{
    ExpandOutput, decompose_ifs_into, expand_parts_into, expand_parts_into_mode,
};
use super::model::render_pattern_from_segments;
use super::parameter::expand_parameter_dollar;
use super::scratch::ExpandScratch;

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

/// Take the context's pooled `ExpandScratch` out of `ctx`, run `body`
/// with it, and always put it back (even on error). Nested expansion
/// calls on the same `ctx` during `body` observe a `None` slot and
/// construct a fresh local scratch; they never panic, and the outer
/// pooled scratch is protected from re-entrant mutation.
///
/// Using an `Option` slot (rather than `std::mem::take` with a
/// `Default`-constructed placeholder) avoids an `ExpandScratch`
/// drop on every expansion — each `ExpandScratch` owns several
/// `Vec`s whose allocators would otherwise fire on the placeholder
/// drop.
fn with_scratch<C, R>(
    ctx: &mut C,
    body: impl FnOnce(&mut C, &mut ExpandScratch) -> Result<R, ExpandError>,
) -> Result<R, ExpandError>
where
    C: Context,
{
    match ctx.expand_scratch_slot_mut().take() {
        Some(mut scratch) => {
            let result = body(ctx, &mut scratch);
            *ctx.expand_scratch_slot_mut() = Some(scratch);
            result
        }
        None => {
            // Re-entrant: outer frame already owns the pooled scratch.
            // Use a fresh one so we do not disturb the outer state.
            let mut scratch = ExpandScratch::default();
            body(ctx, &mut scratch)
        }
    }
}

/// Ensure `scratch.ifs_bytes` holds the current `$IFS`. Cached across
/// calls because IFS is read on every simple command but rarely mutated;
/// [`ExpandScratch::invalidate_ifs`] is called from `set_var` / `unset_var`
/// whenever `IFS` is touched.
///
/// Split into a trivial inlineable fast path (cached) and an outlined
/// slow path (cache miss) so the hot case is a single branch at the
/// call site.
#[inline]
fn ensure_ifs_cached<C: Context>(ctx: &C, scratch: &mut ExpandScratch) {
    if scratch.ifs_valid {
        return;
    }
    ensure_ifs_cached_cold(ctx, scratch);
}

#[cold]
#[inline(never)]
fn ensure_ifs_cached_cold<C: Context>(ctx: &C, scratch: &mut ExpandScratch) {
    scratch.ifs_bytes.clear();
    match ctx.env_var(b"IFS") {
        Some(c) => scratch.ifs_bytes.extend_from_slice(&c),
        None => scratch.ifs_bytes.extend_from_slice(b" \t\n"),
    }
    scratch.ifs_chars.clear();
    decompose_ifs_into(&scratch.ifs_bytes, &mut scratch.ifs_chars);
    scratch.ifs_valid = true;
}

fn expand_word_with_scratch<C: Context>(
    ctx: &mut C,
    word: &Word,
    scratch: &mut ExpandScratch,
    argv: &mut Vec<Vec<u8>>,
) -> Result<(), ExpandError> {
    // Move `scratch.output` out so `scratch` can be threaded alongside
    // the main output buffer without aliasing. The taken value is
    // placed back before returning (even on error).
    let mut output = scratch.output.take().unwrap_or_default();
    let result = expand_word_into(ctx, word, &mut output, scratch, argv);
    scratch.output = Some(output);
    result
}

/// Expand an argv word that the parser marked as a declaration-utility
/// assignment (i.e. `parts[0]` is a `Literal { assignment: true, .. }`
/// carrying the `NAME=` prefix, followed by the value parts).
///
/// The parser has already placed `TildeLiteral` parts at all
/// assignment-context tilde positions (word-start after `=` and after
/// each unquoted `:`), so the expander is a pure structural walk.
pub(crate) fn expand_word_as_declaration_assignment<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    let Some(WordPart::Literal {
        start,
        end,
        assignment: true,
        ..
    }) = word.parts.first()
    else {
        // Parser invariant: `word_is_assignment` gated this call, so the
        // first part is always a `Literal` with `assignment: true`. Fall
        // back to ordinary word expansion for defensive safety.
        return expand_word_text(ctx, word);
    };
    let name_bytes = &word.raw[*start..*end];
    let value_parts = &word.parts[1..];
    with_scratch(ctx, |ctx, scratch| {
        let mut output = scratch.output.take().unwrap_or_default();
        output.clear();
        let result = expand_parts_into_mode(
            ctx,
            &word.raw,
            value_parts,
            true,
            true,
            &mut output,
            scratch,
        );
        let value = match &result {
            Ok(()) => output.drain_single_vec_pooled(ctx.bytes_pool_mut()),
            Err(_) => Vec::new(),
        };
        scratch.output = Some(output);
        result.map(|()| {
            // Declaration-utility assignment words like `export FOO=bar`
            // produce a concatenated `NAME=value` for the child exec;
            // the combined buffer is one allocation per word. Recycle
            // the `value` buffer we borrowed from the pool so it can
            // serve the next expansion.
            let mut combined = ctx.bytes_pool_mut().take();
            combined.reserve(name_bytes.len() + value.len());
            combined.extend_from_slice(name_bytes);
            combined.extend_from_slice(&value);
            ctx.bytes_pool_mut().recycle(value);
            combined
        })
    })
}

/// True iff `word`'s first part is a `Literal` carrying the parser-set
/// `assignment: true` flag. The parser decides at AST-build time whether
/// a given argv word belongs to a declaration-utility call; this check
/// is a pure flag read.
pub(crate) fn word_is_assignment(word: &Word) -> bool {
    matches!(
        word.parts.first(),
        Some(WordPart::Literal {
            assignment: true,
            ..
        })
    )
}

fn expand_word_into<C: Context>(
    ctx: &mut C,
    word: &Word,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
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
    // single owned byte vector directly into argv. The buffer is
    // pulled from the shared `BytesPool` so a warmed-up shell avoids
    // a heap allocation per literal word.
    if let [
        WordPart::Literal {
            start: 0,
            end,
            has_glob: false,
            newlines: 0,
            ..
        },
    ] = &word.parts[..]
        && *end == word.raw.len()
    {
        if !word.raw.is_empty() {
            let mut buf = ctx.bytes_pool_mut().take();
            buf.extend_from_slice(&word.raw);
            argv.push(buf);
        }
        return Ok(());
    }

    // Slow path: make sure `output.current` has a real backing
    // allocation before any `extend_from_slice` / `push` fires. The
    // `push_current_field` path inside field-splitting routinely leaves
    // `self.current` as `Vec::new()` (cap = 0) after a drain; refilling
    // from the pool here keeps the first byte-level push off the
    // first-grow allocator path.
    if output.current.capacity() == 0 {
        output.current = ctx.bytes_pool_mut().take();
    }
    output.clear();
    super::expand_parts::expand_parts_into(ctx, &word.raw, &word.parts, false, output, scratch)?;
    output.finish_into(ctx, argv)
}

pub(crate) fn expand_redirect_word<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    with_scratch(ctx, |ctx, scratch| {
        ensure_ifs_cached(ctx, scratch);
        let mut output = scratch.output.take().unwrap_or_default();
        output.clear();
        let result = expand_parts_into(ctx, &word.raw, &word.parts, false, &mut output, scratch);
        let joined = match &result {
            Ok(()) => {
                let mut argv: Vec<Vec<u8>> = Vec::new();
                output
                    .finish_into_no_glob(&mut argv)
                    .map(|()| bstr::join_bstrings(&argv, b" "))
            }
            Err(_) => Ok(Vec::new()),
        };
        scratch.output = Some(output);
        result?;
        joined
    })
}

pub(crate) fn expand_word_text<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    with_scratch(ctx, |ctx, scratch| {
        let mut output = scratch.output.take().unwrap_or_default();
        output.clear();
        let result = expand_parts_into_mode(
            ctx,
            &word.raw,
            &word.parts,
            true,
            true,
            &mut output,
            scratch,
        );
        let value = match &result {
            Ok(()) => output.drain_single_vec_pooled(ctx.bytes_pool_mut()),
            Err(_) => Vec::new(),
        };
        scratch.output = Some(output);
        result.map(|()| value)
    })
}

pub(crate) fn expand_word_pattern<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    with_scratch(ctx, |ctx, scratch| {
        let mut segments = std::mem::take(&mut scratch.pattern_segments);
        segments.clear();
        let result = super::expand_parts::build_pattern_segments(
            ctx,
            &word.raw,
            &word.parts,
            &mut segments,
            scratch,
        );
        let pattern = match &result {
            Ok(()) => render_pattern_from_segments(&segments),
            Err(_) => Vec::new(),
        };
        scratch.pattern_segments = segments;
        result.map(|()| pattern)
    })
}

pub(crate) fn expand_assignment_value<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(word.line);
    with_scratch(ctx, |ctx, scratch| {
        let mut output = scratch.output.take().unwrap_or_default();
        output.clear();
        let result = expand_parts_into_mode(
            ctx,
            &word.raw,
            &word.parts,
            true,
            true,
            &mut output,
            scratch,
        );
        // Pool-aware drain: for the arith / assignment / here-doc hot
        // paths `fields` is empty, so this is a single `mem::replace`
        // that swaps a pre-sized buffer from the pool into
        // `output.current` and hands the previously accumulated bytes
        // to the caller. The caller stashes the returned `Vec<u8>` in
        // an `ExecScratch::assignments` entry or in the env map; either
        // way it eventually flows back into `BytesPool` via
        // `ExecScratch::clear_into_pool`, completing the zero-malloc
        // loop.
        let value = match &result {
            Ok(()) => output.drain_single_vec_pooled(ctx.bytes_pool_mut()),
            Err(_) => Vec::new(),
        };
        scratch.output = Some(output);
        result.map(|()| value)
    })
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

pub(super) fn trim_trailing_newlines(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    while end > 0 && s[end - 1] == b'\n' {
        end -= 1;
    }
    &s[..end]
}

pub(crate) fn expand_here_document<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    body_line: usize,
) -> Result<Vec<u8>, ExpandError> {
    ctx.set_lineno(body_line);
    with_scratch(ctx, |ctx, scratch| {
        let mut output = scratch.output.take().unwrap_or_default();
        output.clear();
        let result = expand_parts_into_mode(ctx, raw, parts, true, true, &mut output, scratch);
        let value = match &result {
            Ok(()) => output.drain_single_vec_pooled(ctx.bytes_pool_mut()),
            Err(_) => Vec::new(),
        };
        scratch.output = Some(output);
        result.map(|()| value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::arithmetic::eval_arithmetic;
    use crate::expand::core::Context;
    use crate::expand::parameter::lookup_param;
    use crate::expand::test_support::FakeContext;
    use crate::syntax::ast::Word;
    use crate::sys::test_support::assert_no_syscalls;

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
    fn lookup_param_covers_name_and_positional() {
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
    }

    #[test]
    fn here_document_expands_at_sign() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
        let body = b"$@\n";
        let parts = crate::syntax::build_heredoc_parts(body);
        let result = expand_here_document(&mut ctx, body, &parts, 0).expect("heredoc at");
        assert_eq!(result, b"x y\n");
    }
    #[test]
    fn here_document_with_at_expansion() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        let body = b"args: $@\n";
        let parts = crate::syntax::build_heredoc_parts(body);
        let result = expand_here_document(&mut ctx, body, &parts, 0).expect("heredoc @");
        assert_eq!(result, b"args: a b\n");
    }

    #[test]
    fn here_doc_backtick_substitution() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let body = b"`echo ok`\n";
            let parts = crate::syntax::build_heredoc_parts(body);
            let result =
                expand_here_document(&mut ctx, body, &parts, 0).expect("here doc backtick");
            assert_eq!(result, b"echo ok\n");
        });
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
        // In an assignment value context, `$@` is a single-field
        // expansion: POSIX specifies that its elements are joined with
        // the first character of IFS (space by default), matching `$*`.
        // This covers the `single_field` branch of `expand_special_var`
        // for `ch == b'@'`.
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a".to_vec(), b"b".to_vec()];
        let word = parts_word(b"echo $@\n");
        let result = expand_assignment_value(&mut ctx, &word).expect("assign at");
        assert_eq!(result, b"a b");
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
        use crate::syntax::word_part::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let word = Word {
                raw: b"\"\"".to_vec(),
                parts: vec![WordPart::QuotedLiteral {
                    bytes: Vec::new(),
                    newlines: 0,
                }],
                line: 0,
            };
            let result = expand_redirect_word(&mut ctx, &word).expect("redirect empty quoted");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn expand_words_into_skips_truly_empty_word() {
        // `expand_word_into`'s `word.parts.is_empty() && word.raw.is_empty()`
        // early-return must not push anything (the caller relies on this for
        // the "no command words" case).  Driving it through the public
        // `expand_words_into` entry keeps the test free of `pub(super)`
        // plumbing while still exercising the empty-word branch.
        use crate::syntax::word_part::WordPart;
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let empty = Word {
                raw: Vec::new(),
                parts: Vec::new(),
                line: 7,
            };
            let populated = Word {
                raw: b"keep".to_vec(),
                parts: vec![WordPart::Literal {
                    start: 0,
                    end: 4,
                    has_glob: false,
                    newlines: 0,
                    assignment: false,
                }],
                line: 7,
            };
            let mut argv: Vec<Vec<u8>> = Vec::new();
            expand_words_into(&mut ctx, &[empty, populated], &mut argv)
                .expect("mixed empty + populated");
            assert_eq!(argv, vec![b"keep".to_vec()]);
        });
    }
}
