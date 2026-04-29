//! Bash-style process substitution `<(list)` / `>(list)`.
//!
//! Implements the runtime side of
//! [`docs/features/process-substitution.md`](../../../docs/features/process-substitution.md):
//!
//! - Fork a subshell to run the embedded list with stdin or stdout
//!   connected through a pipe to a substitution file descriptor in
//!   the parent shell (§ 5).
//! - Return the path-shaped word that opens that fd (§ 6).
//!
//! ## Path representation
//!
//! This implementation always emits a `/dev/fd/N` path. On Linux that
//! is a symlink under `/proc/self/fd`; on macOS and other BSDs it is
//! a kernel-provided directory entry. The spec § 6.3 also mandates a
//! FIFO fallback under `${TMPDIR:-/tmp}` for systems without
//! `/dev/fd`; that fallback is intentionally out of scope for the
//! initial implementation and tracked in Appendix B of the spec. On
//! a system without `/dev/fd` the consuming command will simply fail
//! to open the substituted path with `ENOENT` — a clean degraded
//! mode that the test suite does not exercise because the shell only
//! supports Linux today.
//!
//! ## Lifetime
//!
//! Each successful substitution pushes a [`ProcSubLease`] onto
//! [`Shell::proc_sub_leases`]. The consuming command's exit hook
//! (`drain_proc_sub_leases_to`) closes the parent-side fd and reaps
//! the subshell. The leases are popped in reverse order so the
//! left-to-right substitution order in the source line is preserved
//! (§ 7.3).

use std::rc::Rc;

use crate::bstr::ByteWriter;
use crate::syntax::ast::Program;
use crate::syntax::word_part::ProcSubDirection;
use crate::sys;
use crate::sys::types::Pid;

use super::state::Shell;

/// One active process substitution. Created during arg expansion;
/// drained after the consuming command finishes.
///
/// `Clone` is implemented so [`Shell`] (which derives `Clone` for
/// subshell forks) compiles, but the cloned vector represents a
/// **duplicated** view of leases the parent already owns. Subshell
/// forks (`(...)`, function bodies, command substitutions) must
/// clear `proc_sub_leases` immediately after forking so they do not
/// double-close fds or double-reap pids the parent will clean up.
/// `process_substitute` does this for its own subshell;
/// [`crate::shell::run::Shell::capture_output_program`] does the same
/// for `$(...)`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProcSubLease {
    /// Substitution fd held by the parent shell. For `<(list)` this
    /// is the read end of the pipe; for `>(list)` the write end. The
    /// consuming command (forked from the parent) inherits the fd
    /// and resolves `/dev/fd/N` against it. Closed by the cleanup
    /// hook after the consumer exits.
    pub(crate) fd: i32,
    /// Subshell child running the embedded list. Reaped by the
    /// cleanup hook with `waitpid(2)`.
    pub(crate) child_pid: Pid,
}

/// Set up a process substitution and return the substituted path.
///
/// Errors are returned as a `Vec<u8>` diagnostic body that
/// [`Context::process_substitute`](crate::expand::core::Context::process_substitute)
/// wraps into an `ExpandError`. The body intentionally omits the
/// `meiksh:` prefix and trailing newline; those are added by the
/// expansion error reporter.
pub(crate) fn process_substitute(
    shell: &mut Shell,
    program: &Rc<Program>,
    direction: ProcSubDirection,
) -> Result<Vec<u8>, Vec<u8>> {
    let (read_fd, write_fd) = sys::fd_io::create_pipe().map_err(|e| diagnose(b"pipe", &e))?;

    // Decide which end the parent retains and which end the child
    // wires to its standard stream. For `<(list)` the child writes
    // (stdout = pipe[write]) and the parent reads (parent_fd =
    // pipe[read]). For `>(list)` it is the mirror.
    let (parent_fd, child_pipe_end, child_target_fd) = match direction {
        ProcSubDirection::Read => (read_fd, write_fd, sys::constants::STDOUT_FILENO),
        ProcSubDirection::Write => (write_fd, read_fd, sys::constants::STDIN_FILENO),
    };
    let parent_other_end = match direction {
        ProcSubDirection::Read => write_fd,
        ProcSubDirection::Write => read_fd,
    };

    let pid = match sys::process::fork_process() {
        Ok(pid) => pid,
        Err(e) => {
            // On fork failure roll back the pipe so we do not leak
            // the file descriptors.
            let _ = sys::fd_io::close_fd(read_fd);
            let _ = sys::fd_io::close_fd(write_fd);
            return Err(diagnose(b"fork", &e));
        }
    };

    if pid == 0 {
        // Child path. Wire the appropriate pipe end to the standard
        // fd, close the pipe ends we own, and execute the embedded
        // program. Per spec § 5.1 the subshell is a POSIX subshell
        // (§ 2.13), so we mark `in_subshell`, clear the parent's
        // job-control state, restore signals, and reset traps before
        // executing.
        let _ = sys::fd_io::close_fd(parent_fd);
        let _ = sys::fd_io::duplicate_fd(child_pipe_end, child_target_fd);
        let _ = sys::fd_io::close_fd(child_pipe_end);
        // The substitution subshell does not inherit other in-flight
        // procsub leases from the parent. The parent owns those; the
        // subshell would close fds the parent still needs at exit.
        shell.proc_sub_leases.clear();
        shell.owns_terminal = false;
        shell.in_subshell = true;
        shell.subshell_nesting_level = shell.subshell_nesting_level.saturating_add(1);
        shell.restore_signals_for_child();
        let _ = shell.reset_traps_for_subshell();
        let status = shell.execute_program(program).unwrap_or(1);
        let status = shell.run_exit_trap(status).unwrap_or(status);
        sys::process::exit_process(status as sys::types::RawFd);
    }

    // Parent path. Close the pipe end we are not retaining; the
    // remaining end is the substitution fd `parent_fd`.
    if let Err(e) = sys::fd_io::close_fd(parent_other_end) {
        // The child has already forked; tear it down before
        // reporting the error so we do not orphan the subshell.
        let _ = sys::fd_io::close_fd(parent_fd);
        let _ = reap_until_exit(pid);
        return Err(diagnose(b"close", &e));
    }

    let path = format_dev_fd_path(parent_fd);
    shell.proc_sub_leases.push(ProcSubLease {
        fd: parent_fd,
        child_pid: pid,
    });
    Ok(path)
}

/// Drain all leases pushed at-or-after `mark` from
/// [`Shell::proc_sub_leases`], in reverse insertion order, closing
/// each parent-side fd and reaping the subshell. Used by the
/// consuming command's exit hook.
///
/// The reverse order matches spec § 7.3: leases were created
/// left-to-right during arg expansion, so popping from the back
/// reaps the rightmost subshell first. This is observable only
/// through diagnostic ordering and shall not affect the consuming
/// command's exit status (spec § 8.1).
pub(crate) fn drain_proc_sub_leases_to(shell: &mut Shell, mark: usize) {
    while shell.proc_sub_leases.len() > mark {
        let Some(lease) = shell.proc_sub_leases.pop() else {
            break;
        };
        cleanup_lease(lease);
    }
}

fn cleanup_lease(lease: ProcSubLease) {
    // Closing the parent-side fd lets the subshell observe EOF on
    // its stdin (`>(...)`) or `SIGPIPE` on its next stdout write
    // (`<(...)`), which lets cooperating programs exit promptly.
    let _ = sys::fd_io::close_fd(lease.fd);
    let _ = reap_until_exit(lease.child_pid);
}

/// Block until the subshell `pid` is reaped. EINTR / `WNOHANG=false`
/// retries are handled the same way `capture_output_program` handles
/// them so a `SIGCHLD` arriving for a different child does not
/// abort the wait.
fn reap_until_exit(pid: Pid) -> sys::error::SysResult<sys::types::WaitStatus> {
    loop {
        match sys::process::wait_pid(pid, false) {
            Ok(Some(ws)) => return Ok(ws),
            Ok(None) => continue,
            Err(ref e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Build the path `/dev/fd/<fd>` (e.g. `/dev/fd/12`).
fn format_dev_fd_path(fd: i32) -> Vec<u8> {
    ByteWriter::new()
        .bytes(b"/dev/fd/")
        .i64_val(fd as i64)
        .finish()
}

/// Format a `meiksh: process substitution: <syscall>: <strerror>`
/// diagnostic body per spec § 9.2. The leading `meiksh:` and
/// trailing newline are added by the expansion error reporter.
fn diagnose(syscall: &[u8], e: &sys::error::SysError) -> Vec<u8> {
    ByteWriter::new()
        .bytes(b"process substitution: ")
        .bytes(syscall)
        .bytes(b": ")
        .bytes(&e.strerror())
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn format_dev_fd_path_writes_decimal_fd() {
        assert_no_syscalls(|| {
            assert_eq!(format_dev_fd_path(0), b"/dev/fd/0");
            assert_eq!(format_dev_fd_path(3), b"/dev/fd/3");
            assert_eq!(format_dev_fd_path(99), b"/dev/fd/99");
        });
    }

    #[test]
    fn diagnose_formats_syscall_and_strerror() {
        assert_no_syscalls(|| {
            let body = diagnose(b"pipe", &sys::error::SysError::Errno(sys::constants::EIO));
            // The exact strerror text is libc-defined, but we can
            // assert the prefix structure mandated by spec § 9.2.
            assert!(
                body.starts_with(b"process substitution: pipe: "),
                "expected `process substitution: pipe: <strerror>`, got {body:?}",
            );
        });
    }

    #[test]
    fn cleanup_lease_closes_fd_and_reaps_child() {
        // Cleanup is the inverse of setup: close the fd, then waitpid
        // until the subshell exits. The trace covers the happy path.
        run_trace(
            trace_entries![
                close(fd(7)) -> 0,
                waitpid(1234, _) -> status(0),
            ],
            || {
                cleanup_lease(ProcSubLease {
                    fd: 7,
                    child_pid: 1234,
                });
            },
        );
    }

    #[test]
    fn drain_proc_sub_leases_pops_back_to_mark_in_reverse_order() {
        // Two leases pushed in left-to-right order; drain pops the
        // most recent first. Spec § 7.3 mandates this ordering.
        run_trace(
            trace_entries![
                close(fd(11)) -> 0,
                waitpid(1002, _) -> status(0),
                close(fd(10)) -> 0,
                waitpid(1001, _) -> status(0),
            ],
            || {
                let mut shell = crate::shell::test_support::test_shell();
                shell.proc_sub_leases.push(ProcSubLease {
                    fd: 10,
                    child_pid: 1001,
                });
                shell.proc_sub_leases.push(ProcSubLease {
                    fd: 11,
                    child_pid: 1002,
                });
                drain_proc_sub_leases_to(&mut shell, 0);
                assert!(shell.proc_sub_leases.is_empty());
            },
        );
    }

    #[test]
    fn drain_respects_mark_and_keeps_outer_leases() {
        // A nested consumer should drain only its own leases (those
        // pushed after `mark`) and leave outer-scope leases intact.
        run_trace(
            trace_entries![
                close(fd(12)) -> 0,
                waitpid(1003, _) -> status(0),
            ],
            || {
                let mut shell = crate::shell::test_support::test_shell();
                shell.proc_sub_leases.push(ProcSubLease {
                    fd: 10,
                    child_pid: 1001,
                });
                let mark = shell.proc_sub_leases.len();
                shell.proc_sub_leases.push(ProcSubLease {
                    fd: 12,
                    child_pid: 1003,
                });
                drain_proc_sub_leases_to(&mut shell, mark);
                assert_eq!(shell.proc_sub_leases.len(), 1);
                assert_eq!(shell.proc_sub_leases[0].fd, 10);
            },
        );
    }
}
