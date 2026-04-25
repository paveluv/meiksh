//! Job-status notification formatting and dispatch.
//!
//! POSIX.1-2024 § 2.11 ("Asynchronous Lists") and the `set -b` /
//! `notify` option together prescribe **two** delivery times for
//! background-job status messages in an interactive shell:
//!
//! 1. With `set -b` ("notify") **on**: the shell shall write a
//!    message *immediately* upon noticing a change of state for a
//!    background job (typically when a `SIGCHLD` interrupts the
//!    blocking read in the line editor). The message must not wait
//!    for the next prompt.
//! 2. With `set -b` **off** (the default): the shell shall write the
//!    message "before the next prompt" — i.e., once per REPL
//!    iteration, *just before* `PS1` is rendered. Messages that arise
//!    asynchronously while the editor is reading must therefore be
//!    *queued* until the next prompt rather than printed mid-line.
//!
//! Both delivery paths share the same byte-for-byte output formatting
//! produced by [`format_notification`]. Path 1 is implemented by
//! `print_notifications_now` (it writes to `stderr` and triggers a
//! line redraw so the user's typing continues on a fresh prompt
//! line). Path 2 is implemented by enqueuing onto
//! [`Shell::pending_notifications`] and draining at the top of the
//! REPL loop.
//!
//! The function [`stash_or_print_notifications`] picks the right path
//! based on `shell.options.notify`. It is the single entry-point
//! called from the editor's `EINTR`-handling helper after every
//! `reap_jobs()` so that the two delivery semantics stay in lockstep.

use crate::bstr::ByteWriter;
use crate::shell::jobs::ReapedJobState;
use crate::shell::state::Shell;
use crate::sys;

/// Format a single reaped-job status change into the byte-for-byte
/// message that gets written to `stderr` (POSIX § 2.11: "the format
/// is unspecified" but every historical shell uses something
/// equivalent to the form below — `[id] STATE\tcommand\n`).
///
/// The formatting is identical regardless of which delivery path
/// (immediate vs. deferred) eventually emits the bytes.
pub(crate) fn format_notification(id: usize, state: &ReapedJobState) -> Vec<u8> {
    match state {
        ReapedJobState::Done(status, cmd) => {
            if *status == 0 {
                ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Done\t")
                    .bytes(cmd)
                    .byte(b'\n')
                    .finish()
            } else {
                ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Done(")
                    .i32_val(*status)
                    .bytes(b")\t")
                    .bytes(cmd)
                    .byte(b'\n')
                    .finish()
            }
        }
        ReapedJobState::Signaled(sig, cmd) => ByteWriter::new()
            .byte(b'[')
            .usize_val(id)
            .bytes(b"] Terminated (")
            .bytes(sys::process::signal_name(*sig))
            .bytes(b")\t")
            .bytes(cmd)
            .byte(b'\n')
            .finish(),
        ReapedJobState::Stopped(sig, cmd) => ByteWriter::new()
            .byte(b'[')
            .usize_val(id)
            .bytes(b"] Stopped (")
            .bytes(sys::process::signal_name(*sig))
            .bytes(b")\t")
            .bytes(cmd)
            .byte(b'\n')
            .finish(),
    }
}

/// Write a single notification to `stderr` immediately. Used by both
/// the `notify` (`set -b`) immediate path and by the prompt-time
/// drain path; the destination FD and write semantics are the same in
/// both cases — the two delivery paths only differ in *when* this
/// runs, not in *how*.
pub(crate) fn write_notification(msg: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, msg);
}

/// Outcome of [`stash_or_print_notifications`]: tells the caller
/// whether any bytes were written to stderr right now (so the editor
/// can redraw its line) versus deferred for the next prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct NotifyOutcome {
    /// Number of notifications that were written to stderr right
    /// now. The editor uses this to decide whether to re-emit `PS1`
    /// and the in-progress edit buffer (so they don't get
    /// "interleaved" by the asynchronous status line).
    pub(crate) printed_now: usize,
    /// Number of notifications that were stashed onto
    /// [`Shell::pending_notifications`] for later drain at the next
    /// prompt.
    pub(crate) stashed: usize,
}

/// Reap any zombie background jobs, then route each resulting status
/// message according to the `notify` (`set -b`) option:
///
/// - `notify` on  → write immediately to stderr. The editor caller
///   is expected to follow up with a line redraw so the in-progress
///   edit (or, if the buffer is empty, the prompt) reappears below
///   the asynchronous status line.
/// - `notify` off → push onto [`Shell::pending_notifications`] for
///   the next prompt to drain. The message is *not* lost: it is
///   guaranteed to be written before the next `PS1` byte (POSIX §
///   2.11: "before the next prompt"). "Next prompt" specifically
///   means the next time the REPL loop is about to call
///   [`crate::interactive::prompt::write_prompt`] — not the prompt
///   that's already on screen with the user staring at it. If the
///   user is mid-edit (or just sitting at an empty prompt) when the
///   bg child dies, the notification waits for them to submit their
///   next command and have its prompt written; that's the historical
///   bash/ksh behavior and what POSIX prescribes.
///
/// Returns a [`NotifyOutcome`] describing what happened so the caller
/// can decide whether to redraw.
pub(crate) fn stash_or_print_notifications(shell: &mut Shell) -> NotifyOutcome {
    let reaped = shell.reap_jobs();
    let immediate = shell.options.notify;
    let mut printed_now = 0usize;
    let mut stashed = 0usize;

    for (id, state) in reaped {
        let msg = format_notification(id, &state);
        if immediate {
            write_notification(&msg);
            printed_now += 1;
        } else {
            shell.pending_notifications.push(msg);
            stashed += 1;
        }
    }
    NotifyOutcome {
        printed_now,
        stashed,
    }
}

/// Drain any deferred notifications from
/// [`Shell::pending_notifications`] and write them to stderr in FIFO
/// order. Called at the top of the REPL loop just before the prompt
/// is rendered (POSIX § 2.11 "before the next prompt"). A no-op when
/// the queue is empty.
pub(crate) fn drain_pending_notifications(shell: &mut Shell) {
    if shell.pending_notifications.is_empty() {
        return;
    }
    let pending = std::mem::take(&mut shell.pending_notifications);
    for msg in pending {
        write_notification(&msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::jobs::ReapedJobState;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn done_zero_status_renders_done_no_status_suffix() {
        // POSIX-compatible historical shells render a clean exit as
        // `[N] Done\tCMD` — no `(0)` suffix. A non-zero status
        // *does* show the suffix (so the user sees the failure).
        assert_no_syscalls(|| {
            let msg = format_notification(
                3,
                &ReapedJobState::Done(0, b"ls -l".to_vec().into_boxed_slice()),
            );
            assert_eq!(msg, b"[3] Done\tls -l\n");
            let msg = format_notification(
                4,
                &ReapedJobState::Done(7, b"false".to_vec().into_boxed_slice()),
            );
            assert_eq!(msg, b"[4] Done(7)\tfalse\n");
        });
    }

    #[test]
    fn signaled_renders_signal_name() {
        // The signal name is rendered with its `SIG` prefix per
        // `signal_name` so the output matches what the matrix test
        // suite expects (`[N] Terminated (SIGTERM)\tCMD\n`).
        assert_no_syscalls(|| {
            let msg = format_notification(
                1,
                &ReapedJobState::Signaled(15, b"sleep 30".to_vec().into_boxed_slice()),
            );
            assert!(msg.starts_with(b"[1] Terminated ("));
            assert!(msg.ends_with(b")\tsleep 30\n"));
        });
    }

    #[test]
    fn stopped_renders_signal_name() {
        assert_no_syscalls(|| {
            let msg = format_notification(
                2,
                &ReapedJobState::Stopped(
                    sys::constants::SIGTSTP,
                    b"vi".to_vec().into_boxed_slice(),
                ),
            );
            assert!(msg.starts_with(b"[2] Stopped ("));
            assert!(msg.ends_with(b")\tvi\n"));
        });
    }

    #[test]
    fn drain_writes_in_fifo_order_and_clears_queue() {
        // `drain_pending_notifications` writes everything in the
        // order it was queued, then leaves the queue empty so a
        // subsequent prompt-time drain is a no-op.
        // `RefCell` because `run_trace` takes `Fn` (single-shot
        // closure runner) but we need `&mut` access to `shell`.
        let shell = std::cell::RefCell::new(crate::interactive::test_support::test_shell());
        shell
            .borrow_mut()
            .pending_notifications
            .push(b"first\n".to_vec());
        shell
            .borrow_mut()
            .pending_notifications
            .push(b"second\n".to_vec());
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"first\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"second\n")) -> auto,
            ],
            || {
                drain_pending_notifications(&mut shell.borrow_mut());
            },
        );
        assert!(shell.borrow().pending_notifications.is_empty());
        assert_no_syscalls(|| drain_pending_notifications(&mut shell.borrow_mut()));
    }
}
