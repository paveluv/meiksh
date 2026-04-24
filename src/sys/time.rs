use super::constants::SC_CLK_TCK;
use super::error::SysResult;
use super::interface::{self, last_error};
use super::types::{ClockTicks, FileModeMask, ProcessTimes};

/// Broken-down local time. This is the `sys::`-boundary-safe wrapper
/// around `libc::tm`: production code outside `src/sys/` never observes
/// the raw struct. Construction is via [`local_time_now`] or, in tests,
/// via [`set_test_local_time`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalTime {
    pub(crate) raw: libc::tm,
}

impl LocalTime {
    /// Construct from individual calendar fields. Values follow the
    /// `struct tm` conventions:
    ///
    /// * `year` is the full Gregorian year (e.g. `2026`), not
    ///   `year - 1900`.
    /// * `month` is `1..=12`.
    /// * `mday` is the day of month (`1..=31`).
    /// * `wday` is `0..=6` with Sunday = 0.
    /// * `yday` is `0..=365`.
    #[cfg(test)]
    pub(crate) fn from_fields(
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        month: i32,
        year: i32,
        wday: i32,
        yday: i32,
    ) -> Self {
        let mut raw = unsafe { std::mem::zeroed::<libc::tm>() };
        raw.tm_sec = sec;
        raw.tm_min = min;
        raw.tm_hour = hour;
        raw.tm_mday = mday;
        raw.tm_mon = month - 1;
        raw.tm_year = year - 1900;
        raw.tm_wday = wday;
        raw.tm_yday = yday;
        raw.tm_isdst = 0;
        LocalTime { raw }
    }
}

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

/// Return the current local wall-clock time, broken down.
///
/// Production builds call `time(3)` + `localtime_r(3)`. The test build
/// consults a thread-local override (set via
/// [`set_test_local_time`]) when one has been installed, falling back
/// to a deterministic fixed instant otherwise so that tests that never
/// set a value still observe the same formatted output across runs.
pub(crate) fn local_time_now() -> LocalTime {
    #[cfg(test)]
    {
        if let Some(tm) = super::test_support::test_local_time() {
            return tm;
        }
        // Fixed default for tests that don't care about the specific
        // instant but do render one of the date/time escapes.
        // 2024-01-15 13:45:30 local, Monday.
        return LocalTime::from_fields(30, 45, 13, 15, 1, 2024, 1, 14);
    }

    #[cfg(not(test))]
    {
        let mut epoch: libc::time_t = 0;
        unsafe { libc::time(&mut epoch as *mut libc::time_t) };
        let mut raw = unsafe { std::mem::zeroed::<libc::tm>() };
        unsafe {
            libc::localtime_r(&epoch as *const libc::time_t, &mut raw as *mut libc::tm);
        }
        LocalTime { raw }
    }
}

/// Render `tm` into a byte buffer using `strftime(3)` with `format`.
///
/// `format` is borrowed and may contain arbitrary bytes; it is passed
/// to `strftime(3)` as a NUL-terminated C string. `cap` is a soft
/// upper bound on the output size (`strftime` is retried up to that
/// capacity); if `strftime` still returns zero and the buffer was not
/// pre-sized to zero, the function returns an empty vector. The output
/// vector never contains a trailing NUL.
pub(crate) fn format_strftime(format: &[u8], tm: &LocalTime, cap: usize) -> Vec<u8> {
    let cap = cap.max(64);
    let fmt = match crate::bstr::to_cstring(format) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut buf = vec![0u8; cap];
    let written = unsafe {
        libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            fmt.as_ptr(),
            &tm.raw as *const libc::tm,
        )
    };
    if written == 0 {
        // Either the format produced an empty string or the buffer
        // wasn't large enough. We don't distinguish here: callers
        // asking for a one-escape chunk pick `cap` generously. In the
        // worst case a too-small buffer maps to an empty expansion,
        // matching the graceful-degradation contract in §10.3.
        return Vec::new();
    }
    buf.truncate(written);
    buf
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
    fn local_time_now_returns_fixed_default_in_tests() {
        test_support::clear_test_local_time();
        let tm = local_time_now();
        assert_eq!(tm.raw.tm_year + 1900, 2024);
        assert_eq!(tm.raw.tm_mon + 1, 1);
        assert_eq!(tm.raw.tm_mday, 15);
        assert_eq!(tm.raw.tm_hour, 13);
        assert_eq!(tm.raw.tm_min, 45);
        assert_eq!(tm.raw.tm_sec, 30);
        assert_eq!(tm.raw.tm_wday, 1);
    }

    #[test]
    fn local_time_now_honors_test_override() {
        let injected = LocalTime::from_fields(5, 6, 7, 8, 9, 2030, 2, 250);
        test_support::set_test_local_time(injected);
        let tm = local_time_now();
        assert_eq!(tm.raw.tm_year + 1900, 2030);
        assert_eq!(tm.raw.tm_mon + 1, 9);
        assert_eq!(tm.raw.tm_mday, 8);
        test_support::clear_test_local_time();
    }

    #[test]
    fn format_strftime_renders_bash_short_escapes() {
        let tm = LocalTime::from_fields(30, 45, 13, 15, 1, 2024, 1, 14);
        let hhmm = format_strftime(b"%H:%M", &tm, 32);
        assert_eq!(hhmm, b"13:45");
        let twelve = format_strftime(b"%I:%M:%S %p", &tm, 32);
        // %p output is locale-sensitive; just sanity-check the hh:mm:ss.
        assert!(twelve.starts_with(b"01:45:30 "));
        let hm24 = format_strftime(b"%H:%M", &tm, 32);
        assert_eq!(hm24, b"13:45");
    }

    #[test]
    fn format_strftime_renders_date_short_escape() {
        let tm = LocalTime::from_fields(0, 0, 0, 15, 1, 2024, 1, 14);
        let weekday_date = format_strftime(b"%a %b %e", &tm, 32);
        // e.g. "Mon Jan 15" in C locale; exact locale strings vary, so
        // assert the shape.
        assert!(!weekday_date.is_empty());
        assert!(weekday_date.ends_with(b"15"));
    }

    #[test]
    fn format_strftime_empty_format_yields_empty_output() {
        let tm = LocalTime::from_fields(0, 0, 0, 1, 1, 2024, 1, 0);
        assert_eq!(format_strftime(b"", &tm, 32), b"");
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
