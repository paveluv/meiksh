//! Blocking byte-level I/O primitives used by the interactive editors.
//!
//! These wrap [`crate::sys::fd_io`] so keystroke dispatch loops and
//! redraw code don't have to thread syscalls or STDIN/STDOUT file
//! descriptor constants through every call site.

use crate::shell::state::Shell;
use crate::sys;

/// Read a single byte from `stdin`. Returns `Ok(None)` on EOF (which
/// matters for C-d / piped input), `Ok(Some(b))` for a real byte, or
/// the underlying [`SysError`] on read failure (including
/// [`sys::error::SysError::is_eintr`] — the editor is responsible for
/// translating an `EINTR` into the appropriate signal-driven action;
/// see [`read_byte_with_signal_handler`] for the high-level wrapper
/// used by the dispatch loop).
///
/// [`SysError`]: crate::sys::error::SysError
pub(crate) fn read_byte() -> sys::error::SysResult<Option<u8>> {
    let mut buf = [0u8; 1];
    match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut buf) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(buf[0])),
        Err(e) => Err(e),
    }
}

/// Outcome of the editor's `EINTR`-handling pass. Used only for
/// signaling EOF-via-trap to the caller now that the per-iteration
/// redraw is performed inline via the caller-provided closure (see
/// [`read_byte_with_signal_handler`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct InterruptOutcome {
    /// `true` iff the read terminated by EOF *or* by a trap-driven
    /// shell shutdown rather than producing a byte.
    pub(crate) eof: bool,
}

/// Read a single byte while properly handling `EINTR` from
/// `SIGCHLD` (and other shell-installed signal handlers). The shell
/// installs `SIGCHLD` for the entire interactive session (see
/// [`crate::interactive::repl::run_loop`]), which means a blocking
/// `read()` will return `EINTR` whenever a background child changes
/// state. Per POSIX § 2.11 the shell must then either deliver an
/// immediate notification (`set -b`) or queue one for the next
/// prompt — see [`crate::interactive::notify`] for the policy.
///
/// Drain phases (run on entry, after each `EINTR`, AND before each
/// blocking `read()` retry):
/// - All zombie children are reaped via `shell.reap_jobs()`.
/// - Each resulting status line is routed by
///   [`crate::interactive::notify::stash_or_print_notifications`]
///   (immediate write vs. enqueue for the next prompt).
/// - Any pending shell traps are run via `shell.run_pending_traps()`.
///   A trap that sets `shell.running = false` (e.g. `trap 'exit' INT`
///   firing on `^C`) is reported by returning `Ok(None)` so the
///   dispatch loop unwinds cleanly to the REPL.
/// - If the drain wrote any bytes to stderr (immediate-mode
///   notification), `redraw` is invoked so the prompt + edit buffer
///   reappear *before* the next blocking `read()`. Doing the redraw
///   inside this helper (rather than after `read()` returns) is
///   essential when the editor was idle and the read would otherwise
///   block forever waiting for a user keystroke that never comes —
///   the test (and the user) would see a stale prompt with the
///   notification glued to it but no fresh prompt below.
///
/// `redraw` — caller-provided closure that re-emits a CRLF, the
/// prompt, and the in-progress edit buffer. Invoked once per drain
/// phase that actually printed something to stderr (i.e. `set -b`
/// fired immediately). The closure captures the editor's prompt +
/// buffer + cursor by reference so its output always reflects the
/// *current* edit state. We invoke it BEFORE the next blocking
/// `read()` rather than after it returns, so that even if the user
/// never types another keystroke they still see a fresh prompt
/// below the asynchronous status line.
///
/// Returns:
/// - `Ok((Some(b), outcome))` on a normal byte (most common path).
/// - `Ok((None, outcome))` on EOF *or* on a trap-driven shutdown.
/// - `Err(e)` on a real I/O failure (anything other than `EINTR`).
pub(crate) fn read_byte_with_signal_handler<F: FnMut()>(
    shell: &mut Shell,
    mut redraw: F,
) -> sys::error::SysResult<(Option<u8>, InterruptOutcome)> {
    let mut outcome = InterruptOutcome::default();
    loop {
        // Pre-check: a signal may have been delivered between the
        // previous syscall and now — for example, the kernel can
        // queue a `SIGCHLD` while the shell is in pure user-space
        // (running a builtin) without ever interrupting a syscall.
        // The signal handler would then merely set a pending bit,
        // and we'd miss the wake-up if our subsequent `read()` is
        // immediately satisfied from a non-empty PTY buffer (which
        // returns successfully without raising `EINTR`).
        //
        // Draining pending signals here also reaps any newly-dead
        // background jobs so `set +b` notifications get stashed
        // promptly. They aren't *delivered* until the next prompt
        // (per POSIX § 2.11); only `set -b` notifications produce
        // immediate stderr output and trigger the redraw closure.
        if drain_pending(shell) {
            redraw();
        }
        if !shell.running {
            outcome.eof = true;
            return Ok((None, outcome));
        }

        match read_byte() {
            Ok(Some(b)) => return Ok((Some(b), outcome)),
            Ok(None) => {
                outcome.eof = true;
                return Ok((None, outcome));
            }
            Err(e) if e.is_eintr() => {
                // `SIGCHLD` (and `SIGINT`-with-trap, etc.) — loop
                // back so the pre-read drain handles the bookkeeping
                // (and runs the redraw closure if `set -b` printed).
                if !shell.running {
                    outcome.eof = true;
                    return Ok((None, outcome));
                }
                // Loop and retry the `read()`.
            }
            Err(e) => return Err(e),
        }
    }
}

/// Reap zombies, route notifications, and run pending traps. Returns
/// `true` if anything was written to stderr right now so the caller
/// knows to redraw. With `set +b` (the default) the notification is
/// stashed onto [`Shell::pending_notifications`] for the next prompt
/// and this returns `false`; with `set -b` it's written immediately
/// and this returns `true`.
fn drain_pending(shell: &mut Shell) -> bool {
    let res = crate::interactive::notify::stash_or_print_notifications(shell);
    let _ = shell.run_pending_traps();
    res.printed_now > 0
}

/// Best-effort write of `data` to `stdout`. Errors are swallowed on
/// purpose; the editor can't do anything useful with a failed redraw
/// and the caller is usually rendering state it already committed to
/// an internal buffer.
pub(crate) fn write_bytes(data: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, data);
}

/// Ring the terminal bell.
pub(crate) fn bell() {
    write_bytes(b"\x07");
}
