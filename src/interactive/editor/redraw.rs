//! Cursor-math and redraw helpers shared between the vi and emacs
//! editors. All public helpers are locale-aware: byte offsets into the
//! buffer are treated as multibyte-safe indices backed by
//! [`crate::sys::locale::decode_char`], so UTF-8 input renders with
//! correct column widths in the prompt.
//!
//! The [`redraw`] helper intentionally writes to `stdout` (buffer
//! bytes) and `stderr` (prompt). Keeping the prompt on `stderr`
//! matches the legacy vi-editor contract so pipelines that capture
//! `stdout` don't end up with the prompt interleaved.

use crate::bstr;
use crate::sys;

use super::input::write_bytes;

/// Compute the visual column width of a byte slice using the current
/// locale's `wcwidth` mapping. Invalid sequences count one column each,
/// matching POSIX terminal behavior where stray bytes render as a
/// single cell.
pub(crate) fn display_width(line: &[u8]) -> usize {
    let mut w = 0;
    let mut i = 0;
    while i < line.len() {
        let (wc, len) = sys::locale::decode_char(&line[i..]);
        let step = if len == 0 { 1 } else { len };
        w += sys::locale::char_width(wc);
        i += step;
    }
    w
}

/// Visual width of the slice `line[from..to]`. Used to compute
/// cursor-back offsets after redraw.
pub(crate) fn display_width_range(line: &[u8], from: usize, to: usize) -> usize {
    if to <= from {
        return 0;
    }
    display_width(&line[from..to])
}

/// Byte length of the multibyte character starting at `pos`. Returns
/// 1 for invalid / ASCII bytes and for out-of-range positions.
pub(crate) fn char_len_at(line: &[u8], pos: usize) -> usize {
    if pos >= line.len() {
        return 0;
    }
    let (_, len) = sys::locale::decode_char(&line[pos..]);
    if len == 0 { 1 } else { len }
}

/// Byte offset of the character *before* `pos`. For a UTF-8 line, this
/// walks back over continuation bytes. For ASCII or invalid input, it
/// yields `pos - 1`.
pub(crate) fn prev_char_start(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while p > 0 && (line[p] & 0xC0) == 0x80 {
        p -= 1;
    }
    p
}

/// Byte offset of the last character in `line`. Zero for empty input.
pub(crate) fn last_char_start(line: &[u8]) -> usize {
    if line.is_empty() {
        return 0;
    }
    prev_char_start(line, line.len())
}

/// Emit the full redraw sequence: carriage-return, clear-to-end-of-
/// line, prompt on stderr, buffer on stdout, and a cursor-back escape
/// if the cursor is not at the end.
///
/// Keeping prompt and buffer on separate streams matches the legacy
/// behavior of `vi_editing::redraw`; downstream consumers (tmux, bash
/// transcripts) rely on that split.
pub(crate) fn redraw(line: &[u8], cursor: usize, prompt: &[u8]) {
    write_bytes(b"\r\x1b[K");
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt);
    let mut buf = Vec::with_capacity(line.len() + 20);
    buf.extend_from_slice(line);
    let cursor_back = display_width_range(line, cursor, line.len());
    if cursor_back > 0 {
        buf.extend_from_slice(b"\x1b[");
        bstr::push_u64(&mut buf, cursor_back as u64);
        buf.push(b'D');
    }
    write_bytes(&buf);
}

/// Build the control sequence [`redraw`] would emit, without touching
/// any file descriptor. Useful for unit tests that assert the produced
/// bytes directly.
pub(crate) fn redraw_sequence(line: &[u8], cursor: usize, prompt: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut to_stdout = Vec::with_capacity(line.len() + 20);
    to_stdout.extend_from_slice(b"\r\x1b[K");
    let to_stderr = prompt.to_vec();
    to_stdout.extend_from_slice(line);
    let cursor_back = display_width_range(line, cursor, line.len());
    if cursor_back > 0 {
        to_stdout.extend_from_slice(b"\x1b[");
        bstr::push_u64(&mut to_stdout, cursor_back as u64);
        to_stdout.push(b'D');
    }
    (to_stdout, to_stderr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{assert_no_syscalls, set_test_locale_c, set_test_locale_utf8};

    #[test]
    fn column_math_ascii_c_locale() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(display_width(b"hello"), 5);
            assert_eq!(display_width_range(b"hello", 0, 3), 3);
            assert_eq!(char_len_at(b"hello", 0), 1);
            assert_eq!(prev_char_start(b"hello", 3), 2);
            assert_eq!(last_char_start(b"hello"), 4);
        });
    }

    #[test]
    fn column_math_multibyte_utf8() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            let line = b"\xc3\xa9"; // "é" — one grapheme, two bytes, one col
            assert_eq!(display_width(line), 1);
            assert_eq!(char_len_at(line, 0), 2);
            assert_eq!(prev_char_start(line, 2), 0);
        });
    }

    #[test]
    fn redraw_sequence_includes_cursor_back_only_when_needed() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            let (out_end, err_end) = redraw_sequence(b"abc", 3, b"$ ");
            assert_eq!(out_end, b"\r\x1b[Kabc");
            assert_eq!(err_end, b"$ ");

            let (out_mid, err_mid) = redraw_sequence(b"abc", 1, b"$ ");
            assert_eq!(out_mid, b"\r\x1b[Kabc\x1b[2D");
            assert_eq!(err_mid, b"$ ");
        });
    }

    #[test]
    fn redraw_sequence_utf8_cursor_math() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            let line = b"\xc3\xa9x"; // "éx"
            let (out, _) = redraw_sequence(line, 2, b"$ ");
            assert!(out.ends_with(b"\x1b[1D"));
        });
    }
}
