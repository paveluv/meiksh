//! Emacs-style interactive line editor (spec
//! `docs/features/emacs-editing-mode.md`).
//!
//! This is the top-level entry point. The heavy lifting lives in
//! four submodules:
//!
//! * [`keymap`] — the default and mutable binding trie.
//! * [`functions`] — the bindable function implementations.
//! * [`kill_buffer`] / [`undo`] / [`state`] — per-call mutable data.
//! * [`search`] — the `C-r` / `C-s` mini-buffer state machine.
//!
//! Shared terminal plumbing (raw mode, byte I/O, redraw, word
//! boundaries, bracketed-paste framing) lives in
//! [`super::editor`].

#![allow(dead_code)]
pub(crate) mod completion_context;
pub(crate) mod functions;
pub(crate) mod keymap;
pub(crate) mod kill_buffer;
pub(crate) mod search;
pub(crate) mod state;
pub(crate) mod undo;

use crate::shell::state::Shell;
use crate::sys;

use self::keymap::{EmacsFn, Keymap, Resolved};
use self::search::{IncrementalSearch, SearchOutcome};
use self::state::EmacsState;
use super::editor::bracketed_paste::{
    FrameDetector, FrameEvent, enter_paste_mode, leave_paste_mode,
};
use super::editor::history_search::Direction;
use super::editor::input::{bell, read_byte, write_bytes};
use super::editor::raw_mode::RawMode;
use super::editor::redraw::redraw;

/// Outer entry point: acquire raw mode (falling back to canonical
/// reads if the terminal isn't available), run the dispatch loop,
/// return the accepted line or `None` on EOF.
pub(super) fn read_line(
    shell: &mut Shell,
    prompt: &[u8],
) -> sys::error::SysResult<Option<Vec<u8>>> {
    let raw = match RawMode::enter() {
        Ok(r) => r,
        Err(_) => return super::prompt::read_line(),
    };

    let erase_char = sys::tty::get_terminal_attrs(sys::constants::STDIN_FILENO)
        .map(|a| a.c_cc[sys::constants::VERASE])
        .unwrap_or(0x7f);

    super::inputrc::ensure_startup_loaded(shell);
    let keymap = {
        let guard = super::inputrc::global().lock();
        match guard {
            Ok(g) => g.keymap.clone(),
            Err(p) => p.into_inner().keymap.clone(),
        }
    };
    let mut state = EmacsState::new(erase_char);
    let mut paste = FrameDetector::new();

    enter_paste_mode();
    redraw(&state.buf, state.cursor, prompt);

    let result = dispatch_loop(shell, prompt, &keymap, &mut state, &mut paste, &raw);

    leave_paste_mode();
    write_bytes(b"\r\n");
    result
}

fn dispatch_loop(
    shell: &mut Shell,
    prompt: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    paste: &mut FrameDetector,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Vec<u8>>> {
    let mut pending: Vec<u8> = Vec::new();
    let mut in_paste = false;

    loop {
        let byte = match read_byte()? {
            Some(b) => b,
            None => {
                // EOF: if buffer is empty return None so the REPL
                // terminates; otherwise discard the partial line and
                // also return None (matches bash behavior).
                return Ok(None);
            }
        };

        let event = paste.feed(byte);
        match event {
            FrameEvent::Pending => continue,
            FrameEvent::Start => {
                in_paste = true;
                state.begin_paste_group();
                continue;
            }
            FrameEvent::End => {
                in_paste = false;
                state.end_paste_group();
                redraw(&state.buf, state.cursor, prompt);
                continue;
            }
            FrameEvent::EmitLiteral(bytes) => {
                if in_paste {
                    for b in bytes {
                        state.insert_paste_byte(b);
                    }
                    continue;
                }
                for b in bytes {
                    if let Some(res) =
                        handle_byte(shell, prompt, keymap, state, &mut pending, b, raw)?
                    {
                        return Ok(res);
                    }
                }
                redraw(&state.buf, state.cursor, prompt);
            }
        }
    }
}

/// Feed a single user byte into dispatch. Returns `Ok(Some(_))` when
/// the loop should return that value, `Ok(None)` to keep reading.
fn handle_byte(
    shell: &mut Shell,
    prompt: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    pending: &mut Vec<u8>,
    b: u8,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Option<Vec<u8>>>> {
    pending.push(b);
    match keymap.resolve(pending) {
        Resolved::NeedsMore => Ok(None),
        Resolved::Unbound => {
            // If the pending buffer is a single printable byte (spec
            // § 5.2: self-insert is the fallback for unbound printable
            // keys), insert it. Otherwise ring the bell.
            if pending.len() == 1 && is_printable(pending[0]) {
                let byte = pending[0];
                pending.clear();
                dispatch_function(shell, prompt, keymap, state, EmacsFn::SelfInsert, byte, raw)
            } else {
                pending.clear();
                bell();
                Ok(None)
            }
        }
        Resolved::Function(EmacsFn::ReverseSearchHistory) => {
            pending.clear();
            run_incremental_search(shell, prompt, keymap, state, Direction::Backward, raw)
        }
        Resolved::Function(EmacsFn::ForwardSearchHistory) => {
            pending.clear();
            run_incremental_search(shell, prompt, keymap, state, Direction::Forward, raw)
        }
        Resolved::Function(f) => {
            let trigger = *pending.last().unwrap_or(&0);
            pending.clear();
            dispatch_function(shell, prompt, keymap, state, f, trigger, raw)
        }
        Resolved::Macro(bytes) => {
            pending.clear();
            // Feed macro bytes back through dispatch as if typed.
            for mb in bytes {
                if let Some(res) = handle_byte(shell, prompt, keymap, state, pending, mb, raw)? {
                    return Ok(Some(res));
                }
            }
            Ok(None)
        }
        Resolved::ExecShell(cmd) => {
            pending.clear();
            functions::run_bind_x(shell, state, &cmd);
            Ok(None)
        }
    }
}

fn dispatch_function(
    shell: &mut Shell,
    prompt: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    f: EmacsFn,
    trigger: u8,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Option<Vec<u8>>>> {
    let outcome = functions::apply(shell, state, f, trigger);
    if outcome.bell {
        bell();
    }
    if outcome.eof {
        return Ok(Some(None));
    }
    if outcome.accepted {
        let mut line = std::mem::take(&mut state.buf);
        line.push(b'\n');
        return Ok(Some(Some(line)));
    }
    if outcome.quoted_insert {
        if let Some(b) = read_byte()? {
            state.insert_bytes_at_cursor(&[b]);
        }
    }
    if let Some(tmp_path) = outcome.edit_and_execute {
        let _ = keymap; // keymap borrowed just to keep the signature stable.
        return run_external_editor(shell, prompt, state, &tmp_path, raw);
    }
    Ok(None)
}

fn run_incremental_search(
    shell: &mut Shell,
    prompt: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    direction: Direction,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Option<Vec<u8>>>> {
    let saved_buf = state.buf.clone();
    let saved_cursor = state.cursor;
    let history: Vec<Box<[u8]>> = shell.history().clone();
    let mut search = IncrementalSearch::new(&history, direction);
    draw_search_prompt(&search);
    loop {
        let byte = match read_byte()? {
            Some(b) => b,
            None => {
                state.buf = saved_buf;
                state.cursor = saved_cursor;
                return Ok(Some(None));
            }
        };
        match search.feed(byte) {
            SearchOutcome::Continue => {
                if let Some(idx) = search.matched() {
                    state.buf = history[idx].to_vec();
                    state.cursor = state.buf.len();
                }
                draw_search_prompt(&search);
            }
            SearchOutcome::Accept => {
                if let Some(idx) = search.matched() {
                    state.buf = history[idx].to_vec();
                    state.cursor = state.buf.len();
                }
                state.undo.clear();
                // Redraw so the user's transcript shows the accepted
                // line sitting on the regular prompt (rather than in
                // the `(reverse-i-search...)` mini-buffer), then
                // terminate the read loop as if `accept-line` had
                // fired — per spec § 7.2.
                redraw(&state.buf, state.cursor, prompt);
                let mut line = std::mem::take(&mut state.buf);
                line.push(b'\n');
                return Ok(Some(Some(line)));
            }
            SearchOutcome::Abort => {
                state.buf = saved_buf;
                state.cursor = saved_cursor;
                redraw(&state.buf, state.cursor, prompt);
                return Ok(None);
            }
            SearchOutcome::Exit { byte: redispatch } => {
                if let Some(idx) = search.matched() {
                    state.buf = history[idx].to_vec();
                    state.cursor = state.buf.len();
                }
                state.undo.clear();
                redraw(&state.buf, state.cursor, prompt);
                let mut pending = Vec::new();
                if let Some(res) =
                    handle_byte(shell, prompt, keymap, state, &mut pending, redispatch, raw)?
                {
                    return Ok(Some(res));
                }
                redraw(&state.buf, state.cursor, prompt);
                return Ok(None);
            }
        }
    }
}

fn draw_search_prompt(search: &IncrementalSearch<'_>) {
    let failing = if search.failing() {
        b"failing "
    } else {
        &b""[..]
    };
    let tag = if matches!(search.direction(), Direction::Backward) {
        &b"reverse-i-search"[..]
    } else {
        &b"i-search"[..]
    };
    let mut buf: Vec<u8> = Vec::new();
    buf.extend_from_slice(b"\r\x1b[K(");
    buf.extend_from_slice(failing);
    buf.extend_from_slice(tag);
    buf.push(b'`');
    buf.extend_from_slice(search.pattern());
    buf.extend_from_slice(b"'): ");
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &buf);
}

fn run_external_editor(
    shell: &mut Shell,
    prompt: &[u8],
    state: &mut EmacsState,
    tmp_path: &[u8],
    raw: &RawMode,
) -> sys::error::SysResult<Option<Option<Vec<u8>>>> {
    let editor = shell
        .get_var(b"VISUAL")
        .or_else(|| shell.get_var(b"EDITOR"))
        .unwrap_or(b"vi")
        .to_vec();
    let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, raw.saved());
    write_bytes(b"\r\n");
    let mut edit_cmd = editor;
    edit_cmd.push(b' ');
    edit_cmd.extend_from_slice(tmp_path);
    let _ = shell.execute_string(&edit_cmd);
    let mut raw_restored = *raw.saved();
    raw_restored.c_lflag &= !(sys::constants::ICANON | sys::constants::ECHO | sys::constants::ISIG);
    raw_restored.c_cc[sys::constants::VMIN] = 1;
    raw_restored.c_cc[sys::constants::VTIME] = 0;
    let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &raw_restored);
    if let Ok(content) = sys::fs::read_file(tmp_path) {
        let mut end = content.len();
        while end > 0 && matches!(content[end - 1], b' ' | b'\t' | b'\n' | b'\r') {
            end -= 1;
        }
        let trimmed = &content[..end];
        if !trimmed.is_empty() {
            super::remove_file_bytes(tmp_path);
            let mut out = trimmed.to_vec();
            out.push(b'\n');
            return Ok(Some(Some(out)));
        }
    }
    super::remove_file_bytes(tmp_path);
    state.buf.clear();
    state.cursor = 0;
    redraw(&state.buf, state.cursor, prompt);
    Ok(None)
}

fn is_printable(b: u8) -> bool {
    b >= 0x20 && b != 0x7f
}
