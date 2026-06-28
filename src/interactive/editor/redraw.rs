//! Cursor-math and redraw helpers shared between the vi and emacs
//! editors. All public helpers are locale-aware: byte offsets into the
//! buffer are treated as multibyte-safe indices backed by
//! [`crate::sys::locale::decode_char`], so UTF-8 input renders with
//! correct column widths in the prompt.
//!
//! The [`redraw`] helper intentionally writes to `stdout` (buffer
//! bytes plus literal `\n`) and `stderr` (prompts: PS1 once at the
//! top, PS2 on every continuation row introduced by an embedded
//! `\n`). Keeping prompts on `stderr` matches the legacy vi-editor
//! contract so pipelines that capture `stdout` don't end up with the
//! prompt interleaved.
//!
//! # Multi-line buffers
//!
//! The buffer is a flat `Vec<u8>` that may contain embedded `\n`
//! bytes — they arrive via bracketed paste, via `insert-newline`
//! (`Alt-Enter` / `Shift-Enter`), or via the parser-driven
//! auto-continuation that `accept-line` performs when the buffer is
//! syntactically incomplete. The walker treats `\n` as a hard row
//! break: it advances the cursor to column `ps2_w` of the next row,
//! and the renderer correspondingly writes the literal `\n` to
//! stdout (the tty's OPOST + ONLCR turns it into `\r\n`) followed by
//! the expanded PS2 bytes to stderr. The end effect is that a
//! buffer like `b"for i in 1 2 3\n  echo $i\ndone"` renders as
//!
//! ```text
//! $ for i in 1 2 3
//! >   echo $i
//! > done
//! ```
//!
//! identical to the cross-call PS2 continuation that
//! [`crate::interactive::repl::run_loop`] performs when the parser
//! reports incomplete input across `read_line` invocations.
//!
//! # Incremental redraw model
//!
//! Each in-progress edit owns a [`DrawAnchor`] that, after the first
//! draw, stores a [`Snapshot`] of the previously-rendered buffer:
//! the bytes painted, the prompts (PS1 and PS2) that were used, the
//! terminal width at the time, and the visual `(row, col)` the
//! cursor was left on plus the `(end_row, end_col)` the buffer
//! occupied.
//!
//! The next call to [`redraw`] / [`redraw_sequence`] takes one of two
//! paths:
//!
//! - **Full repaint** (the wrap-aware path). Used on the first draw,
//!   after an explicit [`DrawAnchor::reset`], when the PS1 or PS2
//!   bytes or terminal width have changed, or when the terminal
//!   width is unknown (e.g. stdout is not a tty — this is what
//!   tests exercise). Emits `\x1b[<N>A` to climb to the prompt's
//!   first row (if needed), `\r\x1b[J` to wipe everything from there
//!   down, writes the prompt to stderr, then walks the buffer
//!   emitting bytes to stdout and PS2 to stderr at every embedded
//!   `\n`, and finally positions the cursor on the logical cursor
//!   cell with relative motion.
//! - **Incremental** (readline-style). Used when a snapshot exists
//!   and nothing structural has changed. Computes the first byte
//!   where the new buffer diverges from the snapshot, moves the
//!   cursor from the previous position to the visual position of
//!   that byte, writes only the new tail (with PS2 emissions at
//!   each embedded `\n`), clears any leftover bytes from the
//!   shrunken old tail with `\x1b[K` / `\x1b[J`, and re-positions
//!   the cursor on the logical cursor cell. PS1 is *never*
//!   repainted on this path, which is what makes single-keystroke
//!   edits flicker-free.
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
///
/// This is a *per-character width* helper. It is **not** a screen
/// position — `\n` is reported as zero width here. Callers that need
/// the on-screen `(row, col)` of a buffer prefix must use [`walk`].
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

/// True iff `wc` is a zero-width *combining* character that should be
/// glued to the preceding base character when forming a grapheme
/// cluster for cursor-movement purposes (e.g. U+0304 COMBINING MACRON
/// in `m̄`).
///
/// The set is exactly the codepoints the renderer draws with zero
/// columns (`char_width(wc) == 0`) *minus* the C0/C1 control range and
/// DEL. Those controls are independently-addressable buffer positions
/// — most importantly `\n`, the editor's logical-line separator —
/// that must never be absorbed into a neighbouring grapheme. Tying the
/// predicate to `char_width` (rather than a hand-rolled Unicode table)
/// keeps grapheme grouping and on-screen width in lockstep: any mark
/// the renderer collapses to zero columns is one the cursor steps over
/// as part of its base, and vice versa.
fn is_zero_width_combining(wc: u32) -> bool {
    if wc < 0x20 || (0x7f..=0x9f).contains(&wc) {
        return false;
    }
    sys::locale::char_width(wc) == 0
}

/// Byte length of the *grapheme cluster* starting at `pos`: the base
/// character plus any immediately-following zero-width combining marks.
/// Returns 0 past end-of-input.
///
/// Cursor motion (`forward-char` and the vi `l`-family) uses this in
/// place of [`char_len_at`] so a single keypress steps over a whole
/// visible glyph (`m̄` = `m` + U+0304) instead of parking the cursor
/// between a base letter and its accent — a position that occupies the
/// same screen column and is therefore invisible to the user.
pub(crate) fn grapheme_len_at(line: &[u8], pos: usize) -> usize {
    if pos >= line.len() {
        return 0;
    }
    let mut end = pos + char_len_at(line, pos);
    while end < line.len() {
        let (wc, len) = sys::locale::decode_char(&line[end..]);
        if !is_zero_width_combining(wc) {
            break;
        }
        end += if len == 0 { 1 } else { len };
    }
    end - pos
}

/// Byte offset of the start of the grapheme cluster that ends just
/// before `pos`: walk back over any trailing zero-width combining
/// marks and then over the single base character they attach to.
/// Mirror of [`grapheme_len_at`] for `backward-char` / vi `h`.
pub(crate) fn prev_grapheme_start(line: &[u8], pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = prev_char_start(line, pos);
    while p > 0 {
        let (wc, _) = sys::locale::decode_char(&line[p..]);
        if !is_zero_width_combining(wc) {
            break;
        }
        p = prev_char_start(line, p);
    }
    p
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

/// A position on the rendered screen, expressed as `(row, col)` where
/// `row = 0` is the row that received PS1.
///
/// `col` is in `0..=cols`. The boundary value `col == cols` represents
/// the **pending-wrap** state: the previous character filled the
/// rightmost cell of `row`, the terminal cursor visually sits at
/// `(row, cols - 1)`, and the very next emitted byte will commit the
/// wrap and land at `(row + 1, 0)`. See [`normalize`] for converting
/// a pending-wrap position to the row/col where the *next* byte will
/// land.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ScreenPos {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

/// Walk `line[..up_to]` and return the screen position after rendering
/// it, assuming PS1 occupies `ps1_w` cells on row 0 and PS2 occupies
/// `ps2_w` cells at the start of every row introduced by an embedded
/// `\n`. Terminal-width wraps (chars overflowing `cols`) restart at
/// column 0 — terminals don't reserve a per-row gutter for wrap, only
/// for our explicit `\n + PS2` emissions.
///
/// `cols == 0` disables wrap entirely (used by the test-mode path
/// that doesn't know the terminal width).
pub(crate) fn walk(
    line: &[u8],
    up_to: usize,
    ps1_w: usize,
    ps2_w: usize,
    cols: usize,
) -> ScreenPos {
    let mut row = 0usize;
    let mut col = ps1_w;
    let limit = up_to.min(line.len());
    let mut i = 0usize;
    while i < limit {
        // Commit a pending wrap from the previous iteration before
        // we read the next byte, so a pending wrap directly followed
        // by `\n` still lands the `\n` cleanly on the next row.
        if cols > 0 && col >= cols {
            row += 1;
            col = 0;
        }
        if line[i] == b'\n' {
            row += 1;
            col = ps2_w;
            i += 1;
            continue;
        }
        let (wc, len) = sys::locale::decode_char(&line[i..]);
        let step = if len == 0 { 1 } else { len };
        let w = sys::locale::char_width(wc);
        if cols > 0 && col + w > cols {
            row += 1;
            col = 0;
        }
        col += w;
        i += step;
    }
    ScreenPos { row, col }
}

/// Convert a [`ScreenPos`] returned by [`walk`] from its raw form
/// (`col` may equal `cols` to indicate pending-wrap) to the logical
/// "where will the next byte land?" form (`col < cols`, always).
/// Used when computing the *target* cursor cell.
fn normalize(pos: ScreenPos, cols: usize) -> ScreenPos {
    if cols > 0 && pos.col >= cols {
        ScreenPos {
            row: pos.row + 1,
            col: 0,
        }
    } else {
        pos
    }
}

/// Memo of the previous redraw's state. Kept inside [`DrawAnchor`] so
/// the next call can decide between a full repaint and an
/// incremental update.
#[derive(Clone, Debug)]
struct Snapshot {
    /// Buffer bytes painted by the previous redraw.
    line: Vec<u8>,
    /// PS1 bytes painted by the previous redraw. A change between
    /// calls forces a full repaint (the visible width may differ
    /// even when the trailing characters look identical).
    prompt: Vec<u8>,
    /// PS2 bytes used as the continuation gutter by the previous
    /// redraw. A change forces a full repaint for the same reason.
    ps2: Vec<u8>,
    /// Terminal width in cells at the time of the previous redraw.
    /// A `cols` change between calls (SIGWINCH, or a transition into
    /// or out of test mode where `cols` is unknown) forces a full
    /// repaint.
    cols: usize,
    /// Logical row offset (from the prompt's first row) where the
    /// cursor was left after the previous redraw's final
    /// repositioning. Always normalized via `\r` + relative moves
    /// plus an explicit commit of any pending wrap, so this is a
    /// reliable origin for the next incremental update.
    cursor_row: usize,
    /// Column where the cursor was left after the previous redraw.
    cursor_col: usize,
    /// Position of the *end* of the previously-rendered content
    /// (i.e. what [`walk`] returns for `up_to = line.len()`). Stored
    /// in raw form so a `col == cols` pending-wrap end is
    /// distinguishable from a real `(end_row + 1, 0)` end.
    end_row: usize,
    end_col: usize,
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
    /// Number of terminal rows between where the cursor was left
    /// after the previous redraw and the visual end of the buffer
    /// painted by that same redraw. Returns 0 when there is no prior
    /// snapshot (cols unknown, first draw, just-reset anchor) or when
    /// the cursor was already on the final row, so callers can
    /// unconditionally do `if rows > 0 { emit ESC[<rows>B }`.
    ///
    /// Used by the accept-line path to step the terminal cursor past
    /// any rows of an accepted multi-line buffer before handing the
    /// line to the executor, so command output appears below the
    /// entire input rather than overwriting trailing rows.
    pub(crate) fn rows_below_cursor(&self) -> usize {
        self.prev
            .as_ref()
            .map(|s| s.end_row.saturating_sub(s.cursor_row))
            .unwrap_or(0)
    }
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

/// Write the bytes of `tail` to `emit_stdout`, interleaving an
/// `emit_stderr(ps2)` call at every embedded `\n` so each continuation
/// row gets its left-margin gutter. Returns the screen position the
/// terminal cursor would occupy after the write.
///
/// `start_row` / `start_col` is the (raw) walker position the cursor
/// occupied before the write began — usually obtained from
/// [`walk`] of the buffer prefix up to the diff byte. Returning a raw
/// walker-style `ScreenPos` (col in `0..=cols`) keeps pending-wrap
/// distinguishable from "after a `\n` + PS2".
fn write_tail_emitting_ps2(
    emit_stdout: &mut dyn FnMut(&[u8]),
    emit_stderr: &mut dyn FnMut(&[u8]),
    tail: &[u8],
    ps2: &[u8],
    ps2_w: usize,
    start_row: usize,
    start_col: usize,
    cols: usize,
) -> ScreenPos {
    let mut row = start_row;
    let mut col = start_col;
    let mut chunk_start = 0;
    let mut i = 0;
    while i < tail.len() {
        if tail[i] == b'\n' {
            if i > chunk_start {
                emit_stdout(&tail[chunk_start..i]);
            }
            emit_stdout(b"\n");
            emit_stderr(ps2);
            row += 1;
            col = ps2_w;
            i += 1;
            chunk_start = i;
            continue;
        }
        let (wc, len) = sys::locale::decode_char(&tail[i..]);
        let step = if len == 0 { 1 } else { len };
        let w = sys::locale::char_width(wc);
        // Commit any pending wrap from the previous byte before
        // updating col.
        if cols > 0 && col >= cols {
            row += 1;
            col = 0;
        }
        if cols > 0 && col + w > cols {
            row += 1;
            col = 0;
        }
        col += w;
        i += step;
    }
    if chunk_start < tail.len() {
        emit_stdout(&tail[chunk_start..]);
    }
    ScreenPos { row, col }
}

/// Build the byte stream for an incremental redraw and emit it
/// through the caller-provided closures. Returns the new snapshot to
/// store on the anchor.
#[allow(clippy::too_many_arguments)]
fn incremental_into(
    emit_stdout: &mut dyn FnMut(&[u8]),
    emit_stderr: &mut dyn FnMut(&[u8]),
    prev: &Snapshot,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    ps2: &[u8],
    ps1_w: usize,
    ps2_w: usize,
    cols: usize,
) -> Snapshot {
    let diff = first_diff_byte(&prev.line, line);
    let diff_pos = walk(line, diff, ps1_w, ps2_w, cols);
    let diff_target = normalize(diff_pos, cols);

    // 1. Move from the snapshot's cursor to the diff position.
    let mut buf = Vec::with_capacity(line.len() + 16);
    move_cursor_relative_into(
        &mut buf,
        (prev.cursor_row, prev.cursor_col),
        (diff_target.row, diff_target.col),
    );
    if !buf.is_empty() {
        emit_stdout(&buf);
    }

    // 1a. If the new tail contains a `\n`, the very first byte we
    //     write will move the cursor off the diff row before
    //     overwriting whatever the old draw had past the diff column
    //     on that row. The post-write shrinkage check below compares
    //     end positions only, and `(new_end_row+1, _)` is
    //     lexicographically *greater* than the old end on the diff
    //     row — so it never fires for newline insertion. Wipe the
    //     stale screen tail with `\x1b[J` (clear-to-end-of-screen)
    //     up front in that case; the new tail then rewrites the
    //     diff row and everything below it from scratch. The cost
    //     is one extra 3-byte escape on operations that already
    //     involve newlines (Alt-Enter, multi-line paste, smart
    //     accept-line auto-continuation); pure single-line edits
    //     skip this branch and stay flicker-free.
    if line[diff..].contains(&b'\n') {
        emit_stdout(b"\x1b[J");
    }

    // 2. Write the new tail, emitting PS2 to stderr at every
    //    embedded `\n` so continuation rows get their gutter. The
    //    `\n` itself goes to stdout — the tty's OPOST + ONLCR turns
    //    it into `\r\n`.
    let end_pos = if diff < line.len() {
        write_tail_emitting_ps2(
            emit_stdout,
            emit_stderr,
            &line[diff..],
            ps2,
            ps2_w,
            diff_target.row,
            diff_target.col,
            cols,
        )
    } else {
        diff_pos
    };

    // 3. If the new buffer ends earlier than the old, wipe the
    //    leftover bytes from the previous draw. Use `\x1b[K`
    //    (clear-to-end-of-row) when both ends sit on the same row,
    //    `\x1b[J` (clear-to-end-of-screen) when the old draw
    //    occupied additional rows below.
    let shrink_buf = {
        let mut b = Vec::new();
        let new_end_norm = normalize(end_pos, cols);
        let prev_end_norm = normalize(
            ScreenPos {
                row: prev.end_row,
                col: prev.end_col,
            },
            cols,
        );
        if (new_end_norm.row, new_end_norm.col) < (prev_end_norm.row, prev_end_norm.col) {
            if new_end_norm.row == prev_end_norm.row {
                b.extend_from_slice(b"\x1b[K");
            } else {
                b.extend_from_slice(b"\x1b[J");
            }
        }
        b
    };
    if !shrink_buf.is_empty() {
        emit_stdout(&shrink_buf);
    }

    // 4. Normalize via `\r` so we have a known origin (column 0 of
    //    the row the cursor currently sits on, regardless of any
    //    pending-wrap state) and emit relative moves to the logical
    //    cursor position. The `\r` is a single byte and is not by
    //    itself visible to the user.
    let cursor_target = normalize(walk(line, cursor, ps1_w, ps2_w, cols), cols);
    let mut tail_buf = Vec::with_capacity(8);
    tail_buf.push(b'\r');
    // After `\r`, the cursor sits at `(end_pos.row, 0)`. Move to the
    // cursor target from there.
    if cursor_target.row > end_pos.row {
        push_csi_num(
            &mut tail_buf,
            (cursor_target.row - end_pos.row) as u64,
            b'B',
        );
    } else if cursor_target.row < end_pos.row {
        push_csi_num(
            &mut tail_buf,
            (end_pos.row - cursor_target.row) as u64,
            b'A',
        );
    }
    if cursor_target.col > 0 {
        push_csi_num(&mut tail_buf, cursor_target.col as u64, b'C');
    }
    emit_stdout(&tail_buf);

    Snapshot {
        line: line.to_vec(),
        prompt: prompt.to_vec(),
        ps2: ps2.to_vec(),
        cols,
        cursor_row: cursor_target.row,
        cursor_col: cursor_target.col,
        end_row: end_pos.row,
        end_col: end_pos.col,
    }
}

/// Build the byte stream for a full repaint. Emits through the
/// caller-provided closures so production code can write straight
/// to file descriptors while tests accumulate the bytes into
/// `Vec<u8>`s for assertion. Returns the new snapshot if `cols` is
/// known, otherwise `None` — the next call will then also take
/// this branch.
#[allow(clippy::too_many_arguments)]
fn full_repaint_into(
    emit_stdout: &mut dyn FnMut(&[u8]),
    emit_stderr: &mut dyn FnMut(&[u8]),
    prev_cursor_row: usize,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    ps2: &[u8],
    ps1_w: usize,
    ps2_w: usize,
    cols_opt: Option<usize>,
) -> Option<Snapshot> {
    // 1. Move up to col 0 of the prompt's row (if we know the
    //    previous cursor row), then clear from there to end of screen.
    let mut prefix = Vec::with_capacity(16);
    if prev_cursor_row > 0 {
        push_csi_num(&mut prefix, prev_cursor_row as u64, b'A');
    }
    prefix.extend_from_slice(b"\r\x1b[J");
    emit_stdout(&prefix);

    // 2. Write PS1 to stderr; the caller's chosen closure honours the
    //    "prompts on stderr" contract.
    emit_stderr(prompt);

    let Some(cols) = cols_opt.filter(|c| *c > 0) else {
        // Unknown terminal width: legacy single-row backwards-only
        // positioning, with no PS2 interleaving (we can't honour
        // multi-line layout without column math). Don't build a
        // snapshot — the next call will also take this branch.
        let mut body = Vec::with_capacity(line.len() + 8);
        body.extend_from_slice(line);
        let cursor_back = display_width_range(line, cursor, line.len());
        if cursor_back > 0 {
            push_csi_num(&mut body, cursor_back as u64, b'D');
        }
        emit_stdout(&body);
        return None;
    };

    // 3. Walk the buffer, writing chunks of bytes to stdout and the
    //    PS2 bytes to stderr at each embedded `\n`.
    let end_pos =
        write_tail_emitting_ps2(emit_stdout, emit_stderr, line, ps2, ps2_w, 0, ps1_w, cols);

    // 4. Position the cursor on the target cell.
    let cursor_target = normalize(walk(line, cursor, ps1_w, ps2_w, cols), cols);
    let mut tail = Vec::with_capacity(8);
    tail.push(b'\r');
    if cursor_target.row > end_pos.row {
        push_csi_num(&mut tail, (cursor_target.row - end_pos.row) as u64, b'B');
    } else if cursor_target.row < end_pos.row {
        push_csi_num(&mut tail, (end_pos.row - cursor_target.row) as u64, b'A');
    }
    if cursor_target.col > 0 {
        push_csi_num(&mut tail, cursor_target.col as u64, b'C');
    }
    emit_stdout(&tail);

    Some(Snapshot {
        line: line.to_vec(),
        prompt: prompt.to_vec(),
        ps2: ps2.to_vec(),
        cols,
        cursor_row: cursor_target.row,
        cursor_col: cursor_target.col,
        end_row: end_pos.row,
        end_col: end_pos.col,
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
    ps2: &[u8],
    mut emit_stdout: impl FnMut(&[u8]),
    mut emit_stderr: impl FnMut(&[u8]),
) {
    let cols_opt = sys::tty::terminal_columns_from_stdio();
    let cols = cols_opt.unwrap_or(0);
    let ps1_w = prompt_visible_width(prompt);
    let ps2_w = prompt_visible_width(ps2);

    let take_incremental = cols > 0
        && match &anchor.prev {
            Some(p) => p.cols == cols && p.prompt == prompt && p.ps2 == ps2,
            None => false,
        };

    if take_incremental {
        let prev = anchor.prev.as_ref().unwrap();
        let mut stdout = |b: &[u8]| emit_stdout(b);
        let mut stderr = |b: &[u8]| emit_stderr(b);
        let new_snapshot = incremental_into(
            &mut stdout,
            &mut stderr,
            prev,
            line,
            cursor,
            prompt,
            ps2,
            ps1_w,
            ps2_w,
            cols,
        );
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
        let mut stdout = |b: &[u8]| emit_stdout(b);
        let mut stderr = |b: &[u8]| emit_stderr(b);
        let new_snapshot = full_repaint_into(
            &mut stdout,
            &mut stderr,
            prev_cursor_row,
            line,
            cursor,
            prompt,
            ps2,
            ps1_w,
            ps2_w,
            cols_opt,
        );
        anchor.prev = new_snapshot;
    }
}

/// Build the bytes the redraw would emit, without touching any file
/// descriptor. Useful for unit tests that assert the produced
/// control sequences directly.
///
/// Returns `(to_stdout, to_stderr)`. On the incremental path
/// `to_stderr` is empty unless the new tail crosses an embedded
/// `\n` — in which case it contains one copy of `ps2` per such
/// crossing.
pub(crate) fn redraw_sequence(
    anchor: &mut DrawAnchor,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    ps2: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::with_capacity(line.len() + 32);
    let mut stderr = Vec::new();
    redraw_internal(
        anchor,
        line,
        cursor,
        prompt,
        ps2,
        |b| stdout.extend_from_slice(b),
        |b| stderr.extend_from_slice(b),
    );
    (stdout, stderr)
}

/// Emit the redraw sequence to `stdout` (buffer + positioning) and
/// `stderr` (PS1 on the full-repaint path, plus one PS2 per
/// embedded `\n` in the new tail on both paths).
pub(crate) fn redraw(
    anchor: &mut DrawAnchor,
    line: &[u8],
    cursor: usize,
    prompt: &[u8],
    ps2: &[u8],
) {
    redraw_internal(
        anchor,
        line,
        cursor,
        prompt,
        ps2,
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

    /// Build a `DrawAnchor` describing the post-redraw state of
    /// `line` rendered under `prompt` (PS1) and `ps2` with cursor at
    /// byte `cursor`. Mirrors what the renderer would have left on
    /// the snapshot after a real redraw.
    fn snapshot_at(
        line: &[u8],
        prompt: &[u8],
        ps2: &[u8],
        cols: usize,
        cursor: usize,
    ) -> DrawAnchor {
        let ps1_w = prompt_visible_width(prompt);
        let ps2_w = prompt_visible_width(ps2);
        let end_pos = walk(line, line.len(), ps1_w, ps2_w, cols);
        let cursor_target = normalize(walk(line, cursor, ps1_w, ps2_w, cols), cols);
        DrawAnchor {
            prev: Some(Snapshot {
                line: line.to_vec(),
                prompt: prompt.to_vec(),
                ps2: ps2.to_vec(),
                cols,
                cursor_row: cursor_target.row,
                cursor_col: cursor_target.col,
                end_row: end_pos.row,
                end_col: end_pos.col,
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
    fn grapheme_motion_groups_combining_marks() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // "m̄" = 'm' (1 byte) + U+0304 COMBINING MACRON (2 bytes,
            // zero width). The whole grapheme is one visible cell and
            // must move under the cursor as a unit.
            let line = b"m\xcc\x84"; // [0x6d, 0xcc, 0x84]
            assert_eq!(display_width(line), 1);
            // Plain char motion would stop between 'm' and the mark…
            assert_eq!(char_len_at(line, 0), 1);
            // …grapheme motion steps over both at once.
            assert_eq!(grapheme_len_at(line, 0), 3);
            assert_eq!(prev_grapheme_start(line, 3), 0);

            // A base char followed by *two* stacked marks groups all
            // three codepoints: 'e' + U+0301 + U+0304.
            let stacked = b"e\xcc\x81\xcc\x84";
            assert_eq!(grapheme_len_at(stacked, 0), 5);
            assert_eq!(prev_grapheme_start(stacked, 5), 0);
        });
    }

    #[test]
    fn grapheme_motion_does_not_cross_newline() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // `\n` is zero-width to `wcwidth` but is the editor's
            // logical-line separator; it must remain an independently
            // addressable position rather than being absorbed into the
            // preceding grapheme.
            let line = b"a\nb";
            assert_eq!(grapheme_len_at(line, 0), 1);
            assert_eq!(prev_grapheme_start(line, 2), 1); // back from 'b' lands on '\n'
            // And a grapheme that ends right before the newline stops
            // at the newline boundary going forward.
            let accented = b"m\xcc\x84\nx"; // "m̄\nx"
            assert_eq!(grapheme_len_at(accented, 0), 3);
            assert_eq!(prev_grapheme_start(accented, 4), 3); // '\n' at index 3
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

    // ---------- walk (the new screen-position walker) ----------

    #[test]
    fn walk_single_line_no_wrap() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // ps1_w=2, ps2_w=2, cols=80
            assert_eq!(walk(b"abc", 0, 2, 2, 80), ScreenPos { row: 0, col: 2 });
            assert_eq!(walk(b"abc", 3, 2, 2, 80), ScreenPos { row: 0, col: 5 });
        });
    }

    #[test]
    fn walk_newline_advances_to_ps2_column() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // b"foo\nbar" with ps1_w=2, ps2_w=2, cols=80:
            //   row 0: "$ foo"  end col 5
            //   row 1: "> bar"  end col 5
            assert_eq!(walk(b"foo\nbar", 7, 2, 2, 80), ScreenPos { row: 1, col: 5 });
            assert_eq!(walk(b"foo\nbar", 4, 2, 2, 80), ScreenPos { row: 1, col: 2 });
            assert_eq!(walk(b"foo\nbar", 3, 2, 2, 80), ScreenPos { row: 0, col: 5 });
        });
    }

    #[test]
    fn walk_multiple_newlines() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // b"a\nbcd\nefg" with ps1_w=2, ps2_w=2, cols=80:
            //   row 0: "$ a"     end col 3
            //   row 1: "> bcd"   end col 5
            //   row 2: "> efg"   end col 5
            assert_eq!(
                walk(b"a\nbcd\nefg", 9, 2, 2, 80),
                ScreenPos { row: 2, col: 5 }
            );
        });
    }

    #[test]
    fn walk_pending_wrap_at_exact_column_boundary() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // b"abcdefgh" with ps1_w=2, cols=10: 8 chars + 2 = 10 cells,
            // hitting the right edge exactly. Raw walker reports
            // col == cols (pending-wrap); normalize() promotes to
            // (1, 0).
            assert_eq!(
                walk(b"abcdefgh", 8, 2, 2, 10),
                ScreenPos { row: 0, col: 10 }
            );
            assert_eq!(
                normalize(walk(b"abcdefgh", 8, 2, 2, 10), 10),
                ScreenPos { row: 1, col: 0 }
            );
        });
    }

    #[test]
    fn walk_wraps_continuation_to_col_zero_not_ps2_width() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // Terminal-width wraps don't carry the PS2 gutter (we only
            // emit PS2 for explicit \n in the buffer). cols=5, ps1_w=2,
            // ps2_w=2, b"abcdef" (6 chars): row 0 "$ abc" col=5
            // (pending-wrap normalized), 'd' commits wrap and lands at
            // (1, 1) — NOT (1, 3) which it would be if wraps inherited
            // the gutter.
            assert_eq!(walk(b"abcdef", 6, 2, 2, 5), ScreenPos { row: 1, col: 3 });
        });
    }

    #[test]
    fn walk_newline_after_pending_wrap_lands_on_continuation_row() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // cols=5, ps1_w=2, ps2_w=2, b"abc\nx": "$ abc" fills the
            // row exactly (pending-wrap), then \n commits + advances
            // one row (so we end on row 1 not row 2), then PS2 lands
            // us at col 2, then 'x' → col 3.
            //
            // Walker commits the pending wrap from the previous
            // iteration before processing the \n. End position:
            // (2, 3).
            assert_eq!(walk(b"abc\nx", 5, 2, 2, 5), ScreenPos { row: 2, col: 3 });
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
                let (out, err) = redraw_sequence(&mut anchor, b"abc", 3, b"$ ", b"> ");
                assert_eq!(out, b"\r\x1b[Jabc\r\x1b[5C");
                assert_eq!(err, b"$ ");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 0);
                assert_eq!(prev.cursor_col, 5);
                assert_eq!(prev.end_row, 0);
                assert_eq!(prev.end_col, 5);
            });
        });
    }

    #[test]
    fn full_repaint_first_call_short_line_cursor_in_middle() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let (out, _) = redraw_sequence(&mut anchor, b"abc", 1, b"$ ", b"> ");
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
                let (out, _) = redraw_sequence(&mut anchor, line, line.len(), b"$ ", b"> ");
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
                let (out, _) = redraw_sequence(&mut anchor, line, 0, b"$ ", b"> ");
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
                let (out, _) = redraw_sequence(&mut anchor, line, line.len(), b"$ ", b"> ");
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
                let (out, _) = redraw_sequence(&mut anchor, b"abc", 1, b"$ ", b"> ");
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
                let (out, _) = redraw_sequence(&mut anchor, line, 2, b"$ ", b"> ");
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
                    b"> ",
                    10,
                    27,
                );
                // Now redraw with "hi" under a *different* prompt so
                // the incremental path is rejected and we go through
                // full-repaint.
                let (out, err) = redraw_sequence(&mut anchor, b"hi", 2, b"# ", b"> ");
                // 27 + 2 = 29 cells, ends on row 2 col 9, so we climb
                // 2 rows and wipe.
                assert_eq!(out, b"\x1b[2A\r\x1b[Jhi\r\x1b[4C");
                assert_eq!(err, b"# ");
            });
        });
    }

    // ---------- full-repaint with embedded newlines ----------

    #[test]
    fn full_repaint_emits_ps2_between_rows() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let (out, err) = redraw_sequence(&mut anchor, b"foo\nbar", 7, b"$ ", b"> ");
                // stdout: clear, "foo", "\n", "bar", positioning.
                // stderr: "$ " (PS1) then "> " (PS2 after the \n).
                // End at row 1 col 5, cursor at row 1 col 5: no
                // vertical motion, \r + 5C.
                assert_eq!(out, b"\r\x1b[Jfoo\nbar\r\x1b[5C");
                assert_eq!(err, b"$ > ");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.end_row, 1);
                assert_eq!(prev.end_col, 5);
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 5);
            });
        });
    }

    #[test]
    fn full_repaint_multi_line_cursor_at_start_climbs_back() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let (out, err) = redraw_sequence(&mut anchor, b"foo\nbar", 0, b"$ ", b"> ");
                // Cursor at start: target (0, 2). End at (1, 5).
                // \r + 1A + 2C.
                assert_eq!(out, b"\r\x1b[Jfoo\nbar\r\x1b[1A\x1b[2C");
                assert_eq!(err, b"$ > ");
            });
        });
    }

    #[test]
    fn full_repaint_three_logical_lines() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = DrawAnchor::new();
                let line = b"for i in 1 2 3\n  echo $i\ndone";
                let cursor = line.len();
                let (_, err) = redraw_sequence(&mut anchor, line, cursor, b"$ ", b"> ");
                // PS1 once + PS2 twice.
                assert_eq!(err, b"$ > > ");
                let prev = anchor.prev.as_ref().unwrap();
                // Row 2: "> done" → col 6.
                assert_eq!(prev.end_row, 2);
                assert_eq!(prev.end_col, 6);
                assert_eq!(prev.cursor_row, 2);
                assert_eq!(prev.cursor_col, 6);
            });
        });
    }

    // ---------- incremental path ----------

    #[test]
    fn incremental_self_insert_at_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hel", b"$ ", b"> ", 80, 3);
                let (out, err) = redraw_sequence(&mut anchor, b"hell", 4, b"$ ", b"> ");
                // Cursor already at the diff point (col 5), so we
                // just write the new char and then issue `\r\x1b[6C`
                // to land on the final cursor position.
                assert_eq!(out, b"l\r\x1b[6C");
                // Prompt is NOT repainted on the incremental path.
                assert_eq!(err, b"");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.line, b"hell");
                assert_eq!(prev.cursor_col, 6);
                assert_eq!(prev.end_row, 0);
                assert_eq!(prev.end_col, 6);
            });
        });
    }

    #[test]
    fn incremental_backspace_at_end() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hellx", b"$ ", b"> ", 80, 5);
                let (out, err) = redraw_sequence(&mut anchor, b"hell", 4, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 80, 5);
                // Insert 'X' at position 2: "heXllo", cursor moves to 3.
                let (out, _) = redraw_sequence(&mut anchor, b"heXllo", 3, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 80, 3);
                // Backspace at cursor=3: "hllo", cursor moves to 2.
                let (out, _) = redraw_sequence(&mut anchor, b"hllo", 2, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 80, 5);
                // Same buffer, cursor moves from 5 to 3 (e.g. left x2).
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 3, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"abcdefghi", b"$ ", b"> ", 10, 9);
                // Type 'j': "abcdefghij" (10 chars + 2 = 12 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcdefghij", 10, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"abcdefg", b"$ ", b"> ", 10, 7);
                // Type "hi": "abcdefghi" (9 chars + 2 = 11 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcdefghi", 9, b"$ ", b"> ");
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
                        ps2: b"> ".to_vec(),
                        cols: 10,
                        cursor_row: 0,
                        cursor_col: 6,
                        end_row: 1,
                        end_col: 10,
                    }),
                };
                // C-k from cursor 4: "abcd" (4 chars + 2 = 6 cells).
                let (out, _) = redraw_sequence(&mut anchor, b"abcd", 4, b"$ ", b"> ");
                // From (0, 6) diff at byte 4, position (0, 6). No
                // move. Empty tail. Shrink from (2, 0) to (0, 6):
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
                let mut anchor = snapshot_at(b"\xc3\xa9x", b"$ ", b"> ", 80, 3);
                // Replace "é" with "è" — same byte count, same width.
                let (out, _) = redraw_sequence(&mut anchor, b"\xc3\xa8x", 3, b"$ ", b"> ");
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
                let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 80, 0);
                // anchor.prev.cursor_col is 2 (just after prompt).
                let (out, _) = redraw_sequence(&mut anchor, b"", 0, b"$ ", b"> ");
                // Cursor already at diff (0, 2). Empty tail. Shrink
                // same-row → \x1b[K. \r + 2C.
                assert_eq!(out, b"\x1b[K\r\x1b[2C");
            });
        });
    }

    // ---------- incremental path with embedded newlines ----------

    #[test]
    fn incremental_append_newline_emits_ps2() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                // Start with "foo" rendered, cursor at end.
                let mut anchor = snapshot_at(b"foo", b"$ ", b"> ", 80, 3);
                // Append "\n" — cursor still after the \n, on row 1.
                let (out, err) = redraw_sequence(&mut anchor, b"foo\n", 4, b"$ ", b"> ");
                // No motion needed (cursor at diff). The diff tail
                // contains a `\n`, so a `\x1b[J` is emitted up front
                // to wipe any stale screen tail before we leave the
                // current row. Then write "\n" to stdout and "> "
                // to stderr. End at (1, 2). Cursor at (1, 2).
                // \r + 2C.
                assert_eq!(out, b"\x1b[J\n\r\x1b[2C");
                assert_eq!(err, b"> ");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.end_row, 1);
                assert_eq!(prev.end_col, 2);
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 2);
            });
        });
    }

    #[test]
    fn incremental_append_newline_and_text_emits_ps2_before_tail() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"foo", b"$ ", b"> ", 80, 3);
                // Paste "\nbar" at end.
                let (out, err) = redraw_sequence(&mut anchor, b"foo\nbar", 7, b"$ ", b"> ");
                // Cursor at diff (0, 5). The diff tail contains a
                // `\n`, so a pre-tail `\x1b[J` wipes any stale
                // screen content past the diff. Then write "\nbar":
                // stdout gets "\n" then "bar"; stderr gets "> "
                // between them. End at (1, 5). \r + 5C.
                assert_eq!(out, b"\x1b[J\nbar\r\x1b[5C");
                assert_eq!(err, b"> ");
            });
        });
    }

    #[test]
    fn incremental_insert_newline_mid_buffer_wipes_stale_tail_on_old_row() {
        // Regression: pressing Alt-Enter in the middle of a single
        // line used to leave the post-cursor bytes of the *old* row
        // on-screen, because the new tail starts with `\n` which
        // immediately leaves the row before overwriting those bytes,
        // and the post-write shrinkage check sees `(new_end_row+1,
        // _)` > `(old_end_row, _)` and skips the wipe. The fix is
        // to emit `\x1b[J` up front whenever the new tail contains
        // a `\n`.
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                // Old state: "echo foo bar" with cursor parked
                // between "foo" and " bar" (byte 8).
                let mut anchor = snapshot_at(b"echo foo bar", b"$ ", b"> ", 80, 8);
                // Press Alt-Enter at the cursor → buffer becomes
                // "echo foo\n bar", cursor now at byte 9 (start of
                // the new continuation row).
                let (out, err) = redraw_sequence(&mut anchor, b"echo foo\n bar", 9, b"$ ", b"> ");
                // Diff byte = 8 (space → `\n`). Diff target is
                // (0, 10). The prev cursor was at (0, 10) as well,
                // so no motion is emitted before the wipe. `\x1b[J`
                // wipes " bar" from the old row 0 tail; we then
                // write "\n" + " bar" with a PS2 in between.
                // End at (1, 5). Cursor target = byte 9 of new
                // buffer = column 2 of row 1 (the PS2-padded start
                // of the second logical row). \r + 2C.
                assert_eq!(out, b"\x1b[J\n bar\r\x1b[2C");
                assert_eq!(err, b"> ");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.end_row, 1);
                assert_eq!(prev.end_col, 6);
                assert_eq!(prev.cursor_row, 1);
                assert_eq!(prev.cursor_col, 2);
            });
        });
    }

    #[test]
    fn incremental_cursor_navigates_to_first_line_after_paste() {
        // The headline bug: after a multi-line paste, Ctrl-a should
        // land the cursor on the *prompt* row, not on the last row
        // of the pasted region.
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"foo\nbar", b"$ ", b"> ", 80, 7);
                // Same buffer, cursor → 0 (Ctrl-a from end).
                let (out, _) = redraw_sequence(&mut anchor, b"foo\nbar", 0, b"$ ", b"> ");
                // Empty tail. From (1, 5) to diff (1, 5): no move.
                // No shrink. \r + 1A + 2C: climb one row, walk to
                // the PS1's column.
                assert_eq!(out, b"\r\x1b[1A\x1b[2C");
                let prev = anchor.prev.as_ref().unwrap();
                assert_eq!(prev.cursor_row, 0);
                assert_eq!(prev.cursor_col, 2);
            });
        });
    }

    // ---------- full-repaint trigger ----------

    #[test]
    fn prompt_change_forces_full_repaint() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 80, 5);
                // Change prompt: incremental must be rejected.
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 5, b"# ", b"> ");
                // Full repaint: prefix + body + position.
                assert_eq!(out, b"\r\x1b[Jhello\r\x1b[7C");
                assert_eq!(err, b"# ");
            });
        });
    }

    #[test]
    fn ps2_change_forces_full_repaint() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            with_cols(Some(80), || {
                let mut anchor = snapshot_at(b"foo\nbar", b"$ ", b"> ", 80, 7);
                // Change ps2 from "> " to ".. ": incremental rejected.
                let (out, err) = redraw_sequence(&mut anchor, b"foo\nbar", 7, b"$ ", b".. ");
                // Full repaint: PS1 + PS2 (new) re-emitted to stderr.
                assert_eq!(err, b"$ .. ");
                // Previous draw left the cursor on row 1; full repaint
                // climbs back one row before clearing to end-of-screen.
                // End at row 1 col 6 ("..bar" → col 3+3=6). Cursor
                // target also (1, 6). \r + 6C.
                assert_eq!(out, b"\x1b[1A\r\x1b[Jfoo\nbar\r\x1b[6C");
            });
        });
    }

    #[test]
    fn cols_change_forces_full_repaint() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // Pre-seed with cols=10 then redraw under cols=20.
            let mut anchor = snapshot_at(b"hello", b"$ ", b"> ", 10, 5);
            with_cols(Some(20), || {
                let (out, err) = redraw_sequence(&mut anchor, b"hello", 5, b"$ ", b"> ");
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
