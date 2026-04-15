use super::*;

pub(super) fn times(shell: &Shell) -> BuiltinOutcome {
    match (sys::process_times(), sys::clock_ticks_per_second()) {
        (Ok(times), Ok(ticks_per_second)) => {
            let line1 = ByteWriter::new()
                .bytes(&format_times_value(times.user_ticks, ticks_per_second))
                .byte(b' ')
                .bytes(&format_times_value(times.system_ticks, ticks_per_second))
                .finish();
            write_stdout_line(&line1);
            let line2 = ByteWriter::new()
                .bytes(&format_times_value(
                    times.child_user_ticks,
                    ticks_per_second,
                ))
                .byte(b' ')
                .bytes(&format_times_value(
                    times.child_system_ticks,
                    ticks_per_second,
                ))
                .finish();
            write_stdout_line(&line2);
            BuiltinOutcome::Status(0)
        }
        (Err(error), _) | (_, Err(error)) => diag_status_syserr(shell, 1, b"times: ", &error),
    }
}

pub(super) fn format_times_value(ticks: u64, ticks_per_second: u64) -> Vec<u8> {
    let total_seconds = ticks as f64 / ticks_per_second as f64;
    let minutes = (total_seconds / 60.0).floor() as u64;
    let seconds = total_seconds - (minutes * 60) as f64;
    ByteWriter::new()
        .u64_val(minutes)
        .byte(b'm')
        .f64_fixed(seconds, 2)
        .byte(b's')
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn format_times_value_helper() {
        assert_no_syscalls(|| {
            assert_eq!(format_times_value(125, 100), b"0m1.25s");
        });
    }

    #[test]
    fn times_error_branch() {
        let msg = crate::builtin::test_support::diag(b"times: Success");
        run_trace(
            trace_entries![
                times(_) -> err(libc::EACCES),
                sysconf(_) -> 100,
                write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"times".to_vec()]).expect("times error");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}
