//! Implementation of every emacs bindable function (spec § 5).
//!
//! The outer dispatch loop in [`super::read_line`] calls [`apply`] with
//! the selected [`EmacsFn`] and the last read byte, which is needed by
//! `self-insert` and `quoted-insert`. Each function is small and
//! testable in isolation; coverage for the I/O side is provided by the
//! PTY integration tests in `tests/integration/emacs_mode.rs`.

use crate::hash::ShellSet;
use crate::shell::state::Shell;
use crate::sys;

use super::super::editor::history_search::{Direction, find_prefix};
use super::super::editor::input::write_bytes;
use super::super::editor::redraw::{char_len_at, display_width, prev_char_start};
use super::super::editor::words::{WordClass, next_word_boundary, prev_word_boundary};
use super::completion_context::{CompletionContext, classify_completion_context};
use super::keymap::EmacsFn;
use super::kill_buffer::KillDirection;
use super::state::{EmacsState, YankArgState};
use super::undo::UndoEntry;

/// The result of applying one bindable function. The outer loop uses
/// these flags to decide whether to keep reading, accept the line,
/// ring the bell, or terminate due to EOF.
#[derive(Clone, Debug, Default)]
pub(crate) struct Outcome {
    pub accepted: bool,
    pub eof: bool,
    pub bell: bool,
    /// When set, the outer loop should `read_byte`, treat whatever it
    /// returns literally (bypassing dispatch), insert it, and continue.
    /// This models `C-q` / `C-v` (`quoted-insert`).
    pub quoted_insert: bool,
    /// When set, the outer loop should run the external editor and
    /// either return its contents as a submitted line or redraw the
    /// current line if the external editor left the temp file empty.
    pub edit_and_execute: Option<Vec<u8>>,
}

/// Evaluate `func` against `state` / `shell`. Returns an [`Outcome`]
/// describing what the outer dispatch loop should do next. The
/// `trigger_byte` argument is the final byte of the key sequence that
/// resolved to `func` — used by `self-insert`.
pub(crate) fn apply(
    shell: &mut Shell,
    state: &mut EmacsState,
    func: EmacsFn,
    trigger_byte: u8,
) -> Outcome {
    let mut out = Outcome::default();

    // Clear yank-last-arg walk state unless the incoming function is
    // itself yank-last-arg (see spec § 5.3).
    if func != EmacsFn::YankLastArg {
        state.yank_last_arg = None;
    }

    // Kill-buffer append/replace contract: any non-kill function
    // marks the buffer so the next kill replaces instead of appends.
    if !is_kill_function(func) {
        state.kill.mark_non_kill();
    }

    match func {
        EmacsFn::SelfInsert => do_self_insert(state, trigger_byte),
        EmacsFn::BeginningOfLine => state.cursor = 0,
        EmacsFn::EndOfLine => state.cursor = state.buf.len(),
        EmacsFn::ForwardChar => {
            if state.cursor >= state.buf.len() {
                out.bell = true;
            } else {
                let n = state.cursor + char_len_at(&state.buf, state.cursor);
                state.cursor = n.min(state.buf.len());
            }
        }
        EmacsFn::BackwardChar => {
            if state.cursor == 0 {
                out.bell = true;
            } else {
                state.cursor = prev_char_start(&state.buf, state.cursor);
            }
        }
        EmacsFn::ForwardWord => {
            state.cursor = next_word_boundary(&state.buf, state.cursor, WordClass::AlnumUnderscore)
        }
        EmacsFn::BackwardWord => {
            state.cursor = prev_word_boundary(&state.buf, state.cursor, WordClass::AlnumUnderscore)
        }
        EmacsFn::ClearScreen => write_bytes(b"\x1b[H\x1b[2J"),
        EmacsFn::BackwardDeleteChar => do_backward_delete_char(state, &mut out),
        EmacsFn::DeleteChar => do_delete_char(state, &mut out),
        EmacsFn::KillLine => do_kill_line(state),
        EmacsFn::UnixLineDiscard => do_unix_line_discard(state),
        EmacsFn::UnixWordRubout => do_unix_word_rubout(state),
        EmacsFn::KillWord => do_kill_word(state),
        EmacsFn::BackwardKillWord => do_backward_kill_word(state),
        EmacsFn::Yank => do_yank(state, &mut out),
        EmacsFn::TransposeChars => do_transpose_chars(state, &mut out),
        EmacsFn::TransposeWords => do_transpose_words(state, &mut out),
        EmacsFn::UpcaseWord => do_case_word(state, CaseOp::Upper),
        EmacsFn::DowncaseWord => do_case_word(state, CaseOp::Lower),
        EmacsFn::CapitalizeWord => do_case_word(state, CaseOp::Capitalize),
        EmacsFn::QuotedInsert => out.quoted_insert = true,
        EmacsFn::Complete => do_complete(shell, state, &mut out),
        EmacsFn::AcceptLine => {
            state.undo.clear();
            out.accepted = true;
        }
        EmacsFn::Undo => {
            if !state.undo.undo(&mut state.buf, &mut state.cursor) {
                out.bell = true;
            }
        }
        EmacsFn::Abort => {
            // C-g's sole job (spec § 5.9) is to abort a composite action
            // (incremental search, `quoted-insert`). Those composites
            // are handled directly by the outer dispatch loop and never
            // reach `apply()`, so if we're here there was nothing to
            // abort — ring the bell.
            out.bell = true;
        }
        EmacsFn::SendSigint => {
            // C-c: emulate the "discard current line, prompt again"
            // behavior without actually raising SIGINT. The shell is
            // already handling SIGINT via its sigaction; raising it
            // here would kill the interactive session.
            state.buf.clear();
            state.cursor = 0;
            state.undo.clear();
            write_bytes(b"^C\r\n");
        }
        EmacsFn::PreviousHistory => do_history_step(shell, state, -1, &mut out),
        EmacsFn::NextHistory => do_history_step(shell, state, 1, &mut out),
        EmacsFn::BeginningOfHistory => do_history_jump(shell, state, true),
        EmacsFn::EndOfHistory => do_history_jump(shell, state, false),
        EmacsFn::HistorySearchBackward => {
            do_history_search_prefix(shell, state, Direction::Backward)
        }
        EmacsFn::HistorySearchForward => do_history_search_prefix(shell, state, Direction::Forward),
        EmacsFn::ReverseSearchHistory | EmacsFn::ForwardSearchHistory => {
            // These are handled by the outer dispatch loop as a
            // search-mini-buffer mode. Treat apply() being called for
            // them as a no-op so the dispatch loop can catch them
            // before we arrive here.
        }
        EmacsFn::YankLastArg => do_yank_last_arg(shell, state, &mut out),
        EmacsFn::EditAndExecuteCommand => {
            let tmp_path = editor_temp_path();
            if let Ok(fd) = sys::fs::open_file(
                &tmp_path,
                sys::constants::O_WRONLY | sys::constants::O_CREAT | sys::constants::O_TRUNC,
                0o600,
            ) {
                let _ = sys::fd_io::write_all_fd(fd, &state.buf);
                let _ = sys::fd_io::write_all_fd(fd, b"\n");
                let _ = sys::fd_io::close_fd(fd);
            }
            out.edit_and_execute = Some(tmp_path);
        }
    }

    state.last_fn = Some(func);
    out
}

/// Invoke the external shell command bound via `bind -x`. The command
/// string is executed in the shell with `READLINE_LINE` /
/// `READLINE_POINT` exported as environment variables, then the
/// buffer and cursor are restored from whatever the command assigned
/// back. Unused flags in the outcome default to false.
pub(crate) fn run_bind_x(shell: &mut Shell, state: &mut EmacsState, command: &[u8]) {
    // Snapshot current line / point into env so the command can read
    // them.
    let cursor_str = format!("{}", state.cursor);
    let prev_line = shell.get_var(b"READLINE_LINE").map(|b| b.to_vec());
    let prev_point = shell.get_var(b"READLINE_POINT").map(|b| b.to_vec());
    let _ = shell.set_var(b"READLINE_LINE", &state.buf);
    let _ = shell.set_var(b"READLINE_POINT", cursor_str.as_bytes());

    let _ = shell.execute_string(command);

    if let Some(new_line) = shell.get_var(b"READLINE_LINE").map(|b| b.to_vec()) {
        state.buf = new_line;
    }
    if let Some(new_point) = shell.get_var(b"READLINE_POINT").map(|b| b.to_vec()) {
        if let Ok(s) = std::str::from_utf8(&new_point) {
            if let Ok(n) = s.trim().parse::<usize>() {
                state.cursor = n.min(state.buf.len());
            }
        }
    }

    // Restore previous values (or remove if unset before).
    match prev_line {
        Some(v) => {
            let _ = shell.set_var(b"READLINE_LINE", &v);
        }
        None => {
            shell.env_mut().remove(b"READLINE_LINE");
        }
    }
    match prev_point {
        Some(v) => {
            let _ = shell.set_var(b"READLINE_POINT", &v);
        }
        None => {
            shell.env_mut().remove(b"READLINE_POINT");
        }
    }
    state.undo.clear();
}

// --- helpers ----------------------------------------------------------

fn is_kill_function(f: EmacsFn) -> bool {
    matches!(
        f,
        EmacsFn::KillLine
            | EmacsFn::UnixLineDiscard
            | EmacsFn::UnixWordRubout
            | EmacsFn::KillWord
            | EmacsFn::BackwardKillWord
    )
}

fn do_self_insert(state: &mut EmacsState, byte: u8) {
    state.insert_bytes_at_cursor(&[byte]);
}

fn do_backward_delete_char(state: &mut EmacsState, out: &mut Outcome) {
    if state.cursor == 0 {
        out.bell = true;
        return;
    }
    let start = prev_char_start(&state.buf, state.cursor);
    let removed: Vec<u8> = state.buf[start..state.cursor].to_vec();
    state.buf.drain(start..state.cursor);
    state.cursor = start;
    state.undo.push(UndoEntry::Killed {
        at: start,
        bytes: removed,
    });
}

fn do_delete_char(state: &mut EmacsState, out: &mut Outcome) {
    if state.cursor >= state.buf.len() {
        if state.buf.is_empty() {
            // C-d on empty line is EOF.
            out.eof = true;
        } else {
            out.bell = true;
        }
        return;
    }
    let end = state.cursor + char_len_at(&state.buf, state.cursor);
    let removed: Vec<u8> = state.buf[state.cursor..end].to_vec();
    state.buf.drain(state.cursor..end);
    state.undo.push(UndoEntry::Killed {
        at: state.cursor,
        bytes: removed,
    });
}

fn do_kill_line(state: &mut EmacsState) {
    let at = state.cursor;
    let killed: Vec<u8> = state.buf.drain(at..).collect();
    if killed.is_empty() {
        return;
    }
    state.undo.push(UndoEntry::Killed {
        at,
        bytes: killed.clone(),
    });
    state.kill.kill(killed, KillDirection::Forward);
}

fn do_unix_line_discard(state: &mut EmacsState) {
    if state.cursor == 0 {
        return;
    }
    let killed: Vec<u8> = state.buf.drain(0..state.cursor).collect();
    state.undo.push(UndoEntry::Killed {
        at: 0,
        bytes: killed.clone(),
    });
    state.cursor = 0;
    state.kill.kill(killed, KillDirection::Backward);
}

fn do_unix_word_rubout(state: &mut EmacsState) {
    let start = prev_word_boundary(&state.buf, state.cursor, WordClass::Whitespace);
    if start == state.cursor {
        return;
    }
    let killed: Vec<u8> = state.buf.drain(start..state.cursor).collect();
    state.undo.push(UndoEntry::Killed {
        at: start,
        bytes: killed.clone(),
    });
    state.cursor = start;
    state.kill.kill(killed, KillDirection::Backward);
}

fn do_kill_word(state: &mut EmacsState) {
    let end = next_word_boundary(&state.buf, state.cursor, WordClass::AlnumUnderscore);
    if end == state.cursor {
        return;
    }
    let killed: Vec<u8> = state.buf.drain(state.cursor..end).collect();
    state.undo.push(UndoEntry::Killed {
        at: state.cursor,
        bytes: killed.clone(),
    });
    state.kill.kill(killed, KillDirection::Forward);
}

fn do_backward_kill_word(state: &mut EmacsState) {
    let start = prev_word_boundary(&state.buf, state.cursor, WordClass::AlnumUnderscore);
    if start == state.cursor {
        return;
    }
    let killed: Vec<u8> = state.buf.drain(start..state.cursor).collect();
    state.undo.push(UndoEntry::Killed {
        at: start,
        bytes: killed.clone(),
    });
    state.cursor = start;
    state.kill.kill(killed, KillDirection::Backward);
}

fn do_yank(state: &mut EmacsState, out: &mut Outcome) {
    if state.kill.is_empty() {
        out.bell = true;
        return;
    }
    let bytes: Vec<u8> = state.kill.as_slice().to_vec();
    let at = state.cursor;
    state.buf.splice(at..at, bytes.iter().copied());
    state.cursor = at + bytes.len();
    state.undo.push(UndoEntry::Yanked {
        at,
        len: bytes.len(),
    });
}

fn do_transpose_chars(state: &mut EmacsState, out: &mut Outcome) {
    let len = state.buf.len();
    if len < 2 {
        out.bell = true;
        return;
    }
    // C-t at end-of-line transposes the last two chars.
    let (a_start, a_len, b_start, b_len) = if state.cursor >= len {
        let b_start = prev_char_start(&state.buf, len);
        let a_start = prev_char_start(&state.buf, b_start);
        let a_len = b_start - a_start;
        let b_len = len - b_start;
        (a_start, a_len, b_start, b_len)
    } else if state.cursor == 0 {
        out.bell = true;
        return;
    } else {
        let a_start = prev_char_start(&state.buf, state.cursor);
        let a_len = state.cursor - a_start;
        let b_start = state.cursor;
        let b_len = char_len_at(&state.buf, b_start);
        (a_start, a_len, b_start, b_len)
    };
    let a: Vec<u8> = state.buf[a_start..a_start + a_len].to_vec();
    let b: Vec<u8> = state.buf[b_start..b_start + b_len].to_vec();
    let total = a_len + b_len;
    let mut merged = Vec::with_capacity(total);
    merged.extend_from_slice(&b);
    merged.extend_from_slice(&a);
    state
        .buf
        .splice(a_start..a_start + total, merged.into_iter());
    state.cursor = a_start + total;
    state.undo.push(UndoEntry::TransposeChars {
        at: a_start,
        a_len,
        b_len,
    });
}

fn do_transpose_words(state: &mut EmacsState, out: &mut Outcome) {
    use super::super::editor::words::is_word_char_at;

    let buf = state.buf.clone();

    // Identify the *right* word (the one that ends up to the left of
    // the cursor after the swap, per spec § 5.6):
    //   - If the cursor is inside a word, that is the right word.
    //   - If the cursor is at EOB or on non-word chars, use the last
    //     word ending at/before the cursor. If no such word exists,
    //     use the next word after the cursor.
    let right_end = if state.cursor < buf.len() && is_word_char_at(&buf, state.cursor) {
        let mut p = state.cursor;
        while p < buf.len() && is_word_char_at(&buf, p) {
            p += char_len_at(&buf, p);
        }
        p
    } else {
        let mut p = state.cursor.min(buf.len());
        while p > 0 && !is_word_char_at(&buf, p - 1) {
            p -= 1;
        }
        if p == 0 {
            // No word behind the cursor; try to use the next word.
            let mut q = state.cursor.min(buf.len());
            while q < buf.len() && !is_word_char_at(&buf, q) {
                q += 1;
            }
            if q == buf.len() {
                out.bell = true;
                return;
            }
            while q < buf.len() && is_word_char_at(&buf, q) {
                q += char_len_at(&buf, q);
            }
            q
        } else {
            p
        }
    };

    let mut right_start = right_end;
    while right_start > 0 && is_word_char_at(&buf, right_start - 1) {
        right_start -= 1;
    }

    let mut left_end = right_start;
    while left_end > 0 && !is_word_char_at(&buf, left_end - 1) {
        left_end -= 1;
    }
    if left_end == 0 {
        out.bell = true;
        return;
    }

    let mut left_start = left_end;
    while left_start > 0 && is_word_char_at(&buf, left_start - 1) {
        left_start -= 1;
    }

    let left_len = left_end - left_start;
    let right_len = right_end - right_start;
    let gap_len = right_start - left_end;
    let left: Vec<u8> = buf[left_start..left_end].to_vec();
    let gap: Vec<u8> = buf[left_end..right_start].to_vec();
    let right: Vec<u8> = buf[right_start..right_end].to_vec();
    let total = left_len + gap_len + right_len;
    let mut merged = Vec::with_capacity(total);
    merged.extend_from_slice(&right);
    merged.extend_from_slice(&gap);
    merged.extend_from_slice(&left);
    state
        .buf
        .splice(left_start..left_start + total, merged.into_iter());
    state.cursor = left_start + total;
    state.undo.push(UndoEntry::TransposeWords {
        at: left_start,
        left_len,
        gap_len,
        right_len,
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaseOp {
    Upper,
    Lower,
    Capitalize,
}

fn do_case_word(state: &mut EmacsState, op: CaseOp) {
    let end = next_word_boundary(&state.buf, state.cursor, WordClass::AlnumUnderscore);
    if end == state.cursor {
        return;
    }
    let before: Vec<u8> = state.buf[state.cursor..end].to_vec();
    let mut after = before.clone();
    apply_case(&mut after, op);
    state.buf.splice(state.cursor..end, after.iter().copied());
    state.undo.push(UndoEntry::CaseChange {
        at: state.cursor,
        before,
    });
    state.cursor = end;
}

fn apply_case(bytes: &mut [u8], op: CaseOp) {
    let mut first_letter_seen = false;
    for b in bytes.iter_mut() {
        let ch = *b;
        match op {
            CaseOp::Upper => {
                if ch.is_ascii_lowercase() {
                    *b = ch.to_ascii_uppercase();
                }
            }
            CaseOp::Lower => {
                if ch.is_ascii_uppercase() {
                    *b = ch.to_ascii_lowercase();
                }
            }
            CaseOp::Capitalize => {
                if !first_letter_seen && ch.is_ascii_alphabetic() {
                    *b = ch.to_ascii_uppercase();
                    first_letter_seen = true;
                } else if ch.is_ascii_alphabetic() {
                    *b = ch.to_ascii_lowercase();
                }
            }
        }
    }
}

fn do_history_step(shell: &Shell, state: &mut EmacsState, delta: i32, out: &mut Outcome) {
    let hist = shell.history();
    let len = hist.len();
    if len == 0 {
        out.bell = true;
        return;
    }
    let current = match state.hist_index {
        Some(i) => i,
        None => {
            state.edit_line = state.buf.clone();
            len
        }
    };
    let new = current as i32 + delta;
    if new < 0 || new > len as i32 {
        out.bell = true;
        return;
    }
    let new = new as usize;
    if new == len {
        state.buf = std::mem::take(&mut state.edit_line);
        state.hist_index = None;
    } else {
        state.buf = hist[new].to_vec();
        state.hist_index = Some(new);
    }
    state.cursor = state.buf.len();
    state.undo.clear();
}

fn do_history_jump(shell: &Shell, state: &mut EmacsState, to_beginning: bool) {
    let hist = shell.history();
    if hist.is_empty() {
        return;
    }
    if state.hist_index.is_none() {
        state.edit_line = state.buf.clone();
    }
    if to_beginning {
        state.hist_index = Some(0);
        state.buf = hist[0].to_vec();
    } else {
        state.hist_index = None;
        state.buf = std::mem::take(&mut state.edit_line);
    }
    state.cursor = state.buf.len();
    state.undo.clear();
}

fn do_history_search_prefix(shell: &Shell, state: &mut EmacsState, direction: Direction) {
    let hist = shell.history();
    if hist.is_empty() {
        return;
    }
    // The prefix is the bytes before the cursor (spec § 5.3).
    let prefix = state.buf[..state.cursor].to_vec();
    let start = match state.hist_index {
        Some(i) => Some(i),
        None => match direction {
            Direction::Backward => None,
            Direction::Forward => Some(0),
        },
    };
    let start_next = match direction {
        Direction::Backward => start,
        Direction::Forward => start.map(|i| i + 1),
    };
    if let Some(idx) = find_prefix(hist, &prefix, start_next, direction) {
        if state.hist_index.is_none() {
            state.edit_line = state.buf.clone();
        }
        state.hist_index = Some(idx);
        state.buf = hist[idx].to_vec();
        // Leave cursor at the original prefix length so continued
        // search extends from the same anchor.
        state.cursor = prefix.len().min(state.buf.len());
        state.undo.clear();
    }
}

fn do_yank_last_arg(shell: &Shell, state: &mut EmacsState, out: &mut Outcome) {
    let hist = shell.history();
    if hist.is_empty() {
        out.bell = true;
        return;
    }
    let walk = state
        .yank_last_arg
        .take()
        .unwrap_or_else(YankArgState::default);
    let new_offset = if state.last_fn == Some(EmacsFn::YankLastArg) {
        walk.hist_offset + 1
    } else {
        1
    };
    if new_offset > hist.len() {
        out.bell = true;
        state.yank_last_arg = Some(walk);
        return;
    }
    let idx = hist.len() - new_offset;
    let last_arg = last_word_of(&hist[idx]);
    // Remove the previous yank if there was one.
    if state.last_fn == Some(EmacsFn::YankLastArg) && walk.last_insert_len > 0 {
        let start = walk.last_insert_at;
        let end = start + walk.last_insert_len;
        if end <= state.buf.len() {
            state.buf.drain(start..end);
            state.cursor = start;
        }
    }
    let at = state.cursor;
    state.buf.splice(at..at, last_arg.iter().copied());
    state.cursor = at + last_arg.len();
    state.undo.push(UndoEntry::Yanked {
        at,
        len: last_arg.len(),
    });
    state.yank_last_arg = Some(YankArgState {
        hist_offset: new_offset,
        last_insert_at: at,
        last_insert_len: last_arg.len(),
    });
}

fn last_word_of(line: &[u8]) -> Vec<u8> {
    let end = {
        let mut e = line.len();
        while e > 0 && matches!(line[e - 1], b' ' | b'\t' | b'\n' | b'\r') {
            e -= 1;
        }
        e
    };
    if end == 0 {
        return Vec::new();
    }
    let mut start = end;
    while start > 0 && !matches!(line[start - 1], b' ' | b'\t' | b'\n' | b'\r') {
        start -= 1;
    }
    line[start..end].to_vec()
}

/// Word-boundary character set per spec § 5.8: SPACE, TAB, NEWLINE,
/// `>`, `<`, `|`, `;`, `(`, `)`, `&`, backtick, double and single
/// quote. Everything else (including `$`, `~`, `/`, `.`, `-`,
/// alphanumerics) is part of the word.
fn is_completion_delim(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\n' | b'>' | b'<' | b'|' | b';' | b'(' | b')' | b'&' | b'`' | b'"' | b'\''
    )
}

/// True iff `word_start` begins a command (argv[0]) position in the
/// buffer. A command position is either the start of the buffer or
/// preceded (after skipping runs of SPACE / TAB) by one of the
/// command-starting bytes: `;`, `&`, `|`, `(`, `` ` ``, or NEWLINE.
/// This catches `; cmd`, `&& cmd`, `| cmd`, `(subshell`, `` `cmd` ``,
/// and command-substitution openers like `$(cmd`, where the `(` is
/// the nearest non-whitespace predecessor.
fn is_command_position(buf: &[u8], word_start: usize) -> bool {
    let mut i = word_start;
    while i > 0 {
        let b = buf[i - 1];
        if b == b' ' || b == b'\t' {
            i -= 1;
            continue;
        }
        return matches!(b, b';' | b'&' | b'|' | b'(' | b'`' | b'\n');
    }
    true
}

/// Find the start of the completion-word containing `cursor`, walking
/// backwards across non-delimiter bytes per spec § 5.8.
fn find_completion_word_start(buf: &[u8], cursor: usize) -> usize {
    let mut s = cursor;
    while s > 0 {
        let prev = prev_char_start(buf, s);
        if is_completion_delim(buf[prev]) {
            break;
        }
        s = prev;
    }
    s
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionKind {
    /// Filesystem completion. Directory matches get a trailing `/` on
    /// a unique match; plain-file matches are inserted as-is.
    Path,
    /// Shell variable (`$NAME`). Inserted as-is.
    Variable,
    /// Brace-wrapped shell variable (`${NAME`). A unique match closes
    /// the expansion with `}`; multi-match LCP replacement leaves the
    /// brace open so the user can keep typing.
    BraceVariable,
    /// First-word command completion (builtins, aliases, functions,
    /// PATH executables). Inserted as-is.
    Command,
}

/// One candidate for display / replacement. The `word` is what replaces
/// the whole partial word (the `[word_start..cursor]` slice of the
/// buffer), and `display` is the short label used by second-TAB
/// listings.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    word: Vec<u8>,
    display: Vec<u8>,
}

fn do_complete(shell: &mut Shell, state: &mut EmacsState, out: &mut Outcome) {
    // Before touching the completion cascade, classify the lexical
    // context to the left of the cursor. Inside a still-open single
    // quote, a `#` line comment, or right after a trailing backslash
    // the user's TAB should land as a literal TAB byte (matching
    // `self-insert` on a plain character), not as a completion
    // request. Double quotes and nested `$(...)` / `` `...` ``
    // substitutions continue through the normal cascade.
    match classify_completion_context(&state.buf, state.cursor) {
        CompletionContext::InsideSingleQuote
        | CompletionContext::InsideComment
        | CompletionContext::AfterBackslash => {
            state.insert_bytes_at_cursor(b"\t");
            return;
        }
        CompletionContext::Normal | CompletionContext::InsideDoubleQuote => {}
    }

    let word_start = find_completion_word_start(&state.buf, state.cursor);
    let prefix = state.buf[word_start..state.cursor].to_vec();

    let is_first_word = is_command_position(&state.buf, word_start);

    let (mut candidates, kind) = gather_candidates(shell, &prefix, is_first_word);
    if candidates.is_empty() {
        out.bell = true;
        return;
    }
    candidates.sort_by(|a, b| a.word.cmp(&b.word));
    candidates.dedup_by(|a, b| a.word == b.word);

    if candidates.len() == 1 {
        let mut full = candidates[0].word.clone();
        append_terminator(&mut full, &candidates[0], kind);
        replace_prefix_with(state, word_start, prefix.len(), &full);
        return;
    }

    let words: Vec<Vec<u8>> = candidates.iter().map(|c| c.word.clone()).collect();
    let lcp = longest_common_prefix(&words).unwrap_or_default();
    if lcp.len() > prefix.len() {
        replace_prefix_with(state, word_start, prefix.len(), &lcp);
        return;
    }

    if state.last_fn == Some(EmacsFn::Complete) {
        list_candidates(&candidates);
    } else {
        out.bell = true;
    }
}

/// Splice `replacement` into the buffer in place of the old partial
/// word `buf[word_start..word_start + old_prefix_len]`, move the cursor
/// to the end of the replacement, and push a single undo entry.
fn replace_prefix_with(
    state: &mut EmacsState,
    word_start: usize,
    old_prefix_len: usize,
    replacement: &[u8],
) {
    let old_end = word_start + old_prefix_len;
    let removed: Vec<u8> = state.buf[word_start..old_end].to_vec();
    state
        .buf
        .splice(word_start..old_end, replacement.iter().copied());
    state.cursor = word_start + replacement.len();
    if !removed.is_empty() {
        state.undo.push(UndoEntry::Killed {
            at: word_start,
            bytes: removed,
        });
    }
    state.undo.push(UndoEntry::Inserted {
        at: word_start,
        bytes: replacement.to_vec(),
    });
}

/// Append the per-candidate terminator (spec § 5.8): `/` for a
/// directory match in [`CompletionKind::Path`]; `}` to close a
/// brace-wrapped variable expansion; nothing for anything else. We
/// intentionally do not auto-insert a space after file / command
/// names — the user is free to continue typing flags / paths.
fn append_terminator(full: &mut Vec<u8>, cand: &Candidate, kind: CompletionKind) {
    match kind {
        CompletionKind::Path if is_dir_candidate(&cand.word) => full.push(b'/'),
        CompletionKind::BraceVariable => full.push(b'}'),
        _ => {}
    }
}

fn is_dir_candidate(word: &[u8]) -> bool {
    // Reject trivially-empty words and already-slash-terminated words.
    if word.is_empty() || word.ends_with(b"/") {
        return false;
    }
    sys::fs::is_directory(word)
}

/// Print a bash-style column-major grid of candidate display labels
/// on stdout. The outer dispatch loop's `redraw` runs immediately
/// after and redraws the prompt + buffer under the listing.
///
/// Layout matches readline's `rl_display_match_list` default:
///
/// - `max_width` = longest display width across all candidates.
/// - `gutter`    = 2 columns of spacing between columns.
/// - `cols`      = max(1, term_width / (max_width + gutter)).
/// - `rows`      = ceil(ncands / cols).
/// - Entries are placed column-major so each column is sorted top-
///   to-bottom: row `r`, column `c` shows `cands[c * rows + r]`.
fn list_candidates(cands: &[Candidate]) {
    if cands.is_empty() {
        return;
    }
    let term_cols = terminal_columns();
    let max_width = cands
        .iter()
        .map(|c| display_width(&c.display))
        .max()
        .unwrap_or(0);
    const GUTTER: usize = 2;
    // `GUTTER` is a non-zero constant, so `max_width.saturating_add(GUTTER)`
    // is always ≥ `GUTTER` — the division below can never face a zero
    // divisor. Asserting this keeps the invariant visible in debug
    // builds and trips loudly if someone lowers `GUTTER` to zero.
    let cell = max_width.saturating_add(GUTTER);
    debug_assert!(cell >= GUTTER && cell != 0);
    let cols = (term_cols / cell).max(1);
    let rows = cands.len().div_ceil(cols);

    let mut buf: Vec<u8> = Vec::with_capacity(cands.len() * (max_width + GUTTER) + 4);
    buf.extend_from_slice(b"\r\n");
    for r in 0..rows {
        for c in 0..cols {
            let idx = c * rows + r;
            if idx >= cands.len() {
                break;
            }
            let entry = &cands[idx];
            buf.extend_from_slice(&entry.display);
            let is_last_on_row = c + 1 == cols || (c + 1) * rows + r >= cands.len();
            if !is_last_on_row {
                let pad = cell - display_width(&entry.display);
                for _ in 0..pad {
                    buf.push(b' ');
                }
            }
        }
        buf.extend_from_slice(b"\r\n");
    }
    write_bytes(&buf);
}

/// Best-effort query for the connected terminal's column count.
/// Falls back to `$COLUMNS` (if it parses as a positive integer) and
/// finally to 80.
fn terminal_columns() -> usize {
    if let Some(cols) = sys::tty::terminal_columns_from_stdio() {
        return cols;
    }
    if let Some(val) = sys::env::env_var(b"COLUMNS")
        && let Ok(text) = std::str::from_utf8(&val)
        && let Ok(n) = text.parse::<usize>()
        && n > 0
    {
        return n;
    }
    80
}

fn gather_candidates(
    shell: &Shell,
    prefix: &[u8],
    is_first_word: bool,
) -> (Vec<Candidate>, CompletionKind) {
    if let Some(stripped) = prefix.strip_prefix(b"${") {
        let mut cands: Vec<Candidate> = Vec::new();
        for (name, _) in shell.env().iter() {
            if name.starts_with(stripped) {
                let mut word = b"${".to_vec();
                word.extend_from_slice(name);
                let display = name.to_vec();
                cands.push(Candidate { word, display });
            }
        }
        return (cands, CompletionKind::BraceVariable);
    }

    if let Some(stripped) = prefix.strip_prefix(b"$") {
        let mut cands: Vec<Candidate> = Vec::new();
        for (name, _) in shell.env().iter() {
            if name.starts_with(stripped) {
                let mut word = b"$".to_vec();
                word.extend_from_slice(name);
                let display = name.to_vec();
                cands.push(Candidate { word, display });
            }
        }
        return (cands, CompletionKind::Variable);
    }

    if prefix.starts_with(b"~") && !prefix[1..].contains(&b'/') {
        // `~` or `~user` with no slash yet: expand `~` alone to the
        // $HOME directory. Usernames other than the current one are
        // not resolved (no /etc/passwd probing).
        if prefix == b"~"
            && let Some(home) = shell.get_var(b"HOME")
        {
            let cand = home.to_vec();
            let display = cand.clone();
            return (
                vec![Candidate {
                    word: cand,
                    display,
                }],
                CompletionKind::Path,
            );
        }
        return (Vec::new(), CompletionKind::Path);
    }

    if prefix.starts_with(b"~/")
        && let Some(home) = shell.get_var(b"HOME")
    {
        let mut expanded = home.to_vec();
        expanded.extend_from_slice(&prefix[1..]);
        let cands = complete_path_candidates(&expanded);
        return (cands, CompletionKind::Path);
    }

    if is_first_word && !prefix.contains(&b'/') {
        let cands = command_candidates(shell, prefix);
        return (cands, CompletionKind::Command);
    }

    (complete_path_candidates(prefix), CompletionKind::Path)
}

fn command_candidates(shell: &Shell, prefix: &[u8]) -> Vec<Candidate> {
    let mut out_cands: Vec<Candidate> = Vec::new();
    let mut seen: ShellSet<Vec<u8>> = ShellSet::default();
    let push = |cands: &mut Vec<Candidate>, seen: &mut ShellSet<Vec<u8>>, name: Vec<u8>| {
        if !name.starts_with(prefix) {
            return;
        }
        if seen.insert(name.clone()) {
            let display = name.clone();
            cands.push(Candidate {
                word: name,
                display,
            });
        }
    };

    for name in crate::builtin::all_builtin_names() {
        push(&mut out_cands, &mut seen, name.to_vec());
    }
    for (name, _) in shell.aliases().iter() {
        push(&mut out_cands, &mut seen, name.to_vec());
    }
    for (name, _) in shell.functions().iter() {
        push(&mut out_cands, &mut seen, name.to_vec());
    }

    if let Some(path) = shell.get_var(b"PATH").map(|s| s.to_vec()) {
        for segment in path.split(|&b| b == b':') {
            let base: &[u8] = if segment.is_empty() { b"." } else { segment };
            let c_dir = match crate::bstr::to_cstring(base) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let entries = match sys::fs::read_dir_entries_cstr(c_dir.as_c_str()) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for e in entries {
                let name = e.as_bytes();
                if !name.starts_with(prefix) {
                    continue;
                }
                push(&mut out_cands, &mut seen, name.to_vec());
            }
        }
    }

    out_cands
}

/// File-system completion for the partial `prefix`. Candidates replace
/// the whole word; the display label is just the matched basename.
fn complete_path_candidates(prefix: &[u8]) -> Vec<Candidate> {
    let (dir, fname) = match prefix.iter().rposition(|&b| b == b'/') {
        Some(pos) => (&prefix[..=pos], &prefix[pos + 1..]),
        None => (&b"."[..], prefix),
    };
    let c_dir = match crate::bstr::to_cstring(dir) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let entries = match sys::fs::read_dir_entries_cstr(c_dir.as_c_str()) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut cands: Vec<Candidate> = Vec::new();
    for e in entries {
        let name = e.as_bytes();
        if !name.starts_with(fname) {
            continue;
        }
        let mut word = if dir == b"." {
            Vec::new()
        } else {
            dir.to_vec()
        };
        word.extend_from_slice(name);
        let display = name.to_vec();
        cands.push(Candidate { word, display });
    }
    cands
}

fn longest_common_prefix(items: &[Vec<u8>]) -> Option<Vec<u8>> {
    if items.is_empty() {
        return None;
    }
    let mut prefix = items[0].clone();
    for it in &items[1..] {
        let mut n = 0;
        while n < prefix.len() && n < it.len() && prefix[n] == it[n] {
            n += 1;
        }
        prefix.truncate(n);
    }
    if prefix.is_empty() {
        None
    } else {
        Some(prefix)
    }
}

fn editor_temp_path() -> Vec<u8> {
    let mut p = b"/tmp/meiksh-edit-".to_vec();
    let pid = sys::process::current_pid();
    p.extend_from_slice(pid.to_string().as_bytes());
    p
}

// --- tests ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use crate::trace_entries;

    #[test]
    fn self_insert_appends_bytes() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::SelfInsert, b'a');
            assert!(!out.accepted);
            assert!(!out.bell);
            assert_eq!(state.buf, b"a");
            assert_eq!(state.cursor, 1);
        });
    }

    #[test]
    fn beginning_and_end_of_line() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"hello".to_vec();
            state.cursor = 3;
            apply(&mut shell, &mut state, EmacsFn::BeginningOfLine, 0);
            assert_eq!(state.cursor, 0);
            apply(&mut shell, &mut state, EmacsFn::EndOfLine, 0);
            assert_eq!(state.cursor, 5);
        });
    }

    #[test]
    fn backward_delete_char_removes_previous() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 3;
            let out = apply(&mut shell, &mut state, EmacsFn::BackwardDeleteChar, 0x7f);
            assert!(!out.bell);
            assert_eq!(state.buf, b"ab");
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn delete_char_empty_signals_eof() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::DeleteChar, 0x04);
            assert!(out.eof);
        });
    }

    #[test]
    fn kill_line_removes_tail_and_populates_kill_buffer() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"hello world".to_vec();
            state.cursor = 5;
            apply(&mut shell, &mut state, EmacsFn::KillLine, 0x0b);
            assert_eq!(state.buf, b"hello");
            assert_eq!(state.kill.as_slice(), b" world");
            apply(&mut shell, &mut state, EmacsFn::Yank, 0x19);
            assert_eq!(state.buf, b"hello world");
        });
    }

    #[test]
    fn kill_word_and_yank() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"foo bar baz".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0x00);
            assert_eq!(state.buf, b"bar baz");
            apply(&mut shell, &mut state, EmacsFn::Yank, 0x19);
            assert_eq!(state.buf, b"foo bar baz");
        });
    }

    #[test]
    fn unix_word_rubout_is_whitespace_scoped() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"path/to/foo".to_vec();
            state.cursor = state.buf.len();
            apply(&mut shell, &mut state, EmacsFn::UnixWordRubout, 0x17);
            // whole word (no ws) is killed
            assert_eq!(state.buf, b"");
        });
    }

    #[test]
    fn backward_kill_word_uses_alnum_boundary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"path/to/foo".to_vec();
            state.cursor = state.buf.len();
            apply(&mut shell, &mut state, EmacsFn::BackwardKillWord, 0x7f);
            assert_eq!(state.buf, b"path/to/");
        });
    }

    #[test]
    fn transpose_chars_swaps_last_two_at_eol() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abcd".to_vec();
            state.cursor = 4;
            apply(&mut shell, &mut state, EmacsFn::TransposeChars, 0x14);
            assert_eq!(state.buf, b"abdc");
        });
    }

    #[test]
    fn upcase_word_uppercases_alnum_run() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"foo bar".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::UpcaseWord, 0);
            assert_eq!(state.buf, b"FOO bar");
        });
    }

    #[test]
    fn downcase_word_lowercases_alnum_run() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"FOO BAR".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::DowncaseWord, 0);
            assert_eq!(state.buf, b"foo BAR");
        });
    }

    #[test]
    fn capitalize_word_title_cases_first_letter() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"foo bar".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::CapitalizeWord, 0);
            assert_eq!(state.buf, b"Foo bar");
        });
    }

    #[test]
    fn undo_reverses_self_insert_run() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            apply(&mut shell, &mut state, EmacsFn::SelfInsert, b'h');
            apply(&mut shell, &mut state, EmacsFn::SelfInsert, b'i');
            apply(&mut shell, &mut state, EmacsFn::Undo, 0x1f);
            assert_eq!(state.buf, b"");
        });
    }

    #[test]
    fn accept_line_sets_outcome_and_clears_undo() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"echo".to_vec();
            apply(&mut shell, &mut state, EmacsFn::SelfInsert, b'x');
            let out = apply(&mut shell, &mut state, EmacsFn::AcceptLine, 0x0d);
            assert!(out.accepted);
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn abort_without_composite_rings_bell() {
        // Per spec § 5.9, C-g (`abort`) outside of a composite action
        // (incremental search, `quoted-insert`) rings the bell and
        // leaves the buffer untouched.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 3;
            let out = apply(&mut shell, &mut state, EmacsFn::Abort, 0x07);
            assert!(out.bell);
            assert_eq!(state.buf, b"abc");
            assert_eq!(state.cursor, 3);
        });
    }

    #[test]
    fn previous_and_next_history_walk() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"one".to_vec().into_boxed_slice(),
                b"two".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"draft".to_vec();
            state.cursor = 5;
            apply(&mut shell, &mut state, EmacsFn::PreviousHistory, 0x10);
            assert_eq!(state.buf, b"two");
            apply(&mut shell, &mut state, EmacsFn::PreviousHistory, 0x10);
            assert_eq!(state.buf, b"one");
            apply(&mut shell, &mut state, EmacsFn::NextHistory, 0x0e);
            assert_eq!(state.buf, b"two");
            apply(&mut shell, &mut state, EmacsFn::NextHistory, 0x0e);
            assert_eq!(state.buf, b"draft");
        });
    }

    #[test]
    fn yank_last_arg_picks_last_whitespace_word() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![b"echo hello world".to_vec().into_boxed_slice()];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"cmd ".to_vec();
            state.cursor = 4;
            apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert_eq!(state.buf, b"cmd world");
        });
    }

    #[test]
    fn kill_buffer_appends_on_consecutive_kill_functions() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"foo bar baz".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0);
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0);
            assert_eq!(state.kill.as_slice(), b"foo bar ");
        });
    }

    #[test]
    fn non_kill_function_resets_kill_append() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"foo".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0);
            apply(&mut shell, &mut state, EmacsFn::ForwardChar, 0);
            state.buf = b"bar".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0);
            assert_eq!(state.kill.as_slice(), b"bar");
        });
    }

    #[test]
    fn quoted_insert_sets_outcome_flag() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::QuotedInsert, 0x11);
            assert!(out.quoted_insert);
        });
    }

    // --- empty-buffer bell / no-op early returns ---------------------

    #[test]
    fn kill_line_at_eol_is_noop_without_undo_entry() {
        // `kill-line` at end-of-buffer drains zero bytes. The helper
        // returns before pushing to the undo stack or kill buffer, which
        // is the only branch where `state.kill` stays untouched.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = state.buf.len();
            apply(&mut shell, &mut state, EmacsFn::KillLine, 0x0b);
            assert_eq!(state.buf, b"abc");
            assert!(state.kill.is_empty());
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn unix_line_discard_at_sol_is_noop() {
        // `unix-line-discard` from the start of the buffer has nothing
        // to kill; the function must return before touching the undo /
        // kill state.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::UnixLineDiscard, 0x15);
            assert_eq!(state.buf, b"abc");
            assert!(state.kill.is_empty());
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn unix_word_rubout_in_whitespace_is_noop() {
        // Cursor sitting on whitespace with only whitespace to the left
        // produces an empty kill range; the early return leaves undo /
        // kill untouched.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"   hello".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::UnixWordRubout, 0x17);
            assert_eq!(state.buf, b"   hello");
            assert!(state.kill.is_empty());
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn kill_word_at_eol_is_noop() {
        // `kill-word` at end-of-buffer has nothing to consume and must
        // exit before touching the undo / kill buffer.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = state.buf.len();
            apply(&mut shell, &mut state, EmacsFn::KillWord, 0);
            assert_eq!(state.buf, b"abc");
            assert!(state.kill.is_empty());
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn backward_kill_word_at_sol_is_noop() {
        // Symmetric to `kill-word`: `backward-kill-word` with cursor at
        // byte 0 leaves state untouched.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 0;
            apply(&mut shell, &mut state, EmacsFn::BackwardKillWord, 0x7f);
            assert_eq!(state.buf, b"abc");
            assert!(state.kill.is_empty());
            assert_eq!(state.undo.len(), 0);
        });
    }

    #[test]
    fn yank_with_empty_kill_buffer_rings_bell() {
        // `yank` with nothing in the kill buffer must ring the bell
        // and make no buffer / cursor changes (spec § 5.7).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 2;
            let out = apply(&mut shell, &mut state, EmacsFn::Yank, 0x19);
            assert!(out.bell);
            assert_eq!(state.buf, b"abc");
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn forward_char_at_eol_rings_bell() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = state.buf.len();
            let out = apply(&mut shell, &mut state, EmacsFn::ForwardChar, 0x06);
            assert!(out.bell);
            assert_eq!(state.cursor, 3);
        });
    }

    #[test]
    fn backward_char_at_sol_rings_bell() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 0;
            let out = apply(&mut shell, &mut state, EmacsFn::BackwardChar, 0x02);
            assert!(out.bell);
            assert_eq!(state.cursor, 0);
        });
    }

    // --- transpose edge cases ----------------------------------------

    #[test]
    fn transpose_chars_with_fewer_than_two_chars_rings_bell() {
        // Spec § 5.5: `transpose-chars` requires at least two bytes in
        // the buffer; otherwise it must ring the bell and leave the
        // buffer alone.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"a".to_vec();
            state.cursor = 1;
            let out = apply(&mut shell, &mut state, EmacsFn::TransposeChars, 0x14);
            assert!(out.bell);
            assert_eq!(state.buf, b"a");
        });
    }

    #[test]
    fn transpose_chars_at_sol_in_middle_of_buffer_rings_bell() {
        // Cursor at position 0 with a non-trivial buffer: there's no
        // character to the left to swap, so the function must ring the
        // bell rather than corrupting the buffer.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 0;
            let out = apply(&mut shell, &mut state, EmacsFn::TransposeChars, 0x14);
            assert!(out.bell);
            assert_eq!(state.buf, b"abc");
        });
    }

    #[test]
    fn transpose_chars_in_middle_swaps_adjacent() {
        // `transpose-chars` with the cursor between two chars swaps
        // them and moves the cursor past the swap.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abcd".to_vec();
            state.cursor = 2;
            apply(&mut shell, &mut state, EmacsFn::TransposeChars, 0x14);
            assert_eq!(state.buf, b"acbd");
            assert_eq!(state.cursor, 3);
        });
    }

    #[test]
    fn transpose_words_with_cursor_in_word_swaps_surrounding_words() {
        // Spec § 5.6: cursor *inside* a word marks that word as the
        // right-hand operand; the previous word on the line becomes the
        // left-hand operand.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"one two three".to_vec();
            state.cursor = 6;
            apply(&mut shell, &mut state, EmacsFn::TransposeWords, 0x14);
            assert_eq!(state.buf, b"two one three");
        });
    }

    #[test]
    fn transpose_words_with_only_one_word_rings_bell() {
        // No left-hand word available → bell, no mutation.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"solo".to_vec();
            state.cursor = state.buf.len();
            let out = apply(&mut shell, &mut state, EmacsFn::TransposeWords, 0x14);
            assert!(out.bell);
            assert_eq!(state.buf, b"solo");
        });
    }

    #[test]
    fn transpose_words_with_cursor_on_leading_ws_walks_forward_then_bells() {
        // Cursor on leading whitespace has no word behind it, so the
        // `p == 0` fallback forward-walks to the first word on the
        // line.  Even with a forward match, there is no *left* word to
        // swap with, so the function must ring the bell.  This
        // exercises the forward-walk success path (`q < buf.len()`
        // after skipping whitespace and consuming a run of word bytes)
        // that otherwise would be unreachable.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"   hello".to_vec();
            state.cursor = 0;
            let out = apply(&mut shell, &mut state, EmacsFn::TransposeWords, 0x14);
            assert!(out.bell);
            assert_eq!(state.buf, b"   hello");
        });
    }

    #[test]
    fn transpose_words_with_cursor_past_ws_after_last_word_rings_bell() {
        // Cursor past the last word, only whitespace behind and no
        // further word ahead → no operands available, bell must fire.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"   ".to_vec();
            state.cursor = state.buf.len();
            let out = apply(&mut shell, &mut state, EmacsFn::TransposeWords, 0x14);
            assert!(out.bell);
            assert_eq!(state.buf, b"   ");
        });
    }

    // --- case-word no-op --------------------------------------------

    #[test]
    fn case_word_at_eol_is_noop() {
        // `upcase-word` / `downcase-word` / `capitalize-word` with no
        // word following the cursor must early-return without pushing
        // an undo entry.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = state.buf.len();
            apply(&mut shell, &mut state, EmacsFn::UpcaseWord, 0);
            assert_eq!(state.buf, b"abc");
            assert_eq!(state.undo.len(), 0);
        });
    }

    // --- history-empty bell cases ------------------------------------

    #[test]
    fn previous_history_with_empty_history_rings_bell() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.history_mut().clear();
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::PreviousHistory, 0x10);
            assert!(out.bell);
        });
    }

    #[test]
    fn next_history_overshoot_rings_bell() {
        // Walking past the current ("not in history") position must
        // ring the bell rather than silently clipping.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![b"one".to_vec().into_boxed_slice()];
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::NextHistory, 0x0e);
            assert!(out.bell);
        });
    }

    #[test]
    fn beginning_of_history_on_empty_history_is_noop() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.history_mut().clear();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"draft".to_vec();
            apply(&mut shell, &mut state, EmacsFn::BeginningOfHistory, 0);
            assert_eq!(state.buf, b"draft");
            assert!(state.hist_index.is_none());
        });
    }

    #[test]
    fn beginning_of_history_jumps_to_oldest() {
        // Spec § 5.3: beginning-of-history stores the current draft as
        // `edit_line`, jumps to index 0, and restores the draft on
        // end-of-history.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"oldest".to_vec().into_boxed_slice(),
                b"newer".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"draft".to_vec();
            state.cursor = 5;
            apply(&mut shell, &mut state, EmacsFn::BeginningOfHistory, 0);
            assert_eq!(state.buf, b"oldest");
            assert_eq!(state.hist_index, Some(0));
            apply(&mut shell, &mut state, EmacsFn::EndOfHistory, 0);
            assert_eq!(state.buf, b"draft");
            assert!(state.hist_index.is_none());
        });
    }

    // --- history-search-prefix ---------------------------------------

    #[test]
    fn history_search_backward_finds_prefix_match() {
        // `history-search-backward` walks backward from "end of history"
        // and lands on the nearest entry whose prefix matches the bytes
        // before the cursor. The cursor anchors at the original prefix
        // length so repeated searches extend from the same column.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"echo hello".to_vec().into_boxed_slice(),
                b"ls -la".to_vec().into_boxed_slice(),
                b"echo world".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"ec".to_vec();
            state.cursor = 2;
            apply(&mut shell, &mut state, EmacsFn::HistorySearchBackward, 0);
            assert_eq!(state.buf, b"echo world");
            assert_eq!(state.hist_index, Some(2));
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn history_search_backward_walks_to_next_match() {
        // Repeating `history-search-backward` from an already-matched
        // entry must walk further back, exercising the
        // `Some(i)`/Backward arm of the start_next computation.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"echo hello".to_vec().into_boxed_slice(),
                b"ls -la".to_vec().into_boxed_slice(),
                b"echo world".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"ec".to_vec();
            state.cursor = 2;
            state.hist_index = Some(2);
            apply(&mut shell, &mut state, EmacsFn::HistorySearchBackward, 0);
            assert_eq!(state.buf, b"echo hello");
            assert_eq!(state.hist_index, Some(0));
        });
    }

    #[test]
    fn history_search_forward_from_beginning_finds_next_match() {
        // Forward search starts from index 0+1 when not already
        // anchored (the `None`/Forward branch seeds `start` to `Some(0)`
        // and then advances to `Some(1)`).  So with a fresh cursor we
        // should find the *second* prefix match onward, not index 0.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"echo hello".to_vec().into_boxed_slice(),
                b"ls -la".to_vec().into_boxed_slice(),
                b"echo world".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"ec".to_vec();
            state.cursor = 2;
            apply(&mut shell, &mut state, EmacsFn::HistorySearchForward, 0);
            assert_eq!(state.buf, b"echo world");
            assert_eq!(state.hist_index, Some(2));
        });
    }

    #[test]
    fn history_search_empty_history_is_noop() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.history_mut().clear();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"ec".to_vec();
            state.cursor = 2;
            apply(&mut shell, &mut state, EmacsFn::HistorySearchBackward, 0);
            assert_eq!(state.buf, b"ec");
            assert!(state.hist_index.is_none());
        });
    }

    #[test]
    fn history_search_no_match_leaves_buffer_untouched() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![b"ls".to_vec().into_boxed_slice()];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"zz".to_vec();
            state.cursor = 2;
            apply(&mut shell, &mut state, EmacsFn::HistorySearchBackward, 0);
            assert_eq!(state.buf, b"zz");
            assert!(state.hist_index.is_none());
        });
    }

    // --- yank-last-arg edge cases ------------------------------------

    #[test]
    fn yank_last_arg_with_empty_history_rings_bell() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.history_mut().clear();
            let mut state = EmacsState::new(0x7f);
            let out = apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert!(out.bell);
        });
    }

    #[test]
    fn yank_last_arg_repeated_walks_older_history() {
        // The second consecutive `yank-last-arg` must undo the previous
        // insert, walk one entry further back, and splice in the newly
        // selected last argument.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![
                b"echo older_last".to_vec().into_boxed_slice(),
                b"echo newer_last".to_vec().into_boxed_slice(),
            ];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"cmd ".to_vec();
            state.cursor = 4;
            apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert_eq!(state.buf, b"cmd newer_last");
            apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert_eq!(state.buf, b"cmd older_last");
        });
    }

    #[test]
    fn yank_last_arg_exhausted_walk_rings_bell_and_preserves_state() {
        // Walking past the oldest history entry must ring the bell and
        // re-stash the current walk state so the caller can back off
        // without losing their cursor.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            *shell.history_mut() = vec![b"echo one".to_vec().into_boxed_slice()];
            let mut state = EmacsState::new(0x7f);
            state.buf = b"cmd ".to_vec();
            state.cursor = 4;
            apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert_eq!(state.buf, b"cmd one");
            let out = apply(&mut shell, &mut state, EmacsFn::YankLastArg, 0);
            assert!(out.bell);
            assert!(state.yank_last_arg.is_some());
        });
    }

    // --- last_word_of pure helper -----------------------------------

    #[test]
    fn last_word_of_all_whitespace_returns_empty() {
        assert_no_syscalls(|| {
            assert_eq!(last_word_of(b"   \t\n  "), Vec::<u8>::new());
            assert_eq!(last_word_of(b""), Vec::<u8>::new());
        });
    }

    #[test]
    fn last_word_of_with_trailing_whitespace_strips_it() {
        assert_no_syscalls(|| {
            assert_eq!(last_word_of(b"echo hello   "), b"hello".to_vec());
            assert_eq!(last_word_of(b"  sole"), b"sole".to_vec());
        });
    }

    // --- longest_common_prefix --------------------------------------

    #[test]
    fn longest_common_prefix_empty_input_is_none() {
        assert_no_syscalls(|| {
            assert!(longest_common_prefix(&[]).is_none());
        });
    }

    #[test]
    fn longest_common_prefix_no_overlap_is_none() {
        assert_no_syscalls(|| {
            let items = vec![b"alpha".to_vec(), b"bravo".to_vec()];
            assert!(longest_common_prefix(&items).is_none());
        });
    }

    #[test]
    fn longest_common_prefix_partial_overlap_is_returned() {
        assert_no_syscalls(|| {
            let items = vec![b"echo".to_vec(), b"echelon".to_vec(), b"eck".to_vec()];
            assert_eq!(longest_common_prefix(&items), Some(b"ec".to_vec()));
        });
    }

    // --- is_dir_candidate trivial rejections -----------------------

    #[test]
    fn is_dir_candidate_rejects_empty_and_slash_suffixed() {
        assert_no_syscalls(|| {
            assert!(!is_dir_candidate(b""));
            assert!(!is_dir_candidate(b"foo/"));
        });
    }

    // --- gather_candidates tilde paths -----------------------------

    #[test]
    fn gather_candidates_bare_tilde_without_home_returns_empty_path_set() {
        // With `HOME` unset, `~` cannot expand; the function must
        // return an empty candidate list (still tagged as `Path` so
        // the caller's terminator logic remains consistent).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_mut().remove(b"HOME");
            let (cands, kind) = gather_candidates(&shell, b"~", false);
            assert!(cands.is_empty());
            assert_eq!(kind, CompletionKind::Path);
        });
    }

    #[test]
    fn gather_candidates_bare_tilde_with_home_returns_single_candidate() {
        // With `HOME` set, a single-candidate completion of `~` yields
        // exactly the `$HOME` directory, tagged as `Path` so the outer
        // caller can append a `/` if it turns out to be a directory.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.set_var(b"HOME", b"/tmp/home");
            let (cands, kind) = gather_candidates(&shell, b"~", false);
            assert_eq!(cands.len(), 1);
            assert_eq!(cands[0].word, b"/tmp/home".to_vec());
            assert_eq!(kind, CompletionKind::Path);
        });
    }

    #[test]
    fn gather_candidates_tilde_user_form_without_slash_returns_empty() {
        // `~root` (no slash) is not resolved by meiksh — only `~` alone
        // is special-cased. The function short-circuits without probing
        // `/etc/passwd`, returning an empty path candidate list.
        assert_no_syscalls(|| {
            let shell = test_shell();
            let (cands, kind) = gather_candidates(&shell, b"~root", false);
            assert!(cands.is_empty());
            assert_eq!(kind, CompletionKind::Path);
        });
    }

    #[test]
    fn gather_candidates_dollar_variable_matches_exported_name() {
        // `$PA…` expansion walks the exported environment; the stripped
        // prefix is matched verbatim against the variable name.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.set_var(b"PATH_TEST_VAR", b"1");
            let (cands, kind) = gather_candidates(&shell, b"$PATH_TEST_", false);
            assert!(!cands.is_empty());
            assert!(
                cands.iter().any(|c| c.word == b"$PATH_TEST_VAR".to_vec()),
                "expected $PATH_TEST_VAR: {:?}",
                cands
            );
            assert_eq!(kind, CompletionKind::Variable);
        });
    }

    #[test]
    fn apply_reverse_search_is_noop_when_routed_through_apply() {
        // The outer dispatch loop intercepts `ReverseSearchHistory` /
        // `ForwardSearchHistory` before they reach `apply`. When tests
        // (or a pathological binding) route them into `apply`, the
        // body is an explicit no-op: no bell, no buffer mutation, and
        // `last_fn` is still recorded so `yank-last-arg` walk detection
        // keeps working.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"preserved".to_vec();
            state.cursor = 3;
            let out1 = apply(&mut shell, &mut state, EmacsFn::ReverseSearchHistory, 0);
            assert!(!out1.bell);
            assert!(!out1.accepted);
            assert_eq!(state.buf, b"preserved");
            assert_eq!(state.cursor, 3);
            assert_eq!(state.last_fn, Some(EmacsFn::ReverseSearchHistory));
            let out2 = apply(&mut shell, &mut state, EmacsFn::ForwardSearchHistory, 0);
            assert!(!out2.bell);
            assert_eq!(state.last_fn, Some(EmacsFn::ForwardSearchHistory));
        });
    }

    #[test]
    fn apply_edit_and_execute_records_tmp_path() {
        // `EditAndExecuteCommand` opens `/tmp/meiksh-edit-<pid>`, writes
        // the current buffer followed by `\n`, closes the fd, and
        // surfaces the path back to the outer loop via
        // `out.edit_and_execute`. We trace every syscall so the test
        // stays deterministic under `run_trace`.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                getpid() -> pid(7777),
                open(str(b"/tmp/meiksh-edit-7777"), int(crate::sys::constants::O_WRONLY | crate::sys::constants::O_CREAT | crate::sys::constants::O_TRUNC), int(0o600)) -> int(9),
                write(fd(9), bytes(b"hello")) -> auto,
                write(fd(9), bytes(b"\n")) -> auto,
                close(fd(9)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let mut state = EmacsState::new(0x7f);
                state.buf = b"hello".to_vec();
                let out = apply(&mut shell, &mut state, EmacsFn::EditAndExecuteCommand, 0);
                assert_eq!(
                    out.edit_and_execute.as_deref(),
                    Some(&b"/tmp/meiksh-edit-7777"[..])
                );
            },
        );
    }

    #[test]
    fn apply_edit_and_execute_handles_open_failure_gracefully() {
        // When `open` fails (e.g. `/tmp` not writable), the function
        // still surfaces the intended path through
        // `out.edit_and_execute` so the outer loop can attempt the
        // external editor. The fd-level writes / close are skipped.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                getpid() -> pid(4321),
                open(str(b"/tmp/meiksh-edit-4321"), int(crate::sys::constants::O_WRONLY | crate::sys::constants::O_CREAT | crate::sys::constants::O_TRUNC), int(0o600))
                    -> err(crate::sys::constants::EACCES),
            ],
            || {
                let mut shell = test_shell();
                let mut state = EmacsState::new(0x7f);
                let out = apply(&mut shell, &mut state, EmacsFn::EditAndExecuteCommand, 0);
                assert_eq!(
                    out.edit_and_execute.as_deref(),
                    Some(&b"/tmp/meiksh-edit-4321"[..])
                );
            },
        );
    }

    #[test]
    fn run_bind_x_updates_buffer_and_point_from_environment() {
        // The `bind -x` handshake: publish READLINE_LINE/_POINT into
        // the environment, run the command (which may mutate them),
        // then pull the updated values back into the editor state.
        // We drive it with a no-op command (`:`) and pre-set the env
        // variables to simulate what the external command would do.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"original".to_vec();
            state.cursor = 8;
            // Pre-populate the "restored" values so `run_bind_x` reads
            // the expected post-command values back. `execute_string`
            // on `:` is an inexpensive no-op that exits via the
            // `true` builtin.
            let _ = shell.set_var(b"READLINE_LINE", b"will be overwritten");
            let _ = shell.set_var(b"READLINE_POINT", b"0");
            run_bind_x(&mut shell, &mut state, b":");
            // After the command runs, the buffer matches whatever
            // READLINE_LINE contained at the end — because `:` didn't
            // touch it, we should see the buffer pre-published from
            // `state.buf`.
            assert_eq!(state.buf, b"original".to_vec());
            // READLINE_LINE/_POINT were previously set, so the restore
            // branch takes the `Some(v)` path: they are written back
            // to the saved value rather than being removed.
            assert_eq!(
                shell.get_var(b"READLINE_LINE").map(|b| b.to_vec()),
                Some(b"will be overwritten".to_vec())
            );
        });
    }

    #[test]
    fn run_bind_x_removes_env_vars_when_previously_unset() {
        // When READLINE_LINE / _POINT were unset before the call, the
        // restore path must remove them (the `None` arms of the two
        // `match` blocks). We verify this by observing that
        // `shell.env().contains(...)` is false afterwards.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_mut().remove(b"READLINE_LINE");
            shell.env_mut().remove(b"READLINE_POINT");
            let mut state = EmacsState::new(0x7f);
            state.buf = b"x".to_vec();
            run_bind_x(&mut shell, &mut state, b":");
            assert!(shell.get_var(b"READLINE_LINE").is_none());
            assert!(shell.get_var(b"READLINE_POINT").is_none());
        });
    }

    #[test]
    fn run_bind_x_ignores_invalid_readline_point() {
        // A non-numeric `READLINE_POINT` value must be silently
        // ignored, taking both inner-`if let Ok` short-circuit arms
        // at the `READLINE_POINT` parsing in `run_bind_x` so the
        // cursor remains at its previous position.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 2;
            run_bind_x(
                &mut shell,
                &mut state,
                b"READLINE_LINE=abc; READLINE_POINT=not-a-number",
            );
            assert_eq!(state.buf, b"abc");
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn run_bind_x_parses_readline_point_on_return() {
        // If the command writes a numeric value into READLINE_POINT
        // and a new string into READLINE_LINE, `run_bind_x` clamps
        // the point to `buf.len()` and copies the new buffer. Here
        // we use a command line that sets both via `export`.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 1;
            // `execute_string` runs the command in the shell. This
            // command replaces READLINE_LINE with a shorter buffer
            // and sets READLINE_POINT to a value larger than the new
            // buffer — the function must clamp to `buf.len()`.
            run_bind_x(
                &mut shell,
                &mut state,
                b"READLINE_LINE=xy; READLINE_POINT=99",
            );
            assert_eq!(state.buf, b"xy");
            assert_eq!(state.cursor, 2);
        });
    }

    // --- list_candidates / completion internals ----------------------

    #[test]
    fn list_candidates_empty_returns_without_syscalls() {
        // An empty candidate slice short-circuits before any stdout
        // write. The function must perform no syscalls in that case.
        assert_no_syscalls(|| {
            list_candidates(&[]);
        });
    }

    // --- gather_candidates filesystem paths --------------------------

    #[test]
    fn gather_candidates_tilde_slash_expands_against_home() {
        // `~/foo` expansion rewrites the tilde with `$HOME` then
        // delegates to `complete_path_candidates`. We drive that
        // downstream call through the mocked-fs trace so the test
        // stays deterministic.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                opendir(str(b"/tmp/tc-home/")) -> int(1),
                readdir(_) -> dir_entry(b"."),
                readdir(_) -> dir_entry(b".."),
                readdir(_) -> dir_entry(b"foobar"),
                readdir(_) -> int(0),
                closedir(_) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"HOME", b"/tmp/tc-home");
                let (cands, kind) = gather_candidates(&shell, b"~/", false);
                assert_eq!(kind, CompletionKind::Path);
                assert!(
                    cands.iter().any(|c| c.display == b"foobar".to_vec()),
                    "expected foobar: {cands:?}"
                );
            },
        );
    }

    #[test]
    fn command_candidates_includes_aliases_and_functions() {
        // Register a shell alias and a shell function, then verify
        // both appear in the first-word completion set (covering
        // the aliases() and functions() iteration branches).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.aliases_mut().insert(
                b"zz_alias_probe".to_vec().into_boxed_slice(),
                b"ls -l".to_vec().into_boxed_slice(),
            );
            shell.define_function(
                b"zz_func_probe".to_vec(),
                std::rc::Rc::new(crate::syntax::ast::Command::Simple(Default::default())),
            );
            // Drop PATH entirely so `command_candidates` skips the
            // directory scan — keeps this test syscall-free.
            shell.env_mut().remove(b"PATH");
            let cands = command_candidates(&shell, b"zz_");
            assert!(
                cands.iter().any(|c| c.display == b"zz_alias_probe"),
                "missing alias: {cands:?}",
            );
            assert!(
                cands.iter().any(|c| c.display == b"zz_func_probe"),
                "missing function: {cands:?}",
            );
        });
    }

    #[test]
    fn complete_path_candidates_skips_dot_and_dotdot() {
        // The `.` / `..` entries returned by `readdir` are hidden
        // conventions; the completer must skip them even when the
        // prefix empty-matches them.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                opendir(str(b".")) -> int(1),
                readdir(_) -> dir_entry(b"."),
                readdir(_) -> dir_entry(b".."),
                readdir(_) -> dir_entry(b"afile"),
                readdir(_) -> int(0),
                closedir(_) -> 0,
            ],
            || {
                let cands = complete_path_candidates(b"");
                assert_eq!(cands.len(), 1);
                assert_eq!(cands[0].display, b"afile".to_vec());
            },
        );
    }

    #[test]
    fn complete_path_candidates_returns_empty_on_readdir_error() {
        // A failing `opendir` short-circuits the whole walk; the
        // function returns an empty Vec instead of propagating the
        // error.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                opendir(str(b".")) -> err(crate::sys::constants::ENOENT),
            ],
            || {
                let cands = complete_path_candidates(b"");
                assert!(cands.is_empty());
            },
        );
    }

    #[test]
    fn command_candidates_skips_path_segment_with_embedded_nul() {
        // A PATH segment containing a NUL byte cannot be turned into
        // a CString, so `command_candidates` must `continue` past
        // that segment (line 1006).
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.set_var(b"PATH", b"bad\0seg");
            let cands = command_candidates(&shell, b"zzz_no_match");
            assert!(cands.is_empty());
        });
    }

    #[test]
    fn command_candidates_skips_path_segment_with_failing_readdir() {
        // A PATH segment that points at a non-openable directory
        // exercises the `Err(_) => continue` arm at line 1010.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                opendir(str(b"/missing/path/segment")) -> err(crate::sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"PATH", b"/missing/path/segment");
                let cands = command_candidates(&shell, b"zzz_no_match");
                assert!(cands.is_empty());
            },
        );
    }

    #[test]
    fn complete_path_candidates_returns_empty_when_dir_contains_nul() {
        // A NUL byte in the directory portion of `prefix` makes
        // `to_cstring` fail; the function returns `Vec::new()`
        // (line 1034).
        assert_no_syscalls(|| {
            let cands = complete_path_candidates(b"bad\0dir/file");
            assert!(cands.is_empty());
        });
    }

    #[test]
    fn complete_path_candidates_skips_non_matching_entries() {
        // `readdir` returns an entry that does not start with `fname`
        // so the inner `continue` arm at line 1044 fires.
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                opendir(str(b".")) -> int(1),
                readdir(_) -> dir_entry(b"alpha"),
                readdir(_) -> dir_entry(b"beta"),
                readdir(_) -> int(0),
                closedir(_) -> 0,
            ],
            || {
                let cands = complete_path_candidates(b"al");
                assert_eq!(cands.len(), 1);
                assert_eq!(cands[0].display, b"alpha");
            },
        );
    }

    #[test]
    fn terminal_columns_falls_back_to_columns_env_var() {
        // With no controlling tty, `terminal_columns` reads the
        // `COLUMNS` env var through the sys boundary.
        crate::sys::test_support::set_test_terminal_columns(None);
        run_trace(
            trace_entries![
                ..vec![t(
                    "getenv",
                    vec![ArgMatcher::Str(b"COLUMNS".to_vec())],
                    TraceResult::StrVal(b"55".to_vec()),
                )]
            ],
            || assert_eq!(terminal_columns(), 55),
        );
    }

    #[test]
    fn terminal_columns_uses_tty_override_when_present() {
        // When the TTY ioctl reports a column count, that wins
        // before the COLUMNS fallback (line 894).
        crate::sys::test_support::set_test_terminal_columns(Some(132));
        let cols = terminal_columns();
        crate::sys::test_support::set_test_terminal_columns(None);
        assert_eq!(cols, 132);
    }

    #[test]
    fn gather_candidates_brace_variable_wraps_match() {
        // `${PA…` produces `BraceVariable` candidates whose `word`
        // already includes the leading `${`; the outer caller appends
        // the closing `}` for unique matches.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.set_var(b"BRACE_TEST_VAR", b"1");
            let (cands, kind) = gather_candidates(&shell, b"${BRACE_TEST_", false);
            assert!(
                cands.iter().any(|c| c.word == b"${BRACE_TEST_VAR".to_vec()),
                "expected ${{BRACE_TEST_VAR: {:?}",
                cands
            );
            assert_eq!(kind, CompletionKind::BraceVariable);
        });
    }
}
