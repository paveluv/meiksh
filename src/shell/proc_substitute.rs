//! Bash-style process substitution `<(list)` / `>(list)`.
//!
//! Implements the runtime side of
//! [`docs/features/process-substitution.md`](../../../docs/features/process-substitution.md):
//!
//! - Fork a subshell to run the embedded list with stdin or stdout
//!   connected through a pipe (or FIFO) to a substitution path in
//!   the parent shell (§ 5).
//! - Return the path-shaped word that opens that path (§ 6).
//!
//! ## Path representation
//!
//! Two backings, selected by a runtime probe of `/dev/fd` (§ 6.1):
//!
//! * **`/dev/fd/N`** ([`ProcSubBacking::DevFd`]): the parent forks
//!   with a `pipe(2)`, retains one end as fd N, and emits the path
//!   `/dev/fd/N`. Used on Linux (where `/dev/fd` is a symlink to
//!   `/proc/self/fd`) and on macOS / *BSDs that mount `fdescfs`.
//! * **Named FIFO under `${TMPDIR:-/tmp}`** ([`ProcSubBacking::Fifo`]):
//!   the parent creates a FIFO via `mkfifo(2)` with permissions
//!   `0600`, forks the subshell, and the subshell opens the FIFO
//!   itself with the appropriate direction. The path is the FIFO
//!   path. Used on systems without `/dev/fd`. The cleanup hook
//!   unlinks the FIFO after the consumer exits.
//!
//! ## Lifetime
//!
//! Each successful substitution pushes a [`ProcSubLease`] onto
//! [`Shell::proc_sub_leases`]. The consuming command's exit hook
//! (`drain_proc_sub_leases_to`) closes the parent-side fd (or
//! unlinks the FIFO), then reaps the subshell. The leases are popped
//! in reverse order so the left-to-right substitution order in the
//! source line is preserved (§ 7.3).

use std::rc::Rc;

use crate::bstr::ByteWriter;
use crate::syntax::ast::Program;
use crate::syntax::word_part::ProcSubDirection;
use crate::sys;
use crate::sys::types::Pid;

use super::state::Shell;

/// What the parent shell holds for an active substitution. The
/// choice is made by [`process_substitute`] based on the runtime
/// probe; cleanup ([`cleanup_lease`]) inspects the variant to decide
/// between `close(fd)` and `unlink(path)`.
#[derive(Clone, Debug)]
pub(crate) enum ProcSubBacking {
    /// `/dev/fd/N` path; parent retains the substitution `fd`.
    DevFd { fd: i32 },
    /// FIFO path; parent does not hold an fd. The path is unlinked
    /// during cleanup.
    Fifo { path: Vec<u8> },
}

/// One active process substitution. Created during arg expansion;
/// drained after the consuming command finishes.
///
/// `Clone` is implemented so [`Shell`] (which derives `Clone` for
/// subshell forks) compiles, but the cloned vector represents a
/// **duplicated** view of leases the parent already owns. Subshell
/// forks (`(...)`, function bodies, command substitutions) must
/// clear `proc_sub_leases` immediately after forking so they do not
/// double-close fds, double-reap pids, or double-unlink FIFOs the
/// parent will clean up. `process_substitute` does this for its own
/// subshell;
/// [`crate::shell::run::Shell::capture_output_program`] does the same
/// for `$(...)`.
#[derive(Clone, Debug)]
pub(crate) struct ProcSubLease {
    /// What the parent shell holds: a substitution fd (for
    /// `/dev/fd/N` mode) or a FIFO path (for the FIFO fallback).
    pub(crate) backing: ProcSubBacking,
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
    if dev_fd_supported(shell) {
        process_substitute_dev_fd(shell, program, direction)
    } else {
        process_substitute_fifo(shell, program, direction)
    }
}

/// Lazy-initialize and return the cached `/dev/fd` probe result.
fn dev_fd_supported(shell: &Shell) -> bool {
    if let Some(supported) = shell.dev_fd_supported.get() {
        return supported;
    }
    let supported = sys::fs::dev_fd_supported();
    shell.dev_fd_supported.set(Some(supported));
    supported
}

/// `/dev/fd/N` backing path: pipe + fork; child dups its end to
/// stdin/stdout; parent retains the other end as fd N.
fn process_substitute_dev_fd(
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
            let _ = sys::fd_io::close_fd(read_fd);
            let _ = sys::fd_io::close_fd(write_fd);
            return Err(diagnose(b"fork", &e));
        }
    };

    if pid == 0 {
        let _ = sys::fd_io::close_fd(parent_fd);
        let _ = sys::fd_io::duplicate_fd(child_pipe_end, child_target_fd);
        let _ = sys::fd_io::close_fd(child_pipe_end);
        run_subshell(shell, program);
    }

    if let Err(e) = sys::fd_io::close_fd(parent_other_end) {
        let _ = sys::fd_io::close_fd(parent_fd);
        let _ = reap_until_exit(pid);
        return Err(diagnose(b"close", &e));
    }

    let path = format_dev_fd_path(parent_fd);
    shell.proc_sub_leases.push(ProcSubLease {
        backing: ProcSubBacking::DevFd { fd: parent_fd },
        child_pid: pid,
    });
    Ok(path)
}

/// FIFO backing path: mkfifo + fork; child opens the FIFO with the
/// appropriate direction (rendezvousing with the consumer's open),
/// dups it to stdin/stdout, and runs the program. Parent does not
/// hold an fd; cleanup unlinks the FIFO.
fn process_substitute_fifo(
    shell: &mut Shell,
    program: &Rc<Program>,
    direction: ProcSubDirection,
) -> Result<Vec<u8>, Vec<u8>> {
    let path = generate_fifo_path(shell);
    sys::fs::make_fifo(&path, sys::constants::S_IRUSR_BITS).map_err(|e| diagnose(b"mkfifo", &e))?;

    let (child_open_flags, child_target_fd) = match direction {
        // `<(producer)`: child writes to FIFO; consumer reads.
        ProcSubDirection::Read => (sys::constants::O_WRONLY, sys::constants::STDOUT_FILENO),
        // `>(consumer)`: child reads from FIFO; producer writes.
        ProcSubDirection::Write => (sys::constants::O_RDONLY, sys::constants::STDIN_FILENO),
    };

    let pid = match sys::process::fork_process() {
        Ok(pid) => pid,
        Err(e) => {
            let _ = sys::fs::unlink(&path);
            return Err(diagnose(b"fork", &e));
        }
    };

    if pid == 0 {
        // The child's open(FIFO) blocks until the consumer (or
        // producer, for `>(...)`) opens the other end. That
        // rendezvous is the whole point of the FIFO fallback.
        let fd = match sys::fs::open_file(&path, child_open_flags, 0) {
            Ok(fd) => fd,
            Err(_) => {
                sys::process::exit_process(1);
            }
        };
        let _ = sys::fd_io::duplicate_fd(fd, child_target_fd);
        let _ = sys::fd_io::close_fd(fd);
        run_subshell(shell, program);
    }

    shell.proc_sub_leases.push(ProcSubLease {
        backing: ProcSubBacking::Fifo { path: path.clone() },
        child_pid: pid,
    });
    Ok(path)
}

/// Run the embedded `program` in this (forked-child) process and
/// `_exit`. Caller has already wired the appropriate fds. Per spec
/// § 5.1 the subshell is a POSIX subshell: `in_subshell` set,
/// signals restored, traps reset. The substitution subshell does
/// not inherit other in-flight procsub leases from the parent; the
/// parent owns those.
fn run_subshell(shell: &mut Shell, program: &Rc<Program>) -> ! {
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

/// Build the FIFO path
/// `${TMPDIR:-/tmp}/meiksh-procsub.<pid>.<seq>` and increment the
/// per-shell seq counter so the next call gets a fresh basename.
fn generate_fifo_path(shell: &mut Shell) -> Vec<u8> {
    let tmpdir: Vec<u8> = shell
        .var_value(b"TMPDIR")
        .filter(|v| !v.is_empty())
        .map(<[u8]>::to_vec)
        .unwrap_or_else(|| b"/tmp".to_vec());
    let seq = shell.proc_sub_seq;
    shell.proc_sub_seq = shell.proc_sub_seq.wrapping_add(1);
    let mut buf = ByteWriter::new().bytes(&tmpdir).bytes(b"/meiksh-procsub.");
    buf = buf.i64_val(shell.pid as i64);
    buf = buf.bytes(b".").i64_val(seq as i64);
    buf.finish()
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
    // For `/dev/fd/N` mode: closing the parent-side fd lets the
    // subshell observe EOF on its stdin (`>(...)`) or `SIGPIPE` on
    // its next stdout write (`<(...)`), so cooperating programs
    // exit promptly. For FIFO mode: there is no parent-side fd to
    // close, but unlinking the FIFO node removes it from the
    // filesystem; the subshell's open fd keeps the inode alive
    // until it exits, then the kernel reclaims it.
    match lease.backing {
        ProcSubBacking::DevFd { fd } => {
            let _ = sys::fd_io::close_fd(fd);
        }
        ProcSubBacking::Fifo { path } => {
            let _ = sys::fs::unlink(&path);
        }
    }
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
        // /dev/fd backing: cleanup closes the fd, then waits.
        run_trace(
            trace_entries![
                close(fd(7)) -> 0,
                waitpid(1234, _) -> status(0),
            ],
            || {
                cleanup_lease(ProcSubLease {
                    backing: ProcSubBacking::DevFd { fd: 7 },
                    child_pid: 1234,
                });
            },
        );
    }

    #[test]
    fn cleanup_lease_unlinks_fifo_and_reaps_child() {
        // FIFO backing: cleanup unlinks the path, then waits. The
        // subshell still has its own fd open against the FIFO; the
        // unlink only removes the directory entry.
        run_trace(
            trace_entries![
                unlink(str(b"/tmp/meiksh-procsub.99.1")) -> 0,
                waitpid(1234, _) -> status(0),
            ],
            || {
                cleanup_lease(ProcSubLease {
                    backing: ProcSubBacking::Fifo {
                        path: b"/tmp/meiksh-procsub.99.1".to_vec(),
                    },
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
                    backing: ProcSubBacking::DevFd { fd: 10 },
                    child_pid: 1001,
                });
                shell.proc_sub_leases.push(ProcSubLease {
                    backing: ProcSubBacking::DevFd { fd: 11 },
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
                    backing: ProcSubBacking::DevFd { fd: 10 },
                    child_pid: 1001,
                });
                let mark = shell.proc_sub_leases.len();
                shell.proc_sub_leases.push(ProcSubLease {
                    backing: ProcSubBacking::DevFd { fd: 12 },
                    child_pid: 1003,
                });
                drain_proc_sub_leases_to(&mut shell, mark);
                assert_eq!(shell.proc_sub_leases.len(), 1);
                assert!(matches!(
                    shell.proc_sub_leases[0].backing,
                    ProcSubBacking::DevFd { fd: 10 }
                ));
            },
        );
    }

    #[test]
    fn drain_with_mixed_backings_dispatches_correctly() {
        // Stack with FIFO on top, DevFd at bottom. Drain pops in
        // reverse order: FIFO first (unlink), then DevFd (close).
        run_trace(
            trace_entries![
                unlink(str(b"/tmp/meiksh-procsub.7.1")) -> 0,
                waitpid(1100, _) -> status(0),
                close(fd(20)) -> 0,
                waitpid(1101, _) -> status(0),
            ],
            || {
                let mut shell = crate::shell::test_support::test_shell();
                shell.proc_sub_leases.push(ProcSubLease {
                    backing: ProcSubBacking::DevFd { fd: 20 },
                    child_pid: 1101,
                });
                shell.proc_sub_leases.push(ProcSubLease {
                    backing: ProcSubBacking::Fifo {
                        path: b"/tmp/meiksh-procsub.7.1".to_vec(),
                    },
                    child_pid: 1100,
                });
                drain_proc_sub_leases_to(&mut shell, 0);
                assert!(shell.proc_sub_leases.is_empty());
            },
        );
    }

    // --- generate_fifo_path -----------------------------------------

    #[test]
    fn generate_fifo_path_uses_default_tmpdir_when_unset() {
        assert_no_syscalls(|| {
            let mut shell = crate::shell::test_support::test_shell();
            shell.pid = 4242;
            shell.proc_sub_seq = 1;
            let path = generate_fifo_path(&mut shell);
            assert_eq!(path, b"/tmp/meiksh-procsub.4242.1");
            // The seq counter advances on each call.
            let path2 = generate_fifo_path(&mut shell);
            assert_eq!(path2, b"/tmp/meiksh-procsub.4242.2");
        });
    }

    #[test]
    fn generate_fifo_path_honors_tmpdir_when_set() {
        assert_no_syscalls(|| {
            let mut shell = crate::shell::test_support::test_shell();
            let _ = shell.set_var(b"TMPDIR", b"/var/tmp");
            shell.pid = 13;
            shell.proc_sub_seq = 5;
            let path = generate_fifo_path(&mut shell);
            assert_eq!(path, b"/var/tmp/meiksh-procsub.13.5");
        });
    }

    #[test]
    fn generate_fifo_path_falls_back_when_tmpdir_empty() {
        // Empty TMPDIR is treated as unset (matches POSIX `: -`
        // expansion behavior with `${TMPDIR:-/tmp}`).
        assert_no_syscalls(|| {
            let mut shell = crate::shell::test_support::test_shell();
            let _ = shell.set_var(b"TMPDIR", b"");
            shell.pid = 7;
            let path = generate_fifo_path(&mut shell);
            assert_eq!(path, b"/tmp/meiksh-procsub.7.1");
        });
    }
}
