use libc::c_int;

use super::constants::{F_DUPFD_CLOEXEC, F_GETFL, F_SETFL, O_NONBLOCK, S_IFIFO, S_IFMT};
use super::error::{SysError, SysResult};
use super::interface::{last_error, sys_interface};
use super::tty::is_interactive_fd;
use super::types::FdReader;

pub fn create_pipe() -> SysResult<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = (sys_interface().pipe)(&mut fds);
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd(oldfd: c_int, newfd: c_int) -> SysResult<()> {
    let result = (sys_interface().dup2)(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd_to_new(fd: c_int) -> SysResult<c_int> {
    let result = (sys_interface().fcntl)(fd, F_DUPFD_CLOEXEC, 10);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn close_fd(fd: c_int) -> SysResult<()> {
    let result = (sys_interface().close)(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fd_status_flags(fd: c_int) -> SysResult<c_int> {
    let result = (sys_interface().fcntl)(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> SysResult<()> {
    let result = (sys_interface().fcntl)(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fifo_like_fd(fd: c_int) -> bool {
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (sys_interface().fstat)(fd, buf.as_mut_ptr());
    if result != 0 {
        return false;
    }
    let buf = unsafe { buf.assume_init() };
    (buf.st_mode & S_IFMT) == S_IFIFO
}

pub fn ensure_blocking_read_fd(fd: c_int) -> SysResult<()> {
    if !is_interactive_fd(fd) && !fifo_like_fd(fd) {
        return Ok(());
    }
    let flags = fd_status_flags(fd)?;
    if flags & O_NONBLOCK != 0 {
        set_fd_status_flags(fd, flags & !O_NONBLOCK)?;
    }
    Ok(())
}

pub fn read_fd(fd: c_int, buf: &mut [u8]) -> SysResult<usize> {
    let result = (sys_interface().read)(fd, buf);
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}
pub fn write_fd(fd: c_int, data: &[u8]) -> SysResult<usize> {
    let result = (sys_interface().write)(fd, data);
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}

pub fn write_all_fd(fd: c_int, mut data: &[u8]) -> SysResult<()> {
    while !data.is_empty() {
        let n = write_fd(fd, data)?;
        if n == 0 {
            return Err(SysError::Errno(libc::EIO));
        }
        data = &data[n..];
    }
    Ok(())
}

impl FdReader {
    pub fn read(&mut self, buf: &mut [u8]) -> SysResult<usize> {
        read_fd(self.fd, buf)
    }
}

#[cfg(test)]
mod tests {
    use libc::{c_char, c_int, c_long, mode_t};
    use std::collections::HashMap;
    use std::ffi::CString;

    use crate::sys::test_support;
    use crate::sys::types::ClockTicks;

    use super::*;
    use crate::sys::*;

    #[test]
    fn pipe_roundtrip() {
        fn fake_pipe(fds: &mut [c_int; 2]) -> c_int {
            fds[0] = 10;
            fds[1] = 11;
            0
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            close: fake_close,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            assert_eq!(read_fd, 10);
            assert_eq!(write_fd, 11);
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        fn fail_isatty(_fd: c_int) -> c_int {
            0
        }
        fn fail_dup2(_old: c_int, _new: c_int) -> c_int {
            -1
        }
        fn fail_close(_fd: c_int) -> c_int {
            -1
        }
        fn fail_tcgetpgrp(_fd: c_int) -> Pid {
            -1
        }
        fn fail_tcsetpgrp(_fd: c_int, _pgid: Pid) -> c_int {
            -1
        }
        fn fail_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            -1
        }

        let fake = SystemInterface {
            isatty: fail_isatty,
            dup2: fail_dup2,
            close: fail_close,
            tcgetpgrp: fail_tcgetpgrp,
            tcsetpgrp: fail_tcsetpgrp,
            setpgid: fail_setpgid,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(!is_interactive_fd(-1));
            assert!(duplicate_fd(-1, -1).is_err());
            assert!(close_fd(-1).is_err());
            assert!(current_foreground_pgrp(-1).is_err());
            assert!(set_foreground_pgrp(-1, 0).is_err());
            assert!(set_process_group(999_999, 999_999).is_err());
        });
    }

    #[test]
    fn success_pipe_and_fd() {
        fn fake_pipe(fds: &mut [c_int; 2]) -> c_int {
            fds[0] = 10;
            fds[1] = 11;
            0
        }
        fn fake_fcntl(fd: c_int, cmd: c_int, _arg: c_int) -> c_int {
            if cmd == F_DUPFD_CLOEXEC { fd + 100 } else { -1 }
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int {
            oldfd
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            fcntl: fake_fcntl,
            dup2: fake_dup2,
            close: fake_close,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert_eq!(create_pipe().expect("pipe"), (10, 11));
            assert_eq!(duplicate_fd_to_new(4).expect("dup"), 104);
            assert!(duplicate_fd(4, 5).is_ok());
            assert!(close_fd(4).is_ok());
        });
    }

    #[test]
    fn error_pipe_and_fd() {
        fn fake_pipe(_fds: &mut [c_int; 2]) -> c_int {
            -1
        }
        fn fake_fcntl(_fd: c_int, _cmd: c_int, _arg: c_int) -> c_int {
            -1
        }
        fn fake_dup2(_oldfd: c_int, _newfd: c_int) -> c_int {
            -1
        }
        fn fake_close(_fd: c_int) -> c_int {
            -1
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            fcntl: fake_fcntl,
            dup2: fake_dup2,
            close: fake_close,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(create_pipe().is_err());
            assert!(duplicate_fd_to_new(1).is_err());
            assert!(duplicate_fd(1, 2).is_err());
            assert!(close_fd(1).is_err());
        });
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_tty() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int((O_NONBLOCK | 0o2) as i64),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_SETFL as i64),
                        ArgMatcher::Int(0o2),
                    ],
                    TraceResult::Int(0),
                ),
            ],
            || {
                ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_fifo() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(42)], TraceResult::Int(0)),
                t(
                    "fstat",
                    vec![ArgMatcher::Fd(42), ArgMatcher::Any],
                    TraceResult::StatFifo,
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int((O_NONBLOCK | 0o2) as i64),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(F_SETFL as i64),
                        ArgMatcher::Int(0o2),
                    ],
                    TraceResult::Int(0),
                ),
            ],
            || {
                ensure_blocking_read_fd(42).expect("fifo blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_surfaces_fcntl_errors() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(libc::EIO),
                ),
            ],
            || {
                assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
            },
        );
    }

    #[test]
    fn fifo_like_fd_fstat_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(99)], TraceResult::Int(0)),
                t(
                    "fstat",
                    vec![ArgMatcher::Fd(99), ArgMatcher::Any],
                    TraceResult::Err(libc::EBADF),
                ),
            ],
            || {
                ensure_blocking_read_fd(99).expect("regular fd no-op");
            },
        );
    }

    #[test]
    fn write_all_fd_zero_write_eio() {
        fn fake_write(_fd: c_int, _data: &[u8]) -> isize {
            0
        }
        let fake = SystemInterface {
            write: fake_write,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let err = write_all_fd(1, b"data").unwrap_err();
            assert_eq!(err, SysError::Errno(libc::EIO));
        });
    }
}
