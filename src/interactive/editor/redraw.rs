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
//!
//! # Incremental redraw model
//!
//! Each in-progress edit owns a [`DrawAnchor`] that, after the first
//! draw, stores a [`Snapshot`] of the previously-rendered buffer:
//! the bytes painted, the prompt that was used, the terminal width
//! at the time, and the visual `(row, col)` the cursor was left on.
//!
//! The next call to [`redraw`] / [`redraw_sequence`] takes one of two
//! paths:
//!
//! - **Full repaint** (the legacy wrap-aware path). Used on the first
//!   draw, after an explicit [`DrawAnchor::reset`], when the prompt or
//!   terminal width have changed, or when the terminal width is
//!   unknown (e.g. stdout is not a tty — this is what tests exercise).
//!   Emits `\x1b[<N>A` to climb to the prompt's first row (if needed),
//!   `\r\x1b[J` to wipe everything from there down, writes the prompt
//!   to stderr, writes the buffer to stdout, and positions the cursor
//!   on the logical cursor cell with relative motion.
//! - **Incremental** (readline-style). Used when a snapshot exists and
//!   nothing structural has changed. Computes the first byte where
//!   the new buffer diverges from the snapshot, moves the cursor from
//!   the previous position to the visual position of that byte, writes
//!   only the new tail, clears any leftover bytes from the shrunken
//!   old tail with `\x1b[K` / `\x1b[J`, and re-positions the cursor on
//!   the logical cursor cell. The prompt is *never* repainted on this
//!   path, which is what makes single-keystroke edits flicker-free.
//!
//! The implementation deliberately uses *relative* moves
//! (`\x1b[<N>A`/`B`/`C`/`D` plus `\r` for the final col-0 normalize)
//! rather than absolute CUP positioning. We never query the terminal
//! for the prompt's screen row, so relative motion plus a known
//! starting cursor (= where the previous draw left it) is the only
//! correct option.

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

/// Display width of `bytes` excluding any ranges in `invisible`.
/// Each range is a half-open `[start, end)` byte offset into `bytes`.
/// This implements the "visible-only" width calculation required by
/// `docs/features/ps1-prompt-extensions.md` § 9.2 for prompts
/// containing `\[...\]` non-printing regions.
pub(crate) fn display_width_visible(bytes: &[u8], invisible: &[(usize, usize)]) -> usize {
    if invisible.is_empty() {
        return display_width(bytes);
    }
    let mut w = 0;
    let mut i = 0;
    while i < bytes.len() {
        if invisible.iter().any(|(s, e)| i >= *s && i < *e) {
            let (_, len) = sys::locale::decode_char(&bytes[i..]);
            i += if len == 0 { 1 } else { len };
            continue;
        }
        let (wc, len) = sys::locale::decode_char(&bytes[i..]);
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

/// Visible column width of a rendered prompt. Strips ANSI escape
/// sequences (CSI `\x1b[...<final-byte>`, OSC `\x1b]...BEL/ST`, and
/// single-character ESC sequences) so that color/title escapes
/// embedded in the prompt don't inflate the column count used by the
/// wrap-aware redraw algorithm. A `\r` byte resets the running width
/// to 0 — terminals honour this and our column math should follow.
///
/// This is intentionally a streaming, byte-level pass rather than a
/// proper ANSI state machine: prompts in the wild are short, and we
/// only need correctness for the SGR and OSC patterns that real
/// `PS1`s use. Embedded literal newlines are *not* supported — a
/// multi-line `PS1` is outside the scope of the editor's wrap model
/// and would need explicit row-counting plumbing the prompt struct
/// already drops.
pub(crate) fn prompt_visible_width(prompt: &[u8]) -> usize {
    const ESC: u8 = 0x1b;
    const BEL: u8 = 0x07;
    let mut w = 0;
    let mut i = 0;
    while i < prompt.len() {
        match prompt[i] {
            ESC => {
                if i + 1 >= prompt.len() {
                    i += 1;
                    continue;
                }
                match prompt[i + 1] {
                    b'[' => {
                        // CSI: skip params/intermediates up to a
                        // final byte in 0x40..=0x7e.
                        i += 2;
                        while i < prompt.len() && !(0x40..=0x7e).contains(&prompt[i]) {
                            i += 1;
                        }
                        if i < prompt.len() {
                            i += 1;
                        }
                    }
                    b']' => {
                        // OSC: skip up to BEL or ST (ESC \).
                        i += 2;
                        while i < prompt.len() {
                            if prompt[i] == BEL {
                                i += 1;
                                break;
                            }
                            if prompt[i] == ESC && i + 1 < prompt.len() && prompt[i + 1] == b'\\' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    _ => {
                        // Other ESC <X>: two-byte sequence.
                        i += 2;
                    }
                }
            }
            b'\r' => {
                w = 0;
                i += 1;
            }
            _ => {
                let (wc, len) = sys::locale::decode_char(&prompt[i..]);
                let step = if len == 0 { 1 } else { len };
                w += sys::locale::char_width(wc);
                i += step;
            }
        }
    }
    w
}

/// Memo of the previous redraw's state. Kept inside [`DrawAnchor`] so
/// the next call can decide between a full repaint and an
/// incremental update.
#[derive(Clone, Debug)]
struct Snapshot {
    /// Buffer bytes painted by the previous redraw.
    line: Vec<u8>,
    /// Prompt bytes painted by the previous redraw. A prompt change
    /// between calls forces a full repaint (the visible width may
    /// differ even when the trailing characters look identical).
    prompt: Vec<u8>,
    /// Terminal width in cells at the time of the previous redraw.
    /// A `cols` change between calls (SIGWINCH, or a transition into
    /// or out of test mode where `cols` is unknown) forces a full
    /// repaint.
    cols: usize,
    /// Row offset (from the prompt's first row) where the cursor was
    /// left after the previous redraw's final repositioning. Always
    /// normalized via `\r` + relative moves, so this is a reliable
    /// origin for the next incremental update.
    cursor_row: usize,
    /// Column where the cursor was left after the previous redraw.
    cursor_col: usize,
    /// `prompt_visible_width + display_width(line)` at the time of
    /// the previous redraw. Used to detect shrinkage on the next
    /// call (so we can wipe the leftover old tail with `\x1b[K` /
    /// `\x1b[J`).
    end_global: usize,
}

/// Per-edit-session redraw state. Carries the snapshot used by
/// the incremental redraw path. A freshly-constructed anchor (or one
/// passed through [`Self::reset`]) means "no prior draw to diff
/// against" and is the right state to use immediately after
/// emitting a `\r\n` (e.g. before reprinting a prompt below an
/// asynchronous notification) or any other byte sequence that
/// repositions the cursor outside the previously-rendered region.
#[derive(Clone, Debug, Default)]
pub(crate) struct DrawAnchor {
    /// `None` means "next call must emit a full repaint". This is
    /// also the state we fall back to in the test environment where
    /// the terminal width is unknown — the incremental path requires
    /// a known `cols` to compute visual positions.
    prev: Option<Snapshot>,
}

impl DrawAnchor {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Forget the snapshot from the previous draw. Call this after
    /// emitting a `\r\n` or any other byte sequence that repositions
    /// the cursor outside the previously-rendered region — otherwise
    /// the next redraw would issue relative moves that miss the new
    /// terminal-cursor position.
    pub(crate) fn reset(&mut self) {
        self.prev = None;
    }
}

/// Byte offset of the first character that differs between `old` and
/// `new`, walking them in lockstep using [`sys::locale::decode_char`]
/// so multibyte characters stay atomic. If one buffer is a prefix of
/// the other, returns the length of the shorter buffer (= where the
/// "new tail" starts).
fn first_diff_byte(old: &[u8], new: &[u8]) -> usize {
    let mut i = 0;
    while i < old.len() && i < new.len() {
        let (old_wc, old_len) = sys::locale::decode_char(&old[i..]);
        let (new_wc, new_len) = sys::locale::decode_char(&new[i..]);
        let old_step = if old_len == 0 { 1 } else { old_len };
        let new_step = if new_len == 0 { 1 } else { new_len };
        if old_wc != new_wc || old_step != new_step {
            return i;
        }
        // Both decoded to the same codepoint of the same byte length:
        // double-check the bytes actually match (defensive, in case
        // `decode_char` ever normalises differing byte sequences to
        // the same codepoint).
        if old[i..i + old_step] != new[i..i + new_step] {
            return i;
        }
        i += old_step;
    }
    i
}

/// Visual position of the byte at `byte_idx` inside `line`, assuming
/// the prompt occupies `prompt_w` cells on row 0. Returns
/// `(row, col, global)` where `global = row * cols + col`.
///
/// Note: this is the **logical** position. If `global` is a non-zero
/// multiple of `cols`, the corresponding character lives on the
/// *next* row at column 0 — which is what we want when positioning
/// the terminal cursor before writing the new tail.
fn position_for_byte(
    line: &[u8],
    byte_idx: usize,
    prompt_w: usize,
    cols: usize,
) -> (usize, usize, usize) {
    debug_assert!(cols > 0);
    let visual_w = display_width(&line[..byte_idx.min(line.len())]);
    let global = prompt_w + visual_w;
    (global / cols, global % cols, global)
}

/// Row the cursor sits on right after writing content totalling
/// `end_global` cells. Honours the "pending wrap" convention used by
/// the full-repaint path: when content ends *exactly* at a column
/// boundary, the cursor visually stays at the right edge of the
/// previous row until the next byte arrives, and a subsequent `\r`
/// commits to column 0 of that same row.
fn end_row_for_global(end_global: usize, cols: usize) -> usize {
    debug_assert!(cols > 0);
    if end_global == 0 {
        0
    } else if end_global.is_multiple_of(cols) {
        end_global / cols - 1
    } else {
        end_global / cols
    }
}

fn push_csi_num(out: &mut Vec<u8>, n: u64, final_byte: u8) {
    out.extend_from_slice(b"\x1b[");
    bstr::push_u64(out, n);
    out.push(final_byte);
}

/// Append the relative cursor-motion bytes that move the terminal
/// cursor from `from` to `to` (each `(row, col)`). Issues a vertical
/// move first, then a horizontal one; never uses `\r`, so it's safe
/// to call between writes without disturbing the "current row" any
/// more than the requested delta.
fn move_cursor_relative_into(out: &mut Vec<u8>, from: (usize, usize), to: (usize, usize)) {
    let (fr, fc) = from;
    let (tr, tc) = to;
    if tr < fr {
        push_csi_num(out, (fr - tr) as u64, b'A');
    } else if tr > fr {
        push_csi_num(out, (tr - fr) as u64, b'B');
    }
    if tc < fc {
        push_csi_num(out, (fc - tc) as u64, b'D');
    } else if tc > fc {
        push_csi_num(out, (tc - fc) as u64, b'C');
    }
}

/// Build the byte stream for an incremental redraw. Returns the new
/// snapshot to store on the anchor.
fn incremental_into(
    stdout: &mut Vec<u8>,
    prev: &Snapshot,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    prompt_w: usize,
    cols: usize,
) -> Snapshot {
    let diff = first_diff_byte(&prev.line, line);
    let (diff_row, diff_col, _) = position_for_byte(line, diff, prompt_w, cols);

    // 1. Move from the snapshot's cursor to the diff position.
    move_cursor_relative_into(
        stdout,
        (prev.cursor_row, prev.cursor_col),
        (diff_row, diff_col),
    );

    // 2. Write the new tail. The terminal auto-wraps if it crosses
    //    column `cols`; we account for that below when computing the
    //    "after write" position.
    let tail = &line[diff..];
    if !tail.is_empty() {
        stdout.extend_from_slice(tail);
    }

    let new_end_global = prompt_w + display_width(line);
    let new_end_row = end_row_for_global(new_end_global, cols);

    // 3. If the new buffer is shorter than the old, wipe the leftover
    //    bytes from the previous draw. `\x1b[K` clears to end of the
    //    current row (sufficient when both ends sit on the same row);
    //    `\x1b[J` clears to end of screen (needed when the old draw
    //    occupied additional rows below).
    if new_end_global < prev.end_global {
        let old_end_row = end_row_for_global(prev.end_global, cols);
        if new_end_row == old_end_row {
            stdout.extend_from_slice(b"\x1b[K");
        } else {
            stdout.extend_from_slice(b"\x1b[J");
        }
    }

    // 4. Normalize via `\r` so we have a known origin (column 0 of
    //    the row the cursor currently sits on, regardless of any
    //    pending-wrap state) and emit relative moves to the logical
    //    cursor position. The `\r` is a single byte and is not by
    //    itself visible to the user.
    let (target_row, target_col, _) = position_for_byte(line, cursor, prompt_w, cols);
    stdout.push(b'\r');
    if target_row > new_end_row {
        push_csi_num(stdout, (target_row - new_end_row) as u64, b'B');
    } else if target_row < new_end_row {
        push_csi_num(stdout, (new_end_row - target_row) as u64, b'A');
    }
    if target_col > 0 {
        push_csi_num(stdout, target_col as u64, b'C');
    }

    Snapshot {
        line: line.to_vec(),
        prompt: prompt.to_vec(),
        cols,
        cursor_row: target_row,
        cursor_col: target_col,
        end_global: new_end_global,
    }
}

/// Build the byte stream for a full repaint, splitting the writes
/// between stdout (prefix + buffer body + positioning) and stderr
/// (prompt). Returns the new snapshot if `cols` is known, otherwise
/// `None` — the next call will then also take this branch.
fn full_repaint_into(
    stdout_prefix: &mut Vec<u8>,
    stdout_body: &mut Vec<u8>,
    prev_cursor_row: usize,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    cols_opt: Option<usize>,
) -> Option<Snapshot> {
    // 1. Move up to col 0 of the prompt's row (if we know the
    //    previous cursor row), then clear from there to end of screen.
    if prev_cursor_row > 0 {
        push_csi_num(stdout_prefix, prev_cursor_row as u64, b'A');
    }
    stdout_prefix.extend_from_slice(b"\r\x1b[J");

    // 2. Write the buffer bytes (caller writes the prompt to stderr
    //    between these two halves).
    stdout_body.extend_from_slice(line);

    let Some(cols) = cols_opt.filter(|c| *c > 0) else {
        // Unknown terminal width: legacy single-row backwards-only
        // positioning. Don't build a snapshot — the next call will
        // also take the full-repaint branch.
        let cursor_back = display_width_range(line, cursor, line.len());
        if cursor_back > 0 {
            push_csi_num(stdout_body, cursor_back as u64, b'D');
        }
        return None;
    };

    let prompt_w = prompt_visible_width(prompt);
    let cursor_w = display_width_range(line, 0, cursor);
    let end_global = prompt_w + display_width(line);
    let cursor_global = prompt_w + cursor_w;
    let end_row = end_row_for_global(end_global, cols);

    let target_row = cursor_global / cols;
    let target_col = cursor_global % cols;

    stdout_body.push(b'\r');
    if target_row > end_row {
        push_csi_num(stdout_body, (target_row - end_row) as u64, b'B');
    } else if target_row < end_row {
        push_csi_num(stdout_body, (end_row - target_row) as u64, b'A');
    }
    if target_col > 0 {
        push_csi_num(stdout_body, target_col as u64, b'C');
    }

    Some(Snapshot {
        line: line.to_vec(),
        prompt: prompt.to_vec(),
        cols,
        cursor_row: target_row,
        cursor_col: target_col,
        end_global,
    })
}

/// Shared engine for [`redraw`] and [`redraw_sequence`]. Routes the
/// stdout / stderr byte streams through caller-provided closures so
/// production code can write straight to file descriptors while
/// tests accumulate the bytes into `Vec<u8>`s for assertion.
fn redraw_internal(
    anchor: &mut DrawAnchor,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    mut emit_stdout: impl FnMut(&[u8]),
    mut emit_stderr: impl FnMut(&[u8]),
) {
    let cols_opt = sys::tty::terminal_columns_from_stdio();
    let cols = cols_opt.unwrap_or(0);
    let prompt_w = prompt_visible_width(prompt);

    let take_incremental = cols > 0
        && match &anchor.prev {
            Some(p) => p.cols == cols && p.prompt == prompt,
            None => false,
        };

    if take_incremental {
        let prev = anchor.prev.as_ref().unwrap();
        let mut out = Vec::with_capacity(line.len() + 16);
        let new_snapshot = incremental_into(&mut out, prev, line, cursor, prompt, prompt_w, cols);
        emit_stdout(&out);
        anchor.prev = Some(new_snapshot);
    } else {
        let prev_cursor_row = anchor.prev.as_ref().map_or(0, |p| {
            // Only trust the previous cursor row when we still know
            // the same terminal width. After a SIGWINCH (or a
            // transition into / out of test mode where `cols` is
            // unknown) the stored row no longer maps to anything
            // useful, so skip the up-move and just rewrite from the
            // current position.
            if cols > 0 && p.cols == cols {
                p.cursor_row
            } else {
                0
            }
        });
        let mut prefix = Vec::with_capacity(16);
        let mut body = Vec::with_capacity(line.len() + 16);
        let new_snapshot = full_repaint_into(
            &mut prefix,
            &mut body,
            prev_cursor_row,
            line,
            cursor,
            prompt,
            cols_opt,
        );
        emit_stdout(&prefix);
        emit_stderr(prompt);
        emit_stdout(&body);
        anchor.prev = new_snapshot;
    }
}

/// Build the bytes the redraw would emit, without touching any file
/// descriptor. Useful for unit tests that assert the produced
/// control sequences directly.
///
/// Returns `(to_stdout, to_stderr)`. On the incremental path
/// `to_stderr` is empty — the prompt is never repainted.
pub(crate) fn redraw_sequence(
    anchor: &mut DrawAnchor,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::with_capacity(line.len() + 32);
    let mut stderr = Vec::new();
    redraw_internal(
        anchor,
        line,
        cursor,
        prompt,
        |b| stdout.extend_from_slice(b),
        |b| stderr.extend_from_slice(b),
    );
    (stdout, stderr)
}

/// Emit the redraw sequence to `stdout` (buffer + positioning) and
/// `stderr` (prompt, only on the full-repaint path). Splits the
/// stdout stream into prefix and body so that, when a full repaint
/// is required, the stderr prompt write happens between them — that
/// ordering is what downstream consumers (tmux, bash transcripts)
/// expect.
pub(crate) fn redraw(anchor: &mut DrawAnchor, line: &[u8], cursor: usize, prompt: &[u8]) {
    redraw_internal(
        anchor,
        line,
        cursor,
        prompt,
        |b| write_bytes(b),
        |b| {
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, b);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{
        assert_no_syscalls, set_test_locale_c, set_test_locale_utf8, set_test_terminal_columns,
    };

    fn with_cols<F: FnOnce()>(cols: Option<usize>, f: F) {
        set_test_terminal_columns(cols);
        f();
        set_test_terminal_columns(None);
    }

    fn snapshot_at(line: &[u8], prompt: &[u8], cols: usize, cursor: usize) -> DrawAnchor {
        let prompt_w = prompt_visible_width(prompt);
        let cursor_w = display_width_range(line, 0, cursor);
        let end_global = prompt_w + display_width(line);
        let cursor_global = prompt_w + cursor_w;
        let cursor_row = cursor_global / cols;
        let cursor_col = cursor_global % cols;
        DrawAnchor {
            prev: Some(Snapshot {
                line: line.to_vec(),
                prompt: prompt.to_vec(),
                cols,
                cursor_row,
                cursor_col,
                end_global,
            }),
        }
    }

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
    fn prompt_visible_width_strips_csi_and_osc() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // SGR colour wrap: ESC[31m + "RED" + ESC[0m  -> 3 cols.
            assert_eq!(prompt_visible_width(b"\x1b[31mRED\x1b[0m"), 3);
            // OSC title set: ESC] 0 ; title BEL + "$ "  -> 2 cols.
            assert_eq!(prompt_visible_width(b"\x1b]0;hi\x07$ "), 2);
            // ESC \\ terminator form.
            assert_eq!(prompt_visible_width(b"\x1b]2;hi\x1b\\$ "), 2);
            // Plain text fallback.
            assert_eq!(prompt_visible_width(b"$ "), 2);
            // `\r` resets the running count.
            assert_eq!(prompt_visible_width(b"foo\r$ "), 2);
        });
    }

    // ---------- first_diff_byte ----------

    #[test]
    fn first_diff_byte_ascii() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(first_diff_byte(b"hello", b"hello"), 5);
            assert_eq!(first_diff_byte(b"hello", b"help"), 3);
            assert_eq!(first_diff_byte(b"hello", b"hellox"), 5);
            assert_eq!(first_diff_byte(b"hello", b"hi"), 1);
            assert_eq!(first_diff_byte(b"", b"abc"), 0);
            assert_eq!(first_diff_byte(b"abc", b""), 0);
        });
    }

    #[test]
    fn first_diff_byte_utf8_keeps_chars_atomic() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // "éx" (c3 a9 78) vs "èx" (c3 a8 78): differ inside the
            // first multibyte char. The diff must point at the start
            // of that char (byte 0), not at the differing
            // continuation byte (1).
            assert_eq!(first_diff_byte(b"\xc3\xa9x", b"\xc3\xa8x"), 0);
            // Same prefix "é", then differing ASCII char.
            assert_eq!(first_diff_byte(b"\xc3\xa9x", b"\xc3\xa9y"), 2);
        });
    }

    // ---------- full-repaint path (first call) ----------

    #[test]
    fn full_repaint_first_call_short_line_at_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let (out, err) = redraw_sequence(&mut anchor, b"abc", 3, b"$ ");
                assert_eq!(out, b"\r\x1b[Jabc\r\x1b[5C");
                assert_eq!(err, b"$ ");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 0);
                assert_eq!(prev.cursor_col, 5);
                assert_eq!(prev.end_global, 5);
            });
        });
    }

    #[test]
    fn full_repaint_first_call_short_line_cursor_in_middle() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let (out, _) = redraw_sequence(&mut anchor, b"abc", 1, b"$ ");
                assert_eq!(out, b"\r\x1b[Jabc\r\x1b[3C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 0);
                assert_eq!(prev.cursor_col, 3);
            });
        });
    }

    #[test]
    fn full_repaint_first_call_wraps_to_next_row() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                let mut anchor = DrawAnchor::new();
                let line = b"abcdefghi"; // 9 chars + prompt 2 = 11 cells
                let (out, _) = redraw_sequence(&mut anchor, line, line.len(), b"$ ");
                assert_eq!(out, b"\r\x1b[Jabcdefghi\r\x1b[1C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 1);
            });
        });
    }

    #[test]
    fn full_repaint_first_call_cursor_on_earlier_row_moves_up() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                let mut anchor = DrawAnchor::new();
                let line = b"abcdefghijkl"; // 12 chars + prompt 2 = 14 cells
                let (out, _) = redraw_sequence(&mut anchor, line, 0, b"$ ");
                assert_eq!(out, b"\r\x1b[Jabcdefghijkl\r\x1b[1A\x1b[2C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 0);
                assert_eq!(prev.cursor_col, 2);
            });
        });
    }

    #[test]
    fn full_repaint_first_call_exact_column_boundary() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                let mut anchor = DrawAnchor::new();
                let line = b"abcdefgh"; // 8 + 2 = 10 cells, exact
                let (out, _) = redraw_sequence(&mut anchor, line, line.len(), b"$ ");
                assert_eq!(out, b"\r\x1b[Jabcdefgh\r\x1b[1B");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 0);
            });
        });
    }

    #[test]
    fn full_repaint_unknown_cols_falls_back_to_legacy_back_only() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(None, || {
                let mut anchor = DrawAnchor::new();
                let (out, _) = redraw_sequence(&mut anchor, b"abc", 1, b"$ ");
                assert_eq!(out, b"\r\x1b[Jabc\x1b[2D");
                // No snapshot is stored when `cols` is unknown — the
                // next call must also take the full-repaint branch.
                assert!(anchor.prev.is_none());
            });
        });
    }

    #[test]
    fn full_repaint_utf8_cursor_math() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let line = b"\xc3\xa9x"; // "éx" — 2 cols visible
                let (out, _) = redraw_sequence(&mut anchor, line, 2, b"$ ");
                // End at col 4, cursor at col 3 (after "é"). Suffix:
                // CR + move right 3.
                assert!(out.ends_with(b"\r\x1b[3C"));
            });
        });
    }

    #[test]
    fn full_repaint_after_reset_with_wrapped_previous_climbs_back() {
        // When a previous draw left the cursor on row 2 and a caller
        // *didn't* reset before triggering a full repaint (e.g. by
        // changing the prompt), the full-repaint path climbs back to
        // the prompt row and wipes everything below.
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                // Pre-seed an anchor describing a long previous draw
                // that wrapped to row 2.
                let mut anchor = snapshot_at(
                    b"abcabcabcabcabcabcabcabcabc", // 27 chars
                    b"$ ",
                    10,
                    27,
                );
                // Now redraw with "hi" under a *different* prompt so
                // the incremental path is rejected and we go through
                // full-repaint.
                let (out, err) = redraw_sequence(&mut anchor, b"hi", 2, b"# ");
                // 27 + 2 = 29 cells, ends on row 2 col 9, so we climb
                // 2 rows and wipe.
                assert_eq!(out, b"\x1b[2A\r\x1b[Jhi\r\x1b[4C");
                assert_eq!(err, b"# ");
            });
        });
    }

    // ---------- incremental path ----------

    #[test]
    fn incremental_self_insert_at_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hel", b"$ ", 80, 3);
                let (out, err) = redraw_sequence(&mut anchor, b"hell", 4, b"$ ");
                // Cursor already at the diff point (col 5), so we
                // just write the new char and then issue `\r\x1b[6C`
                // to land on the final cursor position.
                assert_eq!(out, b"l\r\x1b[6C");
                // Prompt is NOT repainted on the incremental path.
                assert_eq!(err, b"");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.line, b"hell");
                assert_eq!(prev.cursor_col, 6);
                assert_eq!(prev.end_global, 6);
            });
        });
    }

    #[test]
    fn incremental_backspace_at_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hellx", b"$ ", 80, 5);
                let (out, err) = redraw_sequence(&mut anchor, b"hell", 4, b"$ ");
                // Move cursor from col 7 to diff at col 6 (one left),
                // tail empty, shrink → \x1b[K, then \r + move right
                // 6 to land at logical cursor (col 6).
                assert_eq!(out, b"\x1b[1D\x1b[K\r\x1b[6C");
                assert_eq!(err, b"");
            });
        });
    }

    #[test]
    fn incremental_insert_in_middle() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hello", b"$ ", 80, 5);
                // Insert 'X' at position 2: "heXllo", cursor moves to 3.
                let (out, _) = redraw_sequence(&mut anchor, b"heXllo", 3, b"$ ");
                // From cursor at col 7, move left to col 4 (diff
                // point), write "Xllo" (4 chars), end at col 8, then
                // \r + move right 5 to target col 5.
                assert_eq!(out, b"\x1b[3DXllo\r\x1b[5C");
            });
        });
    }

    #[test]
    fn incremental_backspace_in_middle() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hello", b"$ ", 80, 3);
                // Backspace at cursor=3: "hllo", cursor moves to 2.
                let (out, _) = redraw_sequence(&mut anchor, b"hllo", 2, b"$ ");
                // From cursor at col 5, move left to col 3 (diff),
                // write "llo" (3 chars), end at col 6, shrink (was 7)
                // → \x1b[K, then \r + move right 4 to target col 4.
                assert_eq!(out, b"\x1b[2Dllo\x1b[K\r\x1b[4C");
            });
        });
    }

    #[test]
    fn incremental_cursor_only_move() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hello", b"$ ", 80, 5);
                // Same buffer, cursor moves from 5 to 3 (e.g. left x2).
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 3, b"$ ");
                // No tail to write, no shrink. From cursor (0, 7) to
                // diff (0, 7) - no move. Then \r + move right 5 to
                // target col 5.
                assert_eq!(out, b"\r\x1b[5C");
                assert_eq!(err, b"");
            });
        });
    }

    #[test]
    fn incremental_self_insert_at_wrap_row() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                // Prev: "abcdefghi" (9 chars + prompt 2 = 11 cells)
                // wraps to row 1, cursor at end = (1, 1).
                let mut anchor = snapshot_at(b"abcdefghi", b"$ ", 10, 9);
                // Type 'j': "abcdefghij" (10 chars + 2 = 12 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcdefghij", 10, b"$ ");
                // Cursor already at diff point (1, 1). Write 'j'.
                // End at (1, 2). Target (1, 2). \r + move right 2.
                assert_eq!(out, b"j\r\x1b[2C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 2);
            });
        });
    }

    #[test]
    fn incremental_self_insert_crosses_into_new_wrap_row() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                // Prev: 7 chars + prompt 2 = 9 cells, all on row 0.
                let mut anchor = snapshot_at(b"abcdefg", b"$ ", 10, 7);
                // Type "hi": "abcdefghi" (9 chars + 2 = 11 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcdefghi", 9, b"$ ");
                // From (0, 9) write "hi". Auto-wrap puts cursor at
                // (1, 1). \r + move right 1 to target (1, 1).
                assert_eq!(out, b"hi\r\x1b[1C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 1);
            });
        });
    }

    #[test]
    fn incremental_shrink_across_rows_clears_to_screen_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(10), || {
                // Prev: 18 chars + prompt 2 = 20 cells spanning rows
                // 0..=1, cursor parked manually at (0, 6) (simulating
                // a backward-word jump inside the wrapped buffer).
                let mut anchor = DrawAnchor {
                    prev: Some(Snapshot {
                        line: b"abcdefghijklmnopqr".to_vec(),
                        prompt: b"$ ".to_vec(),
                        cols: 10,
                        cursor_row: 0,
                        cursor_col: 6,
                        end_global: 20,
                    }),
                };
                // C-k from cursor 4: "abcd" (4 chars + 2 = 6 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcd", 4, b"$ ");
                // From (0, 6) diff at byte 4, position (0, 6). No
                // move. Empty tail. Shrink to row 0 from row 1:
                // cross-row → \x1b[J. Then \r + move right 6.
                assert_eq!(out, b"\x1b[J\r\x1b[6C");
            });
        });
    }

    #[test]
    fn incremental_utf8_replace_one_char() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            with_cols(Some(80), || {
                // "éx" — 2 cols visible (snapshot cursor at end, col 4).
                let mut anchor = snapshot_at(b"\xc3\xa9x", b"$ ", 80, 3);
                // Replace "é" with "è" — same byte count, same width.
                let (out, _) = redraw_sequence(&mut anchor, b"\xc3\xa8x", 3, b"$ ");
                // Diff at byte 0 (snap-back over multibyte). Move
                // from (0, 4) to (0, 2): \x1b[2D. Write the new bytes
                // for "è" + 'x' (3 bytes). End at col 4. \r + 4C.
                assert_eq!(out, b"\x1b[2D\xc3\xa8x\r\x1b[4C");
            });
        });
    }

    #[test]
    fn incremental_clear_to_end_buffer() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                // C-k clearing everything to end: from "hello" → "".
                let mut anchor = snapshot_at(b"hello", b"$ ", 80, 0);
                // anchor.prev.cursor_col is 2 (just after prompt).
                let (out, _) = redraw_sequence(&mut anchor, b"", 0, b"$ ");
                // Cursor already at diff (0, 2). Empty tail. Shrink
                // same-row → \x1b[K. \r + 2C.
                assert_eq!(out, b"\x1b[K\r\x1b[2C");
            });
        });
    }

    // ---------- full-repaint trigger ----------

    #[test]
    fn prompt_change_forces_full_repaint() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hello", b"$ ", 80, 5);
                // Change prompt: incremental must be rejected.
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 5, b"# ");
                // Full repaint: prefix + body + position.
                assert_eq!(out, b"\r\x1b[Jhello\r\x1b[7C");
                assert_eq!(err, b"# ");
            });
        });
    }

    #[test]
    fn cols_change_forces_full_repaint() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // Pre-seed with cols=10 then redraw under cols=20.
            let mut anchor = snapshot_at(b"hello", b"$ ", 10, 5);
            with_cols(Some(20), || {
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 5, b"$ ");
                // Full repaint, no climb-up (cols differ so we don't
                // trust the previous cursor row).
                assert_eq!(out, b"\r\x1b[Jhello\r\x1b[7C");
                assert_eq!(err, b"$ ");
            });
        });
    }

    #[test]
    fn display_width_visible_skips_invisible_ranges() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            let bytes = b"\x1b[31mRED\x1b[0m";
            let invisible = vec![(0, 5), (8, 12)];
            assert_eq!(display_width_visible(bytes, &invisible), 3);
            assert_eq!(display_width_visible(b"abc", &[]), 3);
        });
    }
}
