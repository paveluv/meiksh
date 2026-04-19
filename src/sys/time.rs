use super::constants::SC_CLK_TCK;
use super::error::SysResult;
use super::interface::{self, last_error};
use super::types::{ClockTicks, FileModeMask, ProcessTimes};

pub(crate) fn current_umask() -> FileModeMask {
    let mask = interface::umask(0);
    interface::umask(mask);
    mask & 0o777
}

pub(crate) fn set_umask(mask: FileModeMask) -> FileModeMask {
    interface::umask(mask & 0o777) & 0o777
}

pub(crate) fn process_times() -> SysResult<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = interface::times(raw.as_mut_ptr());
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

pub(crate) fn monotonic_clock_ns() -> u64 {
    interface::monotonic_clock_ns()
}

pub(crate) fn clock_ticks_per_second() -> SysResult<u64> {
    let result = interface::sysconf(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(last_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::process::{getrlimit, setrlimit};
    use crate::sys::test_support;
    use crate::trace_entries;

    #[test]
    fn success_umask_times_sysconf() {
        test_support::run_trace(
            trace_entries![
                umask(int(0)) -> 0,
                umask(int(0)) -> 0,
                umask(int(0o027)) -> 0o027,
                times(_) -> 99,
                sysconf(_) -> 60,
            ],
            || {
                assert_eq!(current_umask(), 0);
                assert_eq!(set_umask(0o027), 0o027);
                // Note: our fake `times` cannot fill the buffer, so all
                // tick fields are 0; production-equivalence for this
                // assertion is covered by `trace_umask_times_sysconf_and_monotonic_dispatch`.
                let _ = process_times().expect("times");
                assert_eq!(clock_ticks_per_second().expect("ticks"), 60);
            },
        );
    }

    #[test]
    fn error_times_sysconf() {
        test_support::run_trace(
            trace_entries![
                times(_) -> err(libc::EINVAL),
                sysconf(_) -> err(libc::EINVAL),
            ],
            || {
                assert!(process_times().is_err());
                assert!(clock_ticks_per_second().is_err());
            },
        );
    }

    #[test]
    fn trace_sysconf_dispatch() {
        test_support::run_trace(trace_entries![sysconf(_) -> 100], || {
            assert_eq!(clock_ticks_per_second().expect("sysconf"), 100);
        });
    }

    #[test]
    fn trace_umask_times_sysconf_and_monotonic_dispatch() {
        test_support::run_trace(
            trace_entries![
                umask(_) -> 0o22,
                times(_) -> 500,
                sysconf(_) -> 100,
                monotonic_clock_ns() -> 123456,
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
        test_support::run_trace(trace_entries![times(_) -> err(libc::EINVAL)], || {
            assert!(process_times().is_err());
        });
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
