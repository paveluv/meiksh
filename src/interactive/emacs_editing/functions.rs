//! Implementation of every emacs bindable function (spec § 5).
//!
//! The outer dispatch loop in [`super::read_line`] calls [`apply`] with
//! the selected [`EmacsFn`] and the last read byte, which is needed by
//! `self-insert` and `quoted-insert`. Each function is small and
//! testable in isolation; coverage for the I/O side is provided by the
//! PTY integration tests in `tests/integration/emacs_mode.rs`.

use crate::shell::state::Shell;
use crate::sys;

use super::super::editor::history_search::{Direction, find_prefix};
use super::super::editor::input::write_bytes;
use super::super::editor::redraw::{char_len_at, prev_char_start};
use super::super::editor::words::{WordClass, next_word_boundary, prev_word_boundary};
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
            let n = state.cursor + char_len_at(&state.buf, state.cursor);
            state.cursor = n.min(state.buf.len());
        }
        EmacsFn::BackwardChar => state.cursor = prev_char_start(&state.buf, state.cursor),
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
            // C-g clears the current line (per spec § 5.5); the outer
            // loop redraws afterwards.
            state.buf.clear();
            state.cursor = 0;
            state.undo.clear();
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
        EmacsFn::PreviousHistory => do_history_step(shell, state, -1),
        EmacsFn::NextHistory => do_history_step(shell, state, 1),
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
    // Find the two nearest words around the cursor and swap them.
    // Algorithm: walk to the end of the current word, remember the
    // left word, skip whitespace, walk to end of right word, remember
    // it; then splice.
    let buf = state.buf.clone();
    let left_end = match word_end_at(&buf, state.cursor) {
        Some(e) => e,
        None => {
            out.bell = true;
            return;
        }
    };
    let left_start = prev_word_boundary(&buf, left_end, WordClass::AlnumUnderscore);
    let right_start = next_word_boundary(&buf, left_end, WordClass::AlnumUnderscore);
    let right_end = next_word_boundary(&buf, right_start, WordClass::AlnumUnderscore);
    if left_start == left_end
        || right_start == right_end
        || right_start < left_end
        || left_end >= buf.len()
    {
        out.bell = true;
        return;
    }
    // Collapse the trailing-whitespace included in next_word_boundary:
    // the "right word" should end at the first non-word char after it.
    let right_end = trim_trailing_ws(&buf, right_end);

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

/// Start of the word at or before `pos`, or `None` if no word there.
fn word_end_at(buf: &[u8], pos: usize) -> Option<usize> {
    if buf.is_empty() {
        return None;
    }
    let mut p = pos;
    if p > buf.len() {
        p = buf.len();
    }
    // walk forward to end of current word
    while p < buf.len() && super::super::editor::words::is_word_char_at(buf, p) {
        p += char_len_at(buf, p);
    }
    if p == 0 {
        return None;
    }
    Some(p)
}

fn trim_trailing_ws(buf: &[u8], mut end: usize) -> usize {
    while end > 0 && (buf[end - 1] == b' ' || buf[end - 1] == b'\t') {
        end -= 1;
    }
    end
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

fn do_history_step(shell: &Shell, state: &mut EmacsState, delta: i32) {
    let hist = shell.history();
    let len = hist.len();
    if len == 0 {
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

fn do_complete(shell: &mut Shell, state: &mut EmacsState, _out: &mut Outcome) {
    // Minimal completion: expand the current word as a file path
    // prefix. The full spec 5.7-5.8 behaviour (variables, tilde,
    // aliases, builtins, PATH) is implemented as a cascade of checks
    // on the literal prefix.
    let word_start = {
        let mut s = state.cursor;
        while s > 0 {
            let prev = prev_char_start(&state.buf, s);
            let b = state.buf[prev];
            if b == b' ' || b == b'\t' {
                break;
            }
            s = prev;
        }
        s
    };
    let prefix = state.buf[word_start..state.cursor].to_vec();
    if let Some(completion) = try_complete_cascade(shell, &prefix) {
        if completion.len() > prefix.len() {
            let extra: Vec<u8> = completion[prefix.len()..].to_vec();
            let at = state.cursor;
            state.buf.splice(at..at, extra.iter().copied());
            state.cursor = at + extra.len();
            state.undo.push(UndoEntry::Inserted { at, bytes: extra });
        }
    }
}

fn try_complete_cascade(shell: &Shell, prefix: &[u8]) -> Option<Vec<u8>> {
    if let Some(stripped) = prefix.strip_prefix(b"$") {
        let mut matches: Vec<Vec<u8>> = Vec::new();
        for (name, _) in shell.env().iter() {
            if name.starts_with(stripped) {
                matches.push(name.to_vec());
            }
        }
        if let Some(common) = longest_common_prefix(&matches) {
            let mut out = b"$".to_vec();
            out.extend_from_slice(&common);
            return Some(out);
        }
        return None;
    }
    if prefix.starts_with(b"~") {
        return None;
    }
    complete_path(prefix)
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

fn complete_path(prefix: &[u8]) -> Option<Vec<u8>> {
    let (dir, fname) = match prefix.iter().rposition(|&b| b == b'/') {
        Some(pos) => (&prefix[..=pos], &prefix[pos + 1..]),
        None => (&b"."[..], prefix),
    };
    let c_dir = crate::bstr::to_cstring(dir).ok()?;
    let entries = sys::fs::read_dir_entries_cstr(c_dir.as_c_str()).ok()?;
    let mut matches: Vec<Vec<u8>> = Vec::new();
    for e in entries {
        let bytes = e.as_bytes();
        if bytes.starts_with(fname) {
            matches.push(bytes.to_vec());
        }
    }
    let common = longest_common_prefix(&matches)?;
    let mut out = if dir == b"." {
        Vec::new()
    } else {
        dir.to_vec()
    };
    out.extend_from_slice(&common);
    Some(out)
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
    use crate::sys::test_support::assert_no_syscalls;

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
    fn abort_clears_line() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut state = EmacsState::new(0x7f);
            state.buf = b"abc".to_vec();
            state.cursor = 3;
            apply(&mut shell, &mut state, EmacsFn::Abort, 0x07);
            assert_eq!(state.buf, b"");
            assert_eq!(state.cursor, 0);
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
}
