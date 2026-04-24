//! Lexical-context classifier for TAB completion.
//!
//! The completion dispatch in [`super::functions::do_complete`] needs
//! to know the quoting / substitution context of the cursor so it can
//! suppress completion inside single-quoted strings, `#` comments, and
//! right after a trailing backslash — places where the real shell
//! would treat the next byte as literal text, and where dumping
//! filename candidates would be pure noise.
//!
//! This scanner deliberately does NOT reuse [`crate::syntax`]: the
//! real parser returns `ParseError` on unterminated constructs (which
//! is exactly the state we are trying to classify), and its internals
//! are `pub(super)` and entangled with aliases, heredocs, and keyword
//! tables. Instead, we walk the buffer once with a tiny state machine
//! that mirrors the quoting / substitution rules of
//! [`crate::syntax::token`], stopping at `cursor` and returning the
//! state of the top-most nesting frame.
//!
//! If future editor features (syntax highlighting, smarter `M-d`,
//! etc.) need the same information, this module is the right place to
//! promote into a shared helper under `src/syntax/`.

/// The quoting / comment / backslash context immediately to the left
/// of the cursor when TAB is pressed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompletionContext {
    /// Unquoted (including inside any level of unquoted `$(...)` or
    /// backtick substitution). Dispatch the full completion cascade.
    Normal,
    /// Inside a still-open single-quoted string. A literal TAB byte
    /// shall be inserted instead of triggering completion.
    InsideSingleQuote,
    /// Inside a still-open double-quoted string (including nested
    /// unquoted substitutions that never pop back out of the double
    /// quote). The current policy still dispatches the full cascade;
    /// the distinct variant exists so future features (e.g. variable-
    /// only completion inside `"..."`) can branch without a rescan.
    InsideDoubleQuote,
    /// Inside a `#` line comment that has not ended at a newline.
    InsideComment,
    /// Previous byte was an unquoted `\`, which quotes the next byte.
    /// Treated as suppressed so TAB still inserts a literal TAB
    /// instead of completing.
    AfterBackslash,
}

/// One entry in the scanner's nesting stack. `$(` and backtick each
/// push a fresh unquoted frame; the matching closer pops back to the
/// containing frame. Single- and double-quoted frames do not nest
/// further (their substring is treated by their own rules).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameKind {
    Unquoted,
    DoubleQuote,
    /// `$(...)` substitution. Pops on `)`.
    CmdSubParen,
    /// Backtick-wrapped substitution. Pops on `` ` ``.
    CmdSubBacktick,
}

/// Walk `buf[0..cursor]` once and return the lexical state of the top
/// scanner frame at `cursor`. Input beyond `cursor` is never read.
pub(super) fn classify_completion_context(buf: &[u8], cursor: usize) -> CompletionContext {
    let end = cursor.min(buf.len());
    let mut stack: Vec<FrameKind> = vec![FrameKind::Unquoted];
    // Parallel flags for the top frame. They are only meaningful when
    // the top-of-stack is `Unquoted` / `DoubleQuote` / a command-sub
    // frame; they always reset on frame push / pop.
    let mut in_single = false;
    let mut in_comment = false;
    let mut after_backslash = false;
    let mut at_token_start = true;
    let mut i = 0;
    while i < end {
        let b = buf[i];

        if after_backslash {
            after_backslash = false;
            at_token_start = false;
            i += 1;
            continue;
        }

        if in_single {
            if b == b'\'' {
                in_single = false;
                at_token_start = false;
            }
            i += 1;
            continue;
        }

        if in_comment {
            if b == b'\n' {
                in_comment = false;
                at_token_start = true;
            }
            i += 1;
            continue;
        }

        let top = *stack.last().expect("scanner stack never empties");

        match top {
            FrameKind::DoubleQuote => {
                match b {
                    b'"' => {
                        stack.pop();
                        at_token_start = false;
                    }
                    b'\\' => {
                        // Inside double quotes `\` only escapes the
                        // specific bytes `\`, `"`, `$`, `` ` ``, or
                        // newline (matches `consume_double_quote` in
                        // src/syntax/token.rs). Other trailing `\`
                        // pairs are two literal bytes.
                        if i + 1 < end {
                            let next = buf[i + 1];
                            if matches!(next, b'\\' | b'"' | b'$' | b'`' | b'\n') {
                                i += 2;
                                continue;
                            }
                        }
                    }
                    b'$' => {
                        if i + 1 < end && buf[i + 1] == b'(' {
                            stack.push(FrameKind::CmdSubParen);
                            at_token_start = true;
                            i += 2;
                            continue;
                        }
                    }
                    b'`' => {
                        stack.push(FrameKind::CmdSubBacktick);
                        at_token_start = true;
                    }
                    _ => {}
                }
                i += 1;
            }
            FrameKind::Unquoted | FrameKind::CmdSubParen | FrameKind::CmdSubBacktick => {
                match b {
                    b'\\' => {
                        after_backslash = true;
                        at_token_start = false;
                    }
                    b'\'' => {
                        in_single = true;
                        at_token_start = false;
                    }
                    b'"' => {
                        stack.push(FrameKind::DoubleQuote);
                        at_token_start = false;
                    }
                    b'$' => {
                        if i + 1 < end && buf[i + 1] == b'(' {
                            stack.push(FrameKind::CmdSubParen);
                            at_token_start = true;
                            i += 2;
                            continue;
                        }
                        at_token_start = false;
                    }
                    b'`' => {
                        if top == FrameKind::CmdSubBacktick {
                            stack.pop();
                        } else {
                            stack.push(FrameKind::CmdSubBacktick);
                        }
                        at_token_start = true;
                    }
                    b')' => {
                        if top == FrameKind::CmdSubParen {
                            stack.pop();
                        }
                        at_token_start = false;
                    }
                    b'#' if at_token_start => {
                        in_comment = true;
                    }
                    b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' => {
                        at_token_start = true;
                    }
                    _ => {
                        at_token_start = false;
                    }
                }
                i += 1;
            }
        }
    }

    if after_backslash {
        return CompletionContext::AfterBackslash;
    }
    if in_single {
        return CompletionContext::InsideSingleQuote;
    }
    if in_comment {
        return CompletionContext::InsideComment;
    }
    match stack.last().copied() {
        Some(FrameKind::DoubleQuote) => CompletionContext::InsideDoubleQuote,
        _ => CompletionContext::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(buf: &[u8]) -> CompletionContext {
        classify_completion_context(buf, buf.len())
    }

    #[test]
    fn empty_buffer_is_normal() {
        assert_eq!(classify(b""), CompletionContext::Normal);
    }

    #[test]
    fn plain_word_is_normal() {
        assert_eq!(classify(b"echo hello"), CompletionContext::Normal);
    }

    #[test]
    fn trailing_unquoted_backslash_suppresses() {
        assert_eq!(classify(b"echo foo\\"), CompletionContext::AfterBackslash);
    }

    #[test]
    fn backslash_then_char_falls_back_to_normal() {
        assert_eq!(classify(b"echo foo\\bar"), CompletionContext::Normal);
    }

    #[test]
    fn inside_unterminated_single_quote_suppresses() {
        assert_eq!(classify(b"echo 'he"), CompletionContext::InsideSingleQuote);
    }

    #[test]
    fn closed_single_quote_is_normal() {
        assert_eq!(classify(b"echo 'hello' "), CompletionContext::Normal);
    }

    #[test]
    fn inside_unterminated_double_quote_tags_as_double() {
        assert_eq!(classify(b"echo \"he"), CompletionContext::InsideDoubleQuote);
    }

    #[test]
    fn single_quote_inside_double_quote_is_literal() {
        // The `'` is literal inside `"..."`, so the top frame is
        // still the double quote.
        assert_eq!(
            classify(b"echo \"it's"),
            CompletionContext::InsideDoubleQuote
        );
    }

    #[test]
    fn inside_comment_at_token_start_suppresses() {
        assert_eq!(classify(b"# foo bar"), CompletionContext::InsideComment);
        assert_eq!(
            classify(b"echo hi; # bar"),
            CompletionContext::InsideComment
        );
    }

    #[test]
    fn hash_mid_word_is_not_a_comment() {
        assert_eq!(classify(b"echo foo#bar"), CompletionContext::Normal);
    }

    #[test]
    fn comment_ends_at_newline() {
        assert_eq!(classify(b"# foo\nls "), CompletionContext::Normal);
    }

    #[test]
    fn cmd_substitution_unquoted_pushes_frame() {
        assert_eq!(classify(b"echo $(ls"), CompletionContext::Normal);
        assert_eq!(classify(b"echo $(ls)"), CompletionContext::Normal);
    }

    #[test]
    fn single_quote_inside_cmd_substitution_suppresses() {
        assert_eq!(
            classify(b"echo $(echo 'he"),
            CompletionContext::InsideSingleQuote
        );
    }

    #[test]
    fn double_quote_wrapped_cmd_substitution_still_pushes_unquoted_inner() {
        // Top frame is CmdSubParen (Unquoted-like), not DoubleQuote.
        assert_eq!(classify(b"echo \"$(ls"), CompletionContext::Normal);
    }

    #[test]
    fn nested_single_quote_inside_double_quote_cmd_substitution() {
        assert_eq!(
            classify(b"echo \"$(echo 'he"),
            CompletionContext::InsideSingleQuote
        );
    }

    #[test]
    fn backtick_substitution_pushes_and_pops() {
        assert_eq!(classify(b"echo `ls"), CompletionContext::Normal);
        assert_eq!(classify(b"echo `ls` "), CompletionContext::Normal);
        assert_eq!(
            classify(b"echo `echo 'x"),
            CompletionContext::InsideSingleQuote
        );
    }

    #[test]
    fn double_quote_escape_rules() {
        // `\"` keeps us inside the double quote.
        assert_eq!(
            classify(b"echo \"he\\\"ll"),
            CompletionContext::InsideDoubleQuote
        );
        // `\n` (literal backslash + 'n') inside double quote is two
        // literal chars; still inside the double quote.
        assert_eq!(
            classify(b"echo \"he\\nll"),
            CompletionContext::InsideDoubleQuote
        );
    }

    #[test]
    fn closed_double_quote_pops_back_to_unquoted() {
        // The closing `"` pops the DoubleQuote frame, leaving the
        // top-of-stack Unquoted so the final classification is Normal.
        assert_eq!(classify(b"echo \"hi\" "), CompletionContext::Normal);
    }

    #[test]
    fn dollar_in_double_quote_without_paren_stays_literal() {
        // `$x` inside `"..."` has no substitution; the `b'$'` arm
        // exits the "peek for `(`" block without pushing a frame, so
        // the top-of-stack remains DoubleQuote.
        assert_eq!(classify(b"echo \"$x"), CompletionContext::InsideDoubleQuote);
    }

    #[test]
    fn backtick_inside_double_quote_pushes_cmd_sub_frame() {
        // A `` ` `` inside `"..."` pushes a CmdSubBacktick frame
        // whose interior is treated as Unquoted. An opening `'` then
        // suppresses classification as InsideSingleQuote.
        assert_eq!(
            classify(b"echo \"`echo 'he"),
            CompletionContext::InsideSingleQuote
        );
    }

    #[test]
    fn cursor_before_end_respects_limit() {
        // Cursor positioned before the opening `'` means normal.
        assert_eq!(
            classify_completion_context(b"echo 'hello", 5),
            CompletionContext::Normal
        );
        // Cursor right after the `'` means inside single quote.
        assert_eq!(
            classify_completion_context(b"echo 'hello", 6),
            CompletionContext::InsideSingleQuote
        );
    }
}
