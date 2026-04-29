//! Bash-compatible quoting and dequoting for completion candidates.
//!
//! When [`super::functions::do_complete`] inserts a candidate back into
//! the line buffer, any shell metacharacter inside the candidate must
//! be escaped or wrapped in matching quotes — otherwise the parser
//! would re-tokenize a candidate like `foo bar` as two arguments and
//! re-expand `baz$qux` as a parameter substitution. The required
//! transformation depends on the lexical context to the left of the
//! cursor (see [`super::completion_context`]):
//!
//! * **BSQUOTE** ([`QuoteMode::Bsquote`]) — cursor in `Normal` context.
//!   Backslash-escape every byte from [`FILENAME_QUOTE_CHARS`].
//! * **DQUOTE** ([`QuoteMode::Dquote`]) — cursor inside an open `"..."`
//!   string. Only `\`, `"`, `$`, `` ` `` need escaping; the surrounding
//!   double quotes neutralize everything else.
//! * **SQUOTE** ([`QuoteMode::Squote`]) — cursor inside an open `'...'`
//!   string. Single quotes disable every shell metacharacter, so only
//!   the embedded `'` needs the standard `'\''` close-escape-reopen
//!   sequence.
//!
//! In addition to re-quoting, BSQUOTE mode also requires *prefix
//! dequoting*: the user may already have typed `foo\ ba` and the
//! matching directory contains `foo bar`. The partial word is dequoted
//! to `foo ba` before comparing against directory entries; the matched
//! candidate is then re-quoted to `foo\ bar` on insertion.
//!
//! The character set used by BSQUOTE matches bash 5.x's
//! `default_filename_quote_characters` (see `bashline.c`).

#![allow(clippy::disallowed_macros)]

/// The 25-byte character set that BSQUOTE mode backslash-escapes when
/// inserting a candidate. Matches bash 5.x's
/// `default_filename_quote_characters`.
pub(super) const FILENAME_QUOTE_CHARS: &[u8] = b" \t\n\\\"'@<>=;|&()#$`?*[!:{~";

/// Re-quoting mode for a completion candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QuoteMode {
    /// Cursor in `Normal` context — backslash-escape every byte from
    /// [`FILENAME_QUOTE_CHARS`].
    Bsquote,
    /// Cursor inside an open `"..."` — escape only `\ " $ ` `.
    Dquote,
    /// Cursor inside an open `'...'` — encode `'` as `'\''` and emit
    /// every other byte as-is.
    Squote,
}

/// Return `true` if `b` is a member of [`FILENAME_QUOTE_CHARS`].
fn is_filename_quote_char(b: u8) -> bool {
    FILENAME_QUOTE_CHARS.contains(&b)
}

/// Strip BSQUOTE-style backslash escapes from a partial word so it
/// can be matched against on-disk filenames.
///
/// Each unquoted `\X` becomes the literal `X`. A trailing dangling
/// backslash is dropped (matches bash's behavior of treating it as a
/// line-continuation that completion can ignore).
pub(super) fn bsquote_dequote(prefix: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(prefix.len());
    let mut i = 0;
    while i < prefix.len() {
        let b = prefix[i];
        if b == b'\\' {
            if i + 1 < prefix.len() {
                out.push(prefix[i + 1]);
                i += 2;
                continue;
            } else {
                // Dangling `\` at the end: drop it.
                break;
            }
        }
        out.push(b);
        i += 1;
    }
    out
}

/// Re-quote `candidate` so it can be spliced into the line buffer
/// without changing how the parser tokenizes the surrounding command.
pub(super) fn quote_filename(candidate: &[u8], mode: QuoteMode) -> Vec<u8> {
    match mode {
        QuoteMode::Bsquote => quote_bsquote(candidate),
        QuoteMode::Dquote => quote_dquote(candidate),
        QuoteMode::Squote => quote_squote(candidate),
    }
}

fn quote_bsquote(candidate: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(candidate.len());
    for &b in candidate {
        if is_filename_quote_char(b) {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

fn quote_dquote(candidate: &[u8]) -> Vec<u8> {
    // Inside `"..."` only `\ " $ ` ` retain their special meaning, per
    // POSIX § 2.2.3 "Double-Quotes".
    let mut out = Vec::with_capacity(candidate.len());
    for &b in candidate {
        if matches!(b, b'\\' | b'"' | b'$' | b'`') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

fn quote_squote(candidate: &[u8]) -> Vec<u8> {
    // Inside `'...'` no character is special. The only way to encode a
    // literal `'` is to close the quote, emit a backslash-escaped `'`,
    // and reopen the quote: `'\''`. Per POSIX § 2.2.2 "Single-Quotes".
    let mut out = Vec::with_capacity(candidate.len());
    for &b in candidate {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_types, clippy::disallowed_macros)]
    use super::*;

    // --- bsquote_dequote --------------------------------------------------

    #[test]
    fn bsquote_dequote_empty_returns_empty() {
        assert_eq!(bsquote_dequote(b""), b"");
    }

    #[test]
    fn bsquote_dequote_no_escapes_is_identity() {
        assert_eq!(bsquote_dequote(b"foo_bar.txt"), b"foo_bar.txt");
    }

    #[test]
    fn bsquote_dequote_strips_backslash_escaped_space() {
        assert_eq!(bsquote_dequote(b"foo\\ bar"), b"foo bar");
    }

    #[test]
    fn bsquote_dequote_strips_double_backslash() {
        assert_eq!(bsquote_dequote(b"foo\\\\bar"), b"foo\\bar");
    }

    #[test]
    fn bsquote_dequote_drops_trailing_backslash() {
        assert_eq!(bsquote_dequote(b"foo\\"), b"foo");
    }

    #[test]
    fn bsquote_dequote_strips_escaped_dollar() {
        assert_eq!(bsquote_dequote(b"baz\\$qux"), b"baz$qux");
    }

    #[test]
    fn bsquote_dequote_strips_escaped_quote() {
        assert_eq!(bsquote_dequote(b"x\\'y"), b"x'y");
    }

    // --- quote_filename: BSQUOTE -----------------------------------------

    #[test]
    fn bsquote_alphanumerics_unchanged() {
        assert_eq!(
            quote_filename(b"FooBar_42.txt", QuoteMode::Bsquote),
            b"FooBar_42.txt"
        );
    }

    #[test]
    fn bsquote_escapes_space() {
        assert_eq!(quote_filename(b"foo bar", QuoteMode::Bsquote), b"foo\\ bar");
    }

    #[test]
    fn bsquote_escapes_dollar() {
        assert_eq!(quote_filename(b"baz$qux", QuoteMode::Bsquote), b"baz\\$qux");
    }

    #[test]
    fn bsquote_escapes_glob_metacharacters() {
        assert_eq!(quote_filename(b"a*b", QuoteMode::Bsquote), b"a\\*b");
        assert_eq!(quote_filename(b"a?b", QuoteMode::Bsquote), b"a\\?b");
        assert_eq!(quote_filename(b"a[b", QuoteMode::Bsquote), b"a\\[b");
    }

    #[test]
    fn bsquote_escapes_quotes_and_backslash() {
        assert_eq!(quote_filename(b"a'b", QuoteMode::Bsquote), b"a\\'b");
        assert_eq!(quote_filename(b"a\"b", QuoteMode::Bsquote), b"a\\\"b");
        assert_eq!(quote_filename(b"a\\b", QuoteMode::Bsquote), b"a\\\\b");
    }

    #[test]
    fn bsquote_escapes_leading_tilde() {
        assert_eq!(quote_filename(b"~tilde", QuoteMode::Bsquote), b"\\~tilde");
    }

    #[test]
    fn bsquote_escapes_command_separators() {
        assert_eq!(quote_filename(b"a;b", QuoteMode::Bsquote), b"a\\;b");
        assert_eq!(quote_filename(b"a&b", QuoteMode::Bsquote), b"a\\&b");
        assert_eq!(quote_filename(b"a|b", QuoteMode::Bsquote), b"a\\|b");
        assert_eq!(quote_filename(b"a(b", QuoteMode::Bsquote), b"a\\(b");
        assert_eq!(quote_filename(b"a)b", QuoteMode::Bsquote), b"a\\)b");
    }

    #[test]
    fn bsquote_escapes_redirection_chars() {
        assert_eq!(quote_filename(b"a<b", QuoteMode::Bsquote), b"a\\<b");
        assert_eq!(quote_filename(b"a>b", QuoteMode::Bsquote), b"a\\>b");
    }

    #[test]
    fn bsquote_escapes_backtick_and_hash() {
        assert_eq!(quote_filename(b"a`b", QuoteMode::Bsquote), b"a\\`b");
        assert_eq!(quote_filename(b"a#b", QuoteMode::Bsquote), b"a\\#b");
    }

    #[test]
    fn bsquote_escapes_every_byte_in_quote_set() {
        for &b in FILENAME_QUOTE_CHARS {
            let cand = vec![b];
            let quoted = quote_filename(&cand, QuoteMode::Bsquote);
            assert_eq!(
                quoted,
                vec![b'\\', b],
                "byte 0x{b:02x} should be backslash-escaped"
            );
        }
    }

    #[test]
    fn bsquote_leaves_safe_punctuation_alone() {
        // `/`, `.`, `-`, `_`, `+`, `,`, `^`, `%`, `]`, `}` are NOT in
        // the bash quote set: they're inserted bare.
        for &b in b"/.-_+,^%]}" {
            let cand = vec![b];
            let quoted = quote_filename(&cand, QuoteMode::Bsquote);
            assert_eq!(
                quoted,
                vec![b],
                "byte 0x{b:02x} should pass through unmodified"
            );
        }
    }

    // --- quote_filename: DQUOTE ------------------------------------------

    #[test]
    fn dquote_leaves_space_bare() {
        assert_eq!(quote_filename(b"foo bar", QuoteMode::Dquote), b"foo bar");
    }

    #[test]
    fn dquote_escapes_dollar() {
        assert_eq!(quote_filename(b"baz$qux", QuoteMode::Dquote), b"baz\\$qux");
    }

    #[test]
    fn dquote_escapes_double_quote_and_backslash() {
        assert_eq!(quote_filename(b"a\"b", QuoteMode::Dquote), b"a\\\"b");
        assert_eq!(quote_filename(b"a\\b", QuoteMode::Dquote), b"a\\\\b");
    }

    #[test]
    fn dquote_escapes_backtick() {
        assert_eq!(quote_filename(b"a`b", QuoteMode::Dquote), b"a\\`b");
    }

    #[test]
    fn dquote_leaves_single_quote_and_glob_chars_alone() {
        assert_eq!(quote_filename(b"x'y", QuoteMode::Dquote), b"x'y");
        assert_eq!(quote_filename(b"a*b", QuoteMode::Dquote), b"a*b");
        assert_eq!(quote_filename(b"a?b", QuoteMode::Dquote), b"a?b");
        assert_eq!(quote_filename(b"a[b", QuoteMode::Dquote), b"a[b");
    }

    // --- quote_filename: SQUOTE ------------------------------------------

    #[test]
    fn squote_leaves_dollar_alone() {
        assert_eq!(quote_filename(b"baz$qux", QuoteMode::Squote), b"baz$qux");
    }

    #[test]
    fn squote_leaves_double_quote_and_backslash_alone() {
        assert_eq!(quote_filename(b"a\"b", QuoteMode::Squote), b"a\"b");
        assert_eq!(quote_filename(b"a\\b", QuoteMode::Squote), b"a\\b");
    }

    #[test]
    fn squote_leaves_glob_metacharacters_alone() {
        assert_eq!(quote_filename(b"a*b", QuoteMode::Squote), b"a*b");
        assert_eq!(quote_filename(b"a?b", QuoteMode::Squote), b"a?b");
    }

    #[test]
    fn squote_encodes_embedded_single_quote_with_close_escape_reopen() {
        assert_eq!(quote_filename(b"x'y", QuoteMode::Squote), b"x'\\''y");
    }

    #[test]
    fn squote_encodes_consecutive_single_quotes_individually() {
        assert_eq!(quote_filename(b"a''b", QuoteMode::Squote), b"a'\\'''\\''b");
    }

    #[test]
    fn squote_handles_only_quote() {
        assert_eq!(quote_filename(b"'", QuoteMode::Squote), b"'\\''");
    }
}
