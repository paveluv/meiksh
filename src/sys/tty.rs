use libc::c_int;

use super::constants::TCSADRAIN;
use super::error::SysResult;
use super::interface::{self, last_error};
use super::types::Pid;

pub(crate) fn is_interactive_fd(fd: c_int) -> bool {
    interface::isatty(fd) == 1
}
pub(crate) fn current_foreground_pgrp(fd: c_int) -> SysResult<Pid> {
    let result = interface::tcgetpgrp(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub(crate) fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> SysResult<()> {
    let result = interface::tcsetpgrp(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub(crate) fn set_process_group(pid: Pid, pgid: Pid) -> SysResult<()> {
    let result = interface::setpgid(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub(crate) fn get_terminal_attrs(fd: c_int) -> SysResult<libc::termios> {
    let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
    let result = interface::tcgetattr(fd, &mut termios);
    if result == 0 {
        Ok(termios)
    } else {
        Err(last_error())
    }
}

pub(crate) fn set_terminal_attrs(fd: c_int, termios: &libc::termios) -> SysResult<()> {
    let result = interface::tcsetattr(fd, TCSADRAIN, termios);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}
pub(crate) fn isatty_fd(fd: c_int) -> bool {
    interface::isatty(fd) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    use crate::trace_entries;

    #[test]
    fn success_terminal_control() {
        run_trace(
            trace_entries![
                isatty(fd(0)) -> 1,
                tcgetpgrp(fd(0)) -> pid(77),
                tcsetpgrp(fd(0), 77) -> 0,
                setpgid(1, 1) -> 0,
            ],
            || {
                assert!(is_interactive_fd(0));
                assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
                assert!(set_foreground_pgrp(0, 77).is_ok());
                assert!(set_process_group(1, 1).is_ok());
            },
        );
    }

    #[test]
    fn error_terminal_control() {
        run_trace(
            trace_entries![
                isatty(fd(0)) -> 0,
                tcgetpgrp(fd(0)) -> err(libc::EIO),
                tcsetpgrp(fd(0), 1) -> err(libc::EIO),
                setpgid(1, 1) -> err(libc::EIO),
            ],
            || {
                assert!(!is_interactive_fd(0));
                assert!(current_foreground_pgrp(0).is_err());
                assert!(set_foreground_pgrp(0, 1).is_err());
                assert!(set_process_group(1, 1).is_err());
            },
        );
    }

    #[test]
    fn set_terminal_attrs_success_and_error() {
        let termios = unsafe { std::mem::zeroed::<libc::termios>() };

        run_trace(
            trace_entries![
                ..vec![t(
                    "tcsetattr",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(TCSADRAIN as i64)],
                    TraceResult::Int(0),
                )]
            ],
            || {
                assert!(set_terminal_attrs(0, &termios).is_ok());
            },
        );

        run_trace(
            trace_entries![
                ..vec![t(
                    "tcsetattr",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(TCSADRAIN as i64)],
                    TraceResult::Err(libc::EIO),
                )]
            ],
            || {
                assert!(set_terminal_attrs(0, &termios).is_err());
            },
        );
    }

    #[test]
    fn get_terminal_attrs_error() {
        run_trace(
            trace_entries![
                ..vec![t(
                    "tcgetattr",
                    vec![ArgMatcher::Fd(0)],
                    TraceResult::Err(libc::EIO),
                )]
            ],
            || {
                assert!(get_terminal_attrs(0).is_err());
            },
        );
    }

    #[test]
    fn isatty_fd_delegates_to_interface() {
        run_trace(trace_entries![isatty(fd(0)) -> 1], || {
            assert!(isatty_fd(0));
        });
        run_trace(trace_entries![isatty(fd(0)) -> 0], || {
            assert!(!isatty_fd(0));
        });
    }
}
