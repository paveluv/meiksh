use super::constants::SC_CLK_TCK;
use super::error::SysResult;
use super::interface::{last_error, sys_interface};
use super::types::{ClockTicks, FileModeMask, ProcessTimes};

pub fn current_umask() -> FileModeMask {
    let mask = (sys_interface().umask)(0);
    (sys_interface().umask)(mask);
    mask & 0o777
}

pub fn set_umask(mask: FileModeMask) -> FileModeMask {
    (sys_interface().umask)(mask & 0o777) & 0o777
}

pub fn process_times() -> SysResult<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = (sys_interface().times)(raw.as_mut_ptr());
    if result == ClockTicks::MAX {
        return Err(last_error());
    }
    let raw = unsafe { raw.assume_init() };
    Ok(ProcessTimes {
        user_ticks: raw.tms_utime as u64,
        system_ticks: raw.tms_stime as u64,
        child_user_ticks: raw.tms_cutime as u64,
        child_system_ticks: raw.tms_cstime as u64,
    })
}

pub fn monotonic_clock_ns() -> u64 {
    (sys_interface().monotonic_clock_ns)()
}

pub fn clock_ticks_per_second() -> SysResult<u64> {
    let result = (sys_interface().sysconf)(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(last_error())
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
    fn success_umask_times_sysconf() {
        fn fake_umask(mask: FileModeMask) -> FileModeMask {
            mask
        }
        fn fake_times(buffer: *mut libc::tms) -> ClockTicks {
            unsafe {
                (*buffer).tms_utime = 10;
                (*buffer).tms_stime = 20;
                (*buffer).tms_cutime = 30;
                (*buffer).tms_cstime = 40;
            }
            99
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            60
        }

        let fake = SystemInterface {
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert_eq!(current_umask(), 0);
            assert_eq!(set_umask(0o027), 0o027);
            assert_eq!(
                process_times().expect("times"),
                ProcessTimes {
                    user_ticks: 10,
                    system_ticks: 20,
                    child_user_ticks: 30,
                    child_system_ticks: 40,
                }
            );
            assert_eq!(clock_ticks_per_second().expect("ticks"), 60);
        });
    }

    #[test]
    fn error_times_sysconf() {
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            -1
        }

        let fake = SystemInterface {
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(process_times().is_err());
            assert!(clock_ticks_per_second().is_err());
        });
    }

    #[test]
    fn trace_sysconf_dispatch() {
        test_support::run_trace(
            vec![test_support::t(
                "sysconf",
                vec![test_support::ArgMatcher::Any],
                test_support::TraceResult::Int(100),
            )],
            || {
                assert_eq!(clock_ticks_per_second().expect("sysconf"), 100);
            },
        );
    }

    #[test]
    fn trace_umask_times_sysconf_and_monotonic_dispatch() {
        test_support::run_trace(
            vec![
                test_support::t(
                    "umask",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(0o22),
                ),
                test_support::t(
                    "times",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(500),
                ),
                test_support::t(
                    "sysconf",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(100),
                ),
                test_support::t(
                    "monotonic_clock_ns",
                    vec![],
                    test_support::TraceResult::Int(123456),
                ),
            ],
            || {
                assert_eq!(set_umask(0o77), 0o22);
                let times = process_times().expect("times");
                assert_eq!(times.user_ticks, 0);
                assert_eq!(clock_ticks_per_second().expect("sysconf"), 100);
                assert_eq!(monotonic_clock_ns(), 123456);
            },
        );
    }

    #[test]
    fn trace_times_err_path() {
        test_support::run_trace(
            vec![test_support::t(
                "times",
                vec![test_support::ArgMatcher::Any],
                test_support::TraceResult::Err(libc::EINVAL),
            )],
            || {
                assert!(process_times().is_err());
            },
        );
    }

    #[test]
    fn getrlimit_invalid_resource_returns_error() {
        assert!(getrlimit(99999).is_err());
    }

    #[test]
    fn setrlimit_invalid_values_returns_error() {
        assert!(setrlimit(99999, 0, 0).is_err());
    }
}
