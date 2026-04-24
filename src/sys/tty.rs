use libc::c_int;

use super::constants::TCSANOW;
use super::error::SysResult;
use super::interface::{self, last_error};
use super::types::Pid;

pub(crate) fn is_interactive_fd(fd: c_int) -> bool {
    interface::isatty(fd) == 1
}

/// Basename of the controlling terminal for `fd`, if known.
/// Used by `\l` in prompt expansion.
pub(crate) fn tty_basename(fd: c_int) -> Option<Vec<u8>> {
    let full = interface::ttyname_of_fd(fd)?;
    let slash = full.iter().rposition(|b| *b == b'/').map(|i| i + 1);
    match slash {
        Some(i) if i <= full.len() => Some(full[i..].to_vec()),
        _ => Some(full),
    }
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
    let result = interface::tcsetattr(fd, TCSANOW, termios);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}
pub(crate) fn isatty_fd(fd: c_int) -> bool {
    interface::isatty(fd) != 0
}

/// Best-effort query for the connected terminal's column count via
/// `TIOCGWINSZ`. Tries `stdout`, `stderr`, and `stdin` in turn; returns
/// `None` if the call fails or reports a zero-width window (e.g. under
/// a non-terminal fd). This is a display-only helper used by TAB
/// completion listings and is intentionally side-channel: it does not
/// go through the sys trace/mocking interface because it is never on
/// the hot path and has no observable behavior when stdout is not a
/// terminal.
pub(crate) fn terminal_columns_from_stdio() -> Option<usize> {
    use super::constants::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};

    #[repr(C)]
    struct Winsize {
        ws_row: libc::c_ushort,
        ws_col: libc::c_ushort,
        ws_xpixel: libc::c_ushort,
        ws_ypixel: libc::c_ushort,
    }
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    for fd in [STDOUT_FILENO, STDERR_FILENO, STDIN_FILENO] {
        let rc = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };
        if rc == 0 && ws.ws_col > 0 {
            return Some(ws.ws_col as usize);
        }
    }
    None
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
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(TCSANOW as i64)],
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
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(TCSANOW as i64)],
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
