use libc::c_int;
use std::cell::Cell;

use super::constants::{
    F_DUPFD_CLOEXEC, F_GETFL, F_SETFL, O_NONBLOCK, S_IFCHR, S_IFIFO, S_IFMT, SEEK_CUR,
};
use super::error::{SysError, SysResult};
use super::interface::{self, last_error};
use super::tty::is_interactive_fd;
#[cfg(test)]
use super::types::FdReader;

thread_local! {
    /// One-entry "passthrough" cache for [`ensure_blocking_read_fd`].
    /// When the last successful probe confirmed that `fd` needed no
    /// blocking-flag change, a subsequent call with the same `fd`
    /// short-circuits without issuing any syscall. Invalidated
    /// whenever the kernel could plausibly reassign the fd number to
    /// a different kernel file (see `invalidate_passthrough_fd`).
    static PASSTHROUGH_FD: Cell<c_int> = const { Cell::new(-1) };
}

fn invalidate_passthrough_fd(fd: c_int) {
    PASSTHROUGH_FD.with(|c| {
        if c.get() == fd {
            c.set(-1);
        }
    });
}

#[cfg(test)]
pub(crate) fn reset_passthrough_fd_cache_for_test() {
    PASSTHROUGH_FD.with(|c| c.set(-1));
}

pub(crate) fn create_pipe() -> SysResult<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = interface::pipe(&mut fds);
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(last_error())
    }
}

pub(crate) fn duplicate_fd(oldfd: c_int, newfd: c_int) -> SysResult<()> {
    // `dup2` closes `newfd` (if open) and makes it refer to whatever
    // `oldfd` points at, so any cached "passthrough" state for
    // `newfd` is now stale.
    invalidate_passthrough_fd(newfd);
    let result = interface::dup2(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub(crate) fn duplicate_fd_to_new(fd: c_int) -> SysResult<c_int> {
    let result = interface::fcntl(fd, F_DUPFD_CLOEXEC, 10);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub(crate) fn close_fd(fd: c_int) -> SysResult<()> {
    // A closed fd can be reused by the kernel for a different file,
    // so any cached "passthrough" verdict is no longer trustworthy.
    invalidate_passthrough_fd(fd);
    let result = interface::close(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub(crate) fn clear_cloexec(fd: c_int) -> SysResult<()> {
    let result = interface::fcntl(fd, super::constants::F_SETFD, 0);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fd_status_flags(fd: c_int) -> SysResult<c_int> {
    let result = interface::fcntl(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> SysResult<()> {
    let result = interface::fcntl(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

/// Clears `O_NONBLOCK` on `fd` if it names a FIFO or terminal. Regular
/// files, block devices, sockets and other non-tty character devices
/// already block on `read()`, so probing them any further would just
/// burn syscalls. Hot path for `while read … ; do … ; done < file`:
/// every iteration reaches this function, so a one-entry thread-local
/// cache ([`PASSTHROUGH_FD`]) skips even the single `fstat` on repeat
/// calls with the same fd. The cache is invalidated whenever `close`
/// or `dup2` could make the fd point at a different kernel file.
pub(crate) fn ensure_blocking_read_fd(fd: c_int) -> SysResult<()> {
    if PASSTHROUGH_FD.with(|c| c.get()) == fd {
        return Ok(());
    }
    // One fstat replaces the pre-existing `isatty + fstat` pair for
    // the common redirected-regular-file case. `isatty` is only
    // needed when the kernel reports a character device, since other
    // char devices (e.g. `/dev/null`) block on `read` without any
    // flag tweak.
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let fstat_result = interface::fstat(fd, buf.as_mut_ptr());
    if fstat_result != 0 {
        // Preserve prior behaviour: an `fstat` failure is not fatal —
        // the fd might still work for `read`, and if it doesn't, the
        // read itself will surface the error.
        return Ok(());
    }
    let buf = unsafe { buf.assume_init() };
    let mode = buf.st_mode & S_IFMT;
    let needs_probe = mode == S_IFIFO || (mode == S_IFCHR && is_interactive_fd(fd));
    if !needs_probe {
        PASSTHROUGH_FD.with(|c| c.set(fd));
        return Ok(());
    }
    let flags = fd_status_flags(fd)?;
    if flags & O_NONBLOCK != 0 {
        set_fd_status_flags(fd, flags & !O_NONBLOCK)?;
    }
    Ok(())
}

pub(crate) fn read_fd(fd: c_int, buf: &mut [u8]) -> SysResult<usize> {
    let result = interface::read(fd, buf);
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}

/// Returns the current read/write offset of `fd` (as per `lseek(fd, 0,
/// SEEK_CUR)`). On pipes, FIFOs, sockets and terminals the kernel
/// surfaces `ESPIPE`; callers use that to detect a non-seekable fd and
/// fall back to unbuffered I/O.
pub(crate) fn fd_seek_cur(fd: c_int) -> SysResult<libc::off_t> {
    let result = interface::lseek(fd, 0, SEEK_CUR);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

/// Moves the read/write offset of `fd` backwards by `bytes`, relative
/// to the current position. Used to "un-read" buffered bytes past a
/// delimiter so that subsequent commands observe the same post-delimiter
/// fd position that the byte-at-a-time path would leave behind.
pub(crate) fn fd_seek_rewind(fd: c_int, bytes: usize) -> SysResult<()> {
    if bytes == 0 {
        return Ok(());
    }
    let delta = -(bytes as libc::off_t);
    let result = interface::lseek(fd, delta, SEEK_CUR);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}
pub(crate) fn write_fd(fd: c_int, data: &[u8]) -> SysResult<usize> {
    let result = interface::write(fd, data);
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}

pub(crate) fn write_all_fd(fd: c_int, mut data: &[u8]) -> SysResult<()> {
    while !data.is_empty() {
        let n = write_fd(fd, data)?;
        if n == 0 {
            return Err(SysError::Errno(libc::EIO));
        }
        data = &data[n..];
    }
    Ok(())
}

#[cfg(test)]
impl FdReader {
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> SysResult<usize> {
        read_fd(self.fd, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support::{self, run_trace};
    use crate::trace_entries;

    use super::super::constants::{
        F_DUPFD_CLOEXEC, F_GETFL, F_SETFD, F_SETFL, O_NONBLOCK, STDIN_FILENO,
    };
    use super::super::error::SysError;
    use super::super::tty::{
        current_foreground_pgrp, is_interactive_fd, set_foreground_pgrp, set_process_group,
    };

    #[test]
    fn pipe_roundtrip() {
        run_trace(
            trace_entries![
                pipe() -> fds(10, 11),
                close(fd(10)) -> 0,
                close(fd(11)) -> 0,
            ],
            || {
                let (read_fd, write_fd) = create_pipe().expect("pipe");
                assert_eq!(read_fd, 10);
                assert_eq!(write_fd, 11);
                close_fd(read_fd).expect("close read");
                close_fd(write_fd).expect("close write");
            },
        );
    }

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        run_trace(
            trace_entries![
                isatty(fd(-1)) -> 0,
                dup2(fd(-1), fd(-1)) -> err(libc::EBADF),
                close(fd(-1)) -> err(libc::EBADF),
                tcgetpgrp(fd(-1)) -> err(libc::EBADF),
                tcsetpgrp(fd(-1), 0) -> err(libc::EBADF),
                setpgid(999_999, 999_999) -> err(libc::ESRCH),
            ],
            || {
                assert!(!is_interactive_fd(-1));
                assert!(duplicate_fd(-1, -1).is_err());
                assert!(close_fd(-1).is_err());
                assert!(current_foreground_pgrp(-1).is_err());
                assert!(set_foreground_pgrp(-1, 0).is_err());
                assert!(set_process_group(999_999, 999_999).is_err());
            },
        );
    }

    #[test]
    fn success_pipe_and_fd() {
        run_trace(
            trace_entries![
                pipe() -> fds(10, 11),
                fcntl(fd(4), int(F_DUPFD_CLOEXEC), int(10)) -> int(104),
                dup2(fd(4), fd(5)) -> fd(4),
                close(fd(4)) -> 0,
            ],
            || {
                assert_eq!(create_pipe().expect("pipe"), (10, 11));
                assert_eq!(duplicate_fd_to_new(4).expect("dup"), 104);
                assert!(duplicate_fd(4, 5).is_ok());
                assert!(close_fd(4).is_ok());
            },
        );
    }

    #[test]
    fn error_pipe_and_fd() {
        run_trace(
            trace_entries![
                pipe() -> err(libc::EMFILE),
                fcntl(fd(1), int(F_DUPFD_CLOEXEC), int(10)) -> err(libc::EBADF),
                dup2(fd(1), fd(2)) -> err(libc::EBADF),
                close(fd(1)) -> err(libc::EBADF),
            ],
            || {
                assert!(create_pipe().is_err());
                assert!(duplicate_fd_to_new(1).is_err());
                assert!(duplicate_fd(1, 2).is_err());
                assert!(close_fd(1).is_err());
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_tty() {
        run_trace(
            trace_entries![
                fstat(fd(STDIN_FILENO), _) -> stat_char,
                isatty(fd(STDIN_FILENO)) -> int(1),
                fcntl(fd(STDIN_FILENO), int(F_GETFL), int(0)) -> int((O_NONBLOCK | 0o2)),
                fcntl(fd(STDIN_FILENO), int(F_SETFL), int(0o2)) -> int(0),
            ],
            || {
                ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_fifo() {
        run_trace(
            trace_entries![
                fstat(fd(42), _) -> stat_fifo,
                fcntl(fd(42), int(F_GETFL), int(0)) -> int((O_NONBLOCK | 0o2)),
                fcntl(fd(42), int(F_SETFL), int(0o2)) -> int(0),
            ],
            || {
                ensure_blocking_read_fd(42).expect("fifo blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_surfaces_fcntl_errors() {
        run_trace(
            trace_entries![
                fstat(fd(STDIN_FILENO), _) -> stat_char,
                isatty(fd(STDIN_FILENO)) -> int(1),
                fcntl(fd(STDIN_FILENO), int(F_GETFL), int(0)) -> err(libc::EIO),
            ],
            || {
                assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
            },
        );
    }

    #[test]
    fn fifo_like_fd_fstat_error() {
        run_trace(
            trace_entries![
                fstat(fd(99), _) -> err(libc::EBADF),
            ],
            || {
                ensure_blocking_read_fd(99).expect("regular fd no-op");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_caches_passthrough() {
        run_trace(
            trace_entries![
                fstat(fd(99), _) -> stat_file(libc::S_IFREG),
            ],
            || {
                ensure_blocking_read_fd(99).expect("regular fd first call");
                ensure_blocking_read_fd(99).expect("regular fd cached call");
                ensure_blocking_read_fd(99).expect("regular fd cached call");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_cache_invalidates_on_close() {
        run_trace(
            trace_entries![
                fstat(fd(99), _) -> stat_file(libc::S_IFREG),
                close(fd(99)) -> 0,
                fstat(fd(99), _) -> stat_file(libc::S_IFREG),
            ],
            || {
                ensure_blocking_read_fd(99).expect("first probe");
                close_fd(99).expect("close");
                ensure_blocking_read_fd(99).expect("second probe after close");
            },
        );
    }

    #[test]
    fn clear_cloexec_error() {
        run_trace(
            trace_entries![
                fcntl(fd(5), int(F_SETFD), int(0)) -> err(libc::EBADF),
            ],
            || {
                assert!(clear_cloexec(5).is_err());
            },
        );
    }

    #[test]
    fn clear_cloexec_success() {
        run_trace(
            trace_entries![
                fcntl(fd(5), int(F_SETFD), int(0)) -> int(0),
            ],
            || {
                assert!(clear_cloexec(5).is_ok());
            },
        );
    }

    #[test]
    fn write_all_fd_zero_write_eio() {
        run_trace(
            trace_entries![
                ..vec![test_support::t(
                    "write",
                    vec![
                        test_support::ArgMatcher::Fd(1),
                        test_support::ArgMatcher::Bytes(b"data".to_vec()),
                    ],
                    test_support::TraceResult::Int(0),
                )],
            ],
            || {
                let err = write_all_fd(1, b"data").unwrap_err();
                assert_eq!(err, SysError::Errno(libc::EIO));
            },
        );
    }

    #[test]
    fn fd_seek_rewind_zero_bytes_is_noop() {
        run_trace(trace_entries![], || {
            assert!(fd_seek_rewind(3, 0).is_ok());
        });
    }

    #[test]
    fn fd_seek_rewind_reports_lseek_error() {
        use super::super::constants::SEEK_CUR;
        use super::super::test_support::{ArgMatcher, TraceResult, t};
        run_trace(
            vec![t(
                "lseek",
                vec![
                    ArgMatcher::Fd(3),
                    ArgMatcher::Int(-5),
                    ArgMatcher::Int(SEEK_CUR as i64),
                ],
                TraceResult::Err(libc::ESPIPE),
            )],
            || {
                let err = fd_seek_rewind(3, 5).unwrap_err();
                assert_eq!(err, SysError::Errno(libc::ESPIPE));
            },
        );
    }
}
