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
pub(crate) mod completion_quoting;
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
use super::editor::input::{bell, read_byte_with_signal_handler, write_bytes};
use super::editor::raw_mode::RawMode;
use super::editor::redraw::{display_width, redraw};

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
    // Expand PS2 once per editor entry so continuation rows
    // introduced by an embedded `\n` (paste, `insert-newline`, or
    // the parser-driven auto-continuation that `accept-line`
    // performs when the buffer is syntactically incomplete) render
    // with the same gutter the cross-call PS2 reprompt in
    // `repl.rs` would use. Re-expanding mid-edit would chase a
    // moving target (PS2 may interpolate `\#`, `\!`, etc. via
    // `prompt_expand`); a single expansion here mirrors how PS1 is
    // captured exactly once per call.
    let ps2 = super::prompt::expand_prompt(shell, b"PS2", b"> ");

    enter_paste_mode();
    // The prompt was already written to stderr by the REPL before
    // calling us; immediately repaint it through the wrap-aware
    // redraw path so the per-edit `draw_anchor` starts tracking
    // cursor row from a known position.
    redraw(
        &mut state.draw_anchor,
        &state.buf,
        state.cursor,
        prompt,
        &ps2,
    );

    let result = dispatch_loop(shell, prompt, &ps2, &keymap, &mut state, &mut paste, &raw);

    leave_paste_mode();
    write_bytes(b"\r\n");
    result
}

fn dispatch_loop(
    shell: &mut Shell,
    prompt: &[u8],
    ps2: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    paste: &mut FrameDetector,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Vec<u8>>> {
    let mut pending: Vec<u8> = Vec::new();
    let mut in_paste = false;

    loop {
        // The dispatch loop's blocking byte-level read is the *one*
        // place a `SIGCHLD` (job-status change) is allowed to
        // interrupt the editor. `read_byte_with_signal_handler`
        // drains pending notifications via
        // `crate::interactive::notify::stash_or_print_notifications`
        // (POSIX § 2.11): with `set -b` it writes immediately and
        // signals back via `printed_now` so we can re-emit the
        // current edit line; with `set +b` it queues for the next
        // prompt and the editor never sees any output. Other read
        // sites in this module (the inner `quoted-insert` byte and
        // the incremental-search loop) reuse the same helper so the
        // notification policy stays uniform.
        // The redraw closure runs *before* each blocking `read()`
        // whenever `set -b` just wrote a status line to stderr, so
        // the user (and matrix tests) see a fresh prompt + buffer
        // below the asynchronous notification — even if no further
        // keystroke ever arrives. With the default `set +b` the
        // notification is stashed for the next prompt and the
        // closure is not invoked.
        // The closure must reset `draw_anchor` before re-drawing
        // because the `\r\n` has just placed the cursor on a fresh
        // row below whatever the editor had on screen — moving
        // *up* from the new row would land in unrelated terminal
        // output above. After the redraw, the anchor accurately
        // tracks the new prompt's row again.
        let buf_ref = &state.buf;
        let cursor_now = state.cursor;
        let anchor = &mut state.draw_anchor;
        let (maybe_byte, _intr) = read_byte_with_signal_handler(shell, || {
            write_bytes(b"\r\n");
            anchor.reset();
            redraw(anchor, buf_ref, cursor_now, prompt, ps2);
        })?;
        let byte = match maybe_byte {
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
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
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
                        handle_byte(shell, prompt, ps2, keymap, state, &mut pending, b, raw)?
                    {
                        return Ok(res);
                    }
                }
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
            }
        }
    }
}

/// Feed a single user byte into dispatch. Returns `Ok(Some(_))` when
/// the loop should return that value, `Ok(None)` to keep reading.
#[allow(clippy::too_many_arguments)]
fn handle_byte(
    shell: &mut Shell,
    prompt: &[u8],
    ps2: &[u8],
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
                dispatch_function(
                    shell,
                    prompt,
                    ps2,
                    keymap,
                    state,
                    EmacsFn::SelfInsert,
                    byte,
                    raw,
                )
            } else {
                pending.clear();
                bell();
                Ok(None)
            }
        }
        Resolved::Function(EmacsFn::ReverseSearchHistory) => {
            pending.clear();
            run_incremental_search(shell, prompt, ps2, keymap, state, Direction::Backward, raw)
        }
        Resolved::Function(EmacsFn::ForwardSearchHistory) => {
            pending.clear();
            run_incremental_search(shell, prompt, ps2, keymap, state, Direction::Forward, raw)
        }
        Resolved::Function(f) => {
            let trigger = *pending.last().unwrap_or(&0);
            pending.clear();
            dispatch_function(shell, prompt, ps2, keymap, state, f, trigger, raw)
        }
        Resolved::Macro(bytes) => {
            pending.clear();
            // Feed macro bytes back through dispatch as if typed.
            for mb in bytes {
                if let Some(res) = handle_byte(shell, prompt, ps2, keymap, state, pending, mb, raw)?
                {
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

#[allow(clippy::too_many_arguments)]
fn dispatch_function(
    shell: &mut Shell,
    prompt: &[u8],
    ps2: &[u8],
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
        // Park the terminal cursor on the *visual end* of the
        // buffer before the caller emits `\r\n` and hands the line
        // to the executor. Without this, a multi-line buffer that
        // the user accepted from the middle of (cursor on row 1 of
        // 3, say) would let the executor's output overwrite rows
        // 1..N of the freshly-rendered command. We use the
        // snapshot's `(cursor_row, end_row)` to emit a single
        // `\x1b[<N>B` to step down to the last row of the buffer;
        // `read_line`'s post-loop `\r\n` then lands on a clean
        // fresh row below the entire command.
        //
        // We rely on the snapshot because the editor never queries
        // the terminal for absolute row positions — only relative
        // motion is safe. When no snapshot exists (cols unknown /
        // not a tty, first draw, just-reset anchor), there is no
        // multi-line render to step past in the first place, so
        // skipping the move is correct.
        let down = state.draw_anchor.rows_below_cursor();
        if down > 0 {
            let mut buf = Vec::with_capacity(8);
            buf.extend_from_slice(b"\x1b[");
            crate::bstr::push_u64(&mut buf, down as u64);
            buf.push(b'B');
            write_bytes(&buf);
        }
        let mut line = std::mem::take(&mut state.buf);
        line.push(b'\n');
        return Ok(Some(Some(line)));
    }
    if outcome.quoted_insert {
        // Quoted-insert (`C-q` / `C-v`): the user wants the *next*
        // raw byte inserted literally into the buffer. We must still
        // tolerate `EINTR` here — otherwise a `SIGCHLD` arriving
        // between `C-q` and the user's next keystroke would crash
        // the editor with "Interrupted system call". The helper
        // drains notifications transparently and retries the read,
        // so the byte the user finally types is still inserted
        // verbatim. The redraw closure here is the same one used by
        // the outer dispatch loop — if a `set -b` notification fires
        // between `C-q` and the user's next keystroke, we re-emit
        // the prompt + buffer so the literal byte they're about to
        // type still lands on a clean line.
        let buf_ref = &state.buf;
        let cursor_now = state.cursor;
        let anchor = &mut state.draw_anchor;
        let (maybe_byte, _intr) = read_byte_with_signal_handler(shell, || {
            write_bytes(b"\r\n");
            anchor.reset();
            redraw(anchor, buf_ref, cursor_now, prompt, ps2);
        })?;
        if let Some(b) = maybe_byte {
            state.insert_bytes_at_cursor(&[b]);
        }
    }
    if let Some(tmp_path) = outcome.edit_and_execute {
        let _ = keymap; // keymap borrowed just to keep the signature stable.
        return run_external_editor(shell, prompt, ps2, state, &tmp_path, raw);
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn run_incremental_search(
    shell: &mut Shell,
    prompt: &[u8],
    ps2: &[u8],
    keymap: &Keymap,
    state: &mut EmacsState,
    direction: Direction,
    raw: &RawMode,
) -> sys::error::SysResult<Option<Option<Vec<u8>>>> {
    let saved_buf = state.buf.clone();
    let saved_cursor = state.cursor;
    let history: Vec<Box<[u8]>> = shell.history().clone();
    let mut search = IncrementalSearch::new(&history, direction);
    draw_search_prompt(&search, &state.buf);
    loop {
        // Same `EINTR` policy as the outer dispatch loop: drain
        // notifications, redraw the search mini-buffer if a
        // status line was just emitted, then continue. We cannot
        // simply use `redraw(&state.buf, …)` here because the
        // search UI lives in its own mini-buffer rendered by
        // `draw_search_prompt`; a notification is followed by a
        // CRLF + redraw of the search prompt so the user keeps
        // their typed pattern. With `set -b` the notification fires
        // immediately and the closure re-emits the search
        // mini-buffer below it; with `set +b` it stays stashed and
        // appears at the next regular prompt after the search
        // terminates.
        let display = state.buf.clone();
        let (maybe_byte, _intr) = read_byte_with_signal_handler(shell, || {
            write_bytes(b"\r\n");
            draw_search_prompt(&search, &display);
        })?;
        let byte = match maybe_byte {
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
                draw_search_prompt(&search, &state.buf);
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
                // fired — per spec § 7.2. The search mini-buffer
                // overwrote the regular edit row with `\r\x1b[K(…)`,
                // so the incremental-redraw snapshot no longer
                // reflects what is on screen — drop it to force a
                // full repaint of the prompt + buffer.
                state.draw_anchor.reset();
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
                let mut line = std::mem::take(&mut state.buf);
                line.push(b'\n');
                return Ok(Some(Some(line)));
            }
            SearchOutcome::Abort => {
                state.buf = saved_buf;
                state.cursor = saved_cursor;
                state.draw_anchor.reset();
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
                return Ok(None);
            }
            SearchOutcome::Exit { byte: redispatch } => {
                if let Some(idx) = search.matched() {
                    state.buf = history[idx].to_vec();
                    state.cursor = state.buf.len();
                }
                state.undo.clear();
                state.draw_anchor.reset();
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
                let mut pending = Vec::new();
                if let Some(res) = handle_byte(
                    shell,
                    prompt,
                    ps2,
                    keymap,
                    state,
                    &mut pending,
                    redispatch,
                    raw,
                )? {
                    return Ok(Some(res));
                }
                redraw(
                    &mut state.draw_anchor,
                    &state.buf,
                    state.cursor,
                    prompt,
                    ps2,
                );
                return Ok(None);
            }
        }
    }
}

/// Render the incremental-search mini-buffer: the
/// `(reverse-i-search`pat'): ` prompt followed by the current matched
/// history line, with the terminal cursor parked at the start of the
/// matched substring (readline § 7 behaviour). Without the inline
/// `line`, the user only sees their typed pattern and has no way to
/// tell which history entry the search selected.
fn draw_search_prompt(search: &IncrementalSearch<'_>, line: &[u8]) {
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
    buf.extend_from_slice(line);
    // Park the cursor at the start of the matched substring within the
    // displayed line. The next redraw begins with `\r`, which resets
    // the column, so this move never desyncs subsequent updates.
    let pattern = search.pattern();
    let match_off = if pattern.is_empty() {
        0
    } else {
        leftmost_offset(line, pattern).unwrap_or(0)
    };
    let back = display_width(line).saturating_sub(display_width(&line[..match_off]));
    if back > 0 {
        buf.extend_from_slice(b"\x1b[");
        crate::bstr::push_u64(&mut buf, back as u64);
        buf.push(b'D');
    }
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &buf);
}

/// Byte offset of the leftmost occurrence of `needle` in `hay`, or
/// `None` if absent. Used to position the i-search cursor on the
/// matched text.
fn leftmost_offset(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn run_external_editor(
    shell: &mut Shell,
    prompt: &[u8],
    ps2: &[u8],
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
    // The external editor scribbled all over the terminal — there's
    // no reliable way to know which row we are on, so drop the
    // wrap-tracking anchor before painting the fresh prompt.
    state.draw_anchor.reset();
    redraw(
        &mut state.draw_anchor,
        &state.buf,
        state.cursor,
        prompt,
        ps2,
    );
    Ok(None)
}

fn is_printable(b: u8) -> bool {
    b >= 0x20 && b != 0x7f
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::interactive::inputrc;
    use crate::shell::test_support::test_shell;
    use crate::sys::constants::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    use self::keymap::KeymapEntry;

    const PASTE_ON: &[u8] = b"\x1b[?2004h";
    const PASTE_OFF: &[u8] = b"\x1b[?2004l";
    // The wrap-aware redraw clears from the cursor to end-of-screen
    // (`\x1b[J`) rather than just the current row (`\x1b[K`), so
    // stale wrapped tails above the cursor get wiped instead of
    // becoming ghost lines. Tests that go through the test-mode
    // tty (which reports no terminal width) still see exactly this
    // prefix once per redraw.
    const CLEAR_LINE: &[u8] = b"\r\x1b[J";

    /// Snapshot the global emacs context (via the shared inputrc test
    /// helper), install a caller-supplied keymap customization with
    /// `startup_loaded = true` so `ensure_startup_loaded` stays a
    /// no-op, run `body`, and let the helper restore the previous
    /// snapshot on the way out.
    fn with_default_keymap<F: FnOnce()>(customize: impl FnOnce(&mut keymap::Keymap), body: F) {
        inputrc::test_helpers::with_fresh_global(|| {
            {
                let mut g = match inputrc::global().lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                g.keymap = keymap::Keymap::default_emacs();
                g.startup_loaded = true;
                customize(&mut g.keymap);
            }
            body();
        });
    }

    #[test]
    fn read_line_accepts_single_character_on_enter() {
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'a']),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"a")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"a\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_eof_returns_none() {
        // Covers the `read_byte()? == None` arm in `dispatch_loop`
        // (line 94) that terminates the read on EOF.
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, None);
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_ctrl_d_on_empty_line_returns_none() {
        // Pressing C-d on an empty buffer triggers the
        // `outcome.eof` arm in `apply_function`, which returns
        // `Ok(Some(None))` and ends the read loop with `None`.
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x04]),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, None);
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_rings_bell_for_unbound_multibyte_sequence() {
        // `ESC X` with `X` unbound ⇒ pending stays multibyte and the
        // `Unbound` arm takes the `bell(); pending.clear()` path.  We
        // then hit Enter to exit the loop.
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        read(fd(STDIN_FILENO), _) -> bytes([b'~']),
                        write(fd(STDOUT_FILENO), bytes(b"\x07")) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_forward_search_empty_aborts_and_returns_empty_line() {
        // Press C-s with no history: the forward-search state machine
        // immediately reports failure because the empty pattern has no
        // matches; pressing ESC aborts and leaves the line empty, then
        // Enter accepts.  Exercises lines 164–166 (ForwardSearchHistory
        // dispatch) and the `SearchOutcome::Abort` branch in
        // `run_incremental_search`.
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x13]),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(i-search`'): ")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x07]),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_forward_search_fails_then_eofs_inside_search() {
        // Enter incremental search with C-s, type a char that fails to
        // match (we have no history), then EOF while still in the search
        // mini-buffer.  Covers lines 241–243 (EOF within
        // `run_incremental_search`) and the "failing i-search" prompt on
        // line 298/305.
        with_default_keymap(
            |_| {},
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x13]),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(i-search`'): ")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'x']),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(failing i-search`x'): ")) -> auto,
                        read(fd(STDIN_FILENO), _) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, None);
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_reverse_search_exits_on_redispatched_accept_line() {
        // History has one entry that matches "hi".  C-r + "h" finds it
        // and sets `state.buf = history[idx]`.  We've bound ESC (0x1b)
        // to `accept-line` so that pressing ESC while inside the
        // search mini-buffer triggers `SearchOutcome::Exit` with
        // `byte: 0x1b` — redispatch then fires `accept-line` and
        // propagates the accepted line all the way out.  Covers
        // lines 276–287 (the Exit body plus the early
        // `return Ok(Some(res));`).
        with_default_keymap(
            |km| {
                km.bind(b"\x1b", KeymapEntry::Func(EmacsFn::AcceptLine));
            },
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x12]),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(reverse-i-search`'): ")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'h']),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(reverse-i-search`h'): hi\x1b[2D")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"hi")) -> auto,
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        shell.add_history(b"hi");
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"hi\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_reverse_search_exit_falls_through_when_redispatch_no_op() {
        // Same setup as above, but ESC is bound to `BeginningOfLine`
        // which doesn't accept — so after the Exit body we fall
        // through to lines 289–290 (the trailing `redraw` +
        // `Ok(None)`), and the user's next Enter accepts normally.
        with_default_keymap(
            |km| {
                km.bind(b"\x1b", KeymapEntry::Func(EmacsFn::BeginningOfLine));
            },
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x12]),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(reverse-i-search`'): ")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'h']),
                        write(fd(STDERR_FILENO), bytes(b"\r\x1b[K(reverse-i-search`h'): hi\x1b[2D")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"hi")) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"hi\x1b[2D")) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"hi\x1b[2D")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        shell.add_history(b"hi");
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"hi\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_dispatches_macro_binding() {
        // Bind ESC `m` to the macro "ok" so the keymap resolves to
        // `Resolved::Macro(...)`, which expands to self-inserts via
        // recursive `handle_byte`.  Covers lines 173–181.
        with_default_keymap(
            |km| {
                km.bind(b"\x1bm", KeymapEntry::Macro(b"ok".to_vec()));
            },
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        read(fd(STDIN_FILENO), _) -> bytes([b'm']),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"ok")) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"ok\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_macro_accepting_newline_returns_immediately() {
        // A macro whose body ends in `\n` causes the recursive
        // `handle_byte` to fire `accept-line` and bubble up a line.
        // Covers line 178 (`return Ok(Some(res));` inside the macro
        // dispatch loop).
        with_default_keymap(
            |km| {
                km.bind(b"\x1bm", KeymapEntry::Macro(b"ok\n".to_vec()));
            },
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        read(fd(STDIN_FILENO), _) -> bytes([b'm']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"ok\n".to_vec()));
                    },
                );
            },
        );
    }

    #[test]
    fn read_line_runs_bound_shell_command() {
        // Bind ESC `s` to a `bind -x`-style shell command that
        // mutates READLINE_LINE.  Covers lines 183–186 (the
        // `Resolved::ExecShell` arm).
        with_default_keymap(
            |km| {
                km.bind(b"\x1bs", KeymapEntry::ExecShell(b":".to_vec()));
            },
            || {
                run_trace(
                    trace_entries![
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                        tcgetattr(fd(STDIN_FILENO)) -> 0,
                        write(fd(STDOUT_FILENO), bytes(PASTE_ON)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([0x1b]),
                        read(fd(STDIN_FILENO), _) -> bytes([b's']),
                        write(fd(STDOUT_FILENO), bytes(CLEAR_LINE)) -> auto,
                        read(fd(STDIN_FILENO), _) -> bytes([b'\n']),
                        write(fd(STDOUT_FILENO), bytes(PASTE_OFF)) -> auto,
                        write(fd(STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                        tcsetattr(fd(STDIN_FILENO), int(0)) -> 0,
                    ],
                    || {
                        let mut shell = test_shell();
                        let line = super::read_line(&mut shell, b"").unwrap();
                        assert_eq!(line, Some(b"\n".to_vec()));
                    },
                );
            },
        );
    }
}
