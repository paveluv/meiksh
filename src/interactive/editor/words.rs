//! Word-boundary helpers shared between the vi and emacs editors.
//!
//! Two distinct "word" notions appear in the specs:
//!
//! * [`WordClass::AlnumUnderscore`] — alphanumerics plus `_`. Used by
//!   emacs `M-f` / `M-b` / `M-d` / `M-DEL` and by vi's lowercase word
//!   motions.
//! * [`WordClass::Whitespace`] — anything non-whitespace counts as
//!   part of a word. Used by emacs `C-w` (`unix-word-rubout`, spec
//!   5.4) and by vi's uppercase word motions.
//!
//! All byte offsets returned by this module are multibyte-safe: the
//! input is treated as a sequence of locale-decoded characters via
//! [`crate::sys::locale::decode_char`].

use crate::sys;

use super::redraw::{char_len_at, prev_char_start};

/// Character-class selector for word motions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WordClass {
    /// Alphanumeric + underscore word characters. This matches the
    /// emacs `M-*` family (spec 5.2) and POSIX [:alnum:].
    AlnumUnderscore,
    /// Non-whitespace runs. This matches emacs `C-w`
    /// (`unix-word-rubout`, spec 5.4) and vi's uppercase word motions.
    Whitespace,
}

fn is_word_char_wc(wc: u32) -> bool {
    if wc == b'_' as u32 {
        return true;
    }
    sys::locale::classify_char(b"alnum", wc)
}

pub(crate) fn is_word_char_at(line: &[u8], pos: usize) -> bool {
    let (wc, _) = sys::locale::decode_char(&line[pos..]);
    is_word_char_wc(wc)
}

pub(crate) fn is_ws_at(line: &[u8], pos: usize) -> bool {
    let b = line[pos];
    b == b' ' || b == b'\t' || b == b'\n'
}

pub(crate) fn is_word_char_before(line: &[u8], pos: usize) -> bool {
    is_word_char_at(line, prev_char_start(line, pos))
}

pub(crate) fn is_ws_before(line: &[u8], pos: usize) -> bool {
    is_ws_at(line, prev_char_start(line, pos))
}

/// Move forward from `pos` to the *next* word boundary per `class`.
///
/// For [`WordClass::AlnumUnderscore`] this matches GNU Readline's
/// `forward-word`: skip any current word, then skip over non-word
/// characters, landing on the first character of the next word (or
/// end-of-line).
///
/// For [`WordClass::Whitespace`] the motion is vi-style:
/// skip the current non-whitespace run and the following whitespace.
pub(crate) fn next_word_boundary(line: &[u8], pos: usize, class: WordClass) -> usize {
    let mut p = pos;
    let len = line.len();
    match class {
        WordClass::AlnumUnderscore => {
            if p >= len {
                return p;
            }
            if is_word_char_at(line, p) {
                while p < len && is_word_char_at(line, p) {
                    p += char_len_at(line, p);
                }
            } else if !is_ws_at(line, p) {
                while p < len && !is_word_char_at(line, p) && !is_ws_at(line, p) {
                    p += char_len_at(line, p);
                }
            }
            while p < len && is_ws_at(line, p) {
                p += char_len_at(line, p);
            }
            p
        }
        WordClass::Whitespace => {
            while p < len && !is_ws_at(line, p) {
                p += char_len_at(line, p);
            }
            while p < len && is_ws_at(line, p) {
                p += char_len_at(line, p);
            }
            p
        }
    }
}

/// Move backward from `pos` to the previous word boundary per `class`.
///
/// The rules mirror [`next_word_boundary`] reading right-to-left:
/// skip trailing whitespace, then walk back over a contiguous run of
/// characters of the relevant class.
pub(crate) fn prev_word_boundary(line: &[u8], pos: usize, class: WordClass) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos;
    match class {
        WordClass::AlnumUnderscore => {
            while p > 0 && is_ws_before(line, p) {
                p = prev_char_start(line, p);
            }
            if p == 0 {
                return 0;
            }
            if is_word_char_before(line, p) {
                while p > 0 && is_word_char_before(line, p) {
                    p = prev_char_start(line, p);
                }
            } else {
                while p > 0 && !is_word_char_before(line, p) && !is_ws_before(line, p) {
                    p = prev_char_start(line, p);
                }
            }
            p
        }
        WordClass::Whitespace => {
            while p > 0 && is_ws_before(line, p) {
                p = prev_char_start(line, p);
            }
            while p > 0 && !is_ws_before(line, p) {
                p = prev_char_start(line, p);
            }
            p
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{assert_no_syscalls, set_test_locale_c, set_test_locale_utf8};

    #[test]
    fn next_word_boundary_alnum_c_locale() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(
                next_word_boundary(b"foo bar", 0, WordClass::AlnumUnderscore),
                4
            );
            assert_eq!(
                next_word_boundary(b"foo bar", 4, WordClass::AlnumUnderscore),
                7
            );
            assert_eq!(
                next_word_boundary(b"foo.bar", 0, WordClass::AlnumUnderscore),
                3
            );
        });
    }

    #[test]
    fn next_word_boundary_whitespace_vs_alnum() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(
                next_word_boundary(b"foo.bar baz", 0, WordClass::Whitespace),
                8
            );
            assert_eq!(
                next_word_boundary(b"foo.bar baz", 0, WordClass::AlnumUnderscore),
                3
            );
        });
    }

    #[test]
    fn prev_word_boundary_alnum_c_locale() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(
                prev_word_boundary(b"foo bar", 7, WordClass::AlnumUnderscore),
                4
            );
            assert_eq!(
                prev_word_boundary(b"foo bar", 3, WordClass::AlnumUnderscore),
                0
            );
        });
    }

    #[test]
    fn next_word_boundary_paired_locales_on_multibyte() {
        let line = b"ab\xc3\xa9cd ef"; // "abécd ef"
        let boundary_c = assert_no_syscalls(|| {
            set_test_locale_c();
            next_word_boundary(line, 0, WordClass::AlnumUnderscore)
        });
        let boundary_utf8 = assert_no_syscalls(|| {
            set_test_locale_utf8();
            next_word_boundary(line, 0, WordClass::AlnumUnderscore)
        });
        // In C locale the 0xC3 byte is not alnum, so the walk stops
        // inside the multibyte run. In UTF-8 the accented letter is
        // alnum, so the walk continues to the space.
        assert!(boundary_c < boundary_utf8);
    }

    #[test]
    fn prev_word_boundary_paired_locales_on_multibyte() {
        let line = b"ab\xc3\xa9cd ef";
        let pos = line.len();
        let b_c = assert_no_syscalls(|| {
            set_test_locale_c();
            prev_word_boundary(line, pos, WordClass::AlnumUnderscore)
        });
        let b_utf8 = assert_no_syscalls(|| {
            set_test_locale_utf8();
            prev_word_boundary(line, pos, WordClass::AlnumUnderscore)
        });
        assert_eq!(b_c, b_utf8); // both land on 'e' start of "ef"
        // both agree on the trailing word
        assert_eq!(b_c, 7);
    }
}
