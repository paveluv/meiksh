use libc::c_int;

use super::constants::TCSADRAIN;
use super::error::SysResult;
use super::interface::{last_error, sys_interface};
use super::types::Pid;

pub fn is_interactive_fd(fd: c_int) -> bool {
    (sys_interface().isatty)(fd) == 1
}
pub fn current_foreground_pgrp(fd: c_int) -> SysResult<Pid> {
    let result = (sys_interface().tcgetpgrp)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> SysResult<()> {
    let result = (sys_interface().tcsetpgrp)(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn set_process_group(pid: Pid, pgid: Pid) -> SysResult<()> {
    let result = (sys_interface().setpgid)(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn get_terminal_attrs(fd: c_int) -> SysResult<libc::termios> {
    let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
    let result = (sys_interface().tcgetattr)(fd, &mut termios);
    if result == 0 {
        Ok(termios)
    } else {
        Err(last_error())
    }
}

pub fn set_terminal_attrs(fd: c_int, termios: &libc::termios) -> SysResult<()> {
    let result = (sys_interface().tcsetattr)(fd, TCSADRAIN, termios);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}
pub fn isatty_fd(fd: c_int) -> bool {
    (sys_interface().isatty)(fd) != 0
}

#[cfg(test)]
mod tests {
    use libc::c_int;

    use crate::sys::test_support;

    use super::*;
    use crate::sys::*;

    #[test]
    fn success_terminal_control() {
        fn fake_isatty(_fd: c_int) -> c_int {
            1
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            77
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            0
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            0
        }

        let fake = SystemInterface {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(is_interactive_fd(0));
            assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
            assert!(set_foreground_pgrp(0, 77).is_ok());
            assert!(set_process_group(1, 1).is_ok());
        });
    }

    #[test]
    fn error_terminal_control() {
        fn fake_isatty(_fd: c_int) -> c_int {
            0
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            -1
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            -1
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            -1
        }

        let fake = SystemInterface {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(!is_interactive_fd(0));
            assert!(current_foreground_pgrp(0).is_err());
            assert!(set_foreground_pgrp(0, 1).is_err());
            assert!(set_process_group(1, 1).is_err());
        });
    }

    #[test]
    fn set_terminal_attrs_success_and_error() {
        let termios = unsafe { std::mem::zeroed::<libc::termios>() };

        fn fake_tcsetattr_ok(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
            0
        }
        let fake_ok = SystemInterface {
            tcsetattr: fake_tcsetattr_ok,
            ..default_interface()
        };
        test_support::with_test_interface(fake_ok, || {
            assert!(set_terminal_attrs(0, &termios).is_ok());
        });

        fn fake_tcsetattr_err(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
            -1
        }
        let fake_err = SystemInterface {
            tcsetattr: fake_tcsetattr_err,
            ..default_interface()
        };
        test_support::with_test_interface(fake_err, || {
            assert!(set_terminal_attrs(0, &termios).is_err());
        });
    }

    #[test]
    fn get_terminal_attrs_error() {
        fn fake_tcgetattr_err(_: c_int, _: *mut libc::termios) -> c_int {
            -1
        }
        let fake = SystemInterface {
            tcgetattr: fake_tcgetattr_err,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(get_terminal_attrs(0).is_err());
        });
    }

    #[test]
    fn isatty_fd_delegates_to_interface() {
        fn fake_isatty_yes(_fd: c_int) -> c_int {
            1
        }
        fn fake_isatty_no(_fd: c_int) -> c_int {
            0
        }

        let fake_yes = SystemInterface {
            isatty: fake_isatty_yes,
            ..default_interface()
        };
        test_support::with_test_interface(fake_yes, || {
            assert!(isatty_fd(0));
        });

        let fake_no = SystemInterface {
            isatty: fake_isatty_no,
            ..default_interface()
        };
        test_support::with_test_interface(fake_no, || {
            assert!(!isatty_fd(0));
        });
    }
}
