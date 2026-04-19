use libc::c_int;

use super::constants::{F_DUPFD_CLOEXEC, F_GETFL, F_SETFL, O_NONBLOCK, S_IFIFO, S_IFMT};
use super::error::{SysError, SysResult};
use super::interface::{self, last_error};
use super::tty::is_interactive_fd;
#[cfg(test)]
use super::types::FdReader;

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

fn fifo_like_fd(fd: c_int) -> bool {
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = interface::fstat(fd, buf.as_mut_ptr());
    if result != 0 {
        return false;
    }
    let buf = unsafe { buf.assume_init() };
    (buf.st_mode & S_IFMT) == S_IFIFO
}

pub(crate) fn ensure_blocking_read_fd(fd: c_int) -> SysResult<()> {
    if !is_interactive_fd(fd) && !fifo_like_fd(fd) {
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
                isatty(fd(42)) -> int(0),
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
                isatty(fd(99)) -> int(0),
                fstat(fd(99), _) -> err(libc::EBADF),
            ],
            || {
                ensure_blocking_read_fd(99).expect("regular fd no-op");
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
}
