use super::*;

pub(super) fn kill(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Ok(diag_status(
            shell,
            2,
            b"kill: usage: kill [-s sigspec | -signum] pid... | -l [exit_status]",
        ));
    }

    let mut args = &argv[1..];
    if args[0] == b"-l" || args[0] == b"-L" {
        if args.len() == 1 {
            let names: Vec<&[u8]> = sys::all_signal_names()
                .iter()
                .map(|(name, _)| {
                    let n = *name;
                    if n.starts_with(b"SIG") { &n[3..] } else { n }
                })
                .collect();
            let line = bstr::join_bytes(&names, b' ');
            write_stdout_line(&line);
            return Ok(BuiltinOutcome::Status(0));
        }
        for arg in &args[1..] {
            if let Some(code) = parse_i32(arg) {
                let sig = if code > 128 { code - 128 } else { code };
                let name = sys::signal_name(sig);
                if name != b"UNKNOWN" {
                    write_stdout_line(&name[3..]);
                } else {
                    let msg = ByteWriter::new()
                        .bytes(b"kill: unknown signal: ")
                        .bytes(arg)
                        .finish();
                    return Ok(diag_status(shell, 1, &msg));
                }
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"kill: invalid exit status: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let mut signal = sys::SIGTERM;
    if args[0] == b"-s" {
        if args.len() < 3 {
            return Ok(diag_status(shell, 2, b"kill: -s requires a signal name"));
        }
        signal = parse_kill_signal(shell, &args[1])?;
        args = &args[2..];
    } else if args[0].first() == Some(&b'-') && args[0] != b"--" {
        let spec = &args[0][1..];
        signal = parse_kill_signal(shell, spec)?;
        args = &args[1..];
    }

    if args.is_empty() || (args.len() == 1 && args[0] == b"--") {
        return Ok(diag_status(shell, 2, b"kill: no process id specified"));
    }
    if args[0] == b"--" {
        args = &args[1..];
    }

    let mut status = 0;
    for operand in args {
        if operand.first() == Some(&b'%') {
            let resolved_id = resolve_job_id(shell, Some(operand));
            let resolved = resolved_id.and_then(|id| shell.jobs.iter().find(|j| j.id == id));
            if let Some(job) = resolved {
                let pid = job
                    .pgid
                    .unwrap_or_else(|| job.children.first().map(|c| c.pid).unwrap_or(0));
                if pid != 0 {
                    if sys::send_signal(-pid, signal).is_err() {
                        let msg = ByteWriter::new()
                            .bytes(b"kill: (")
                            .i64_val(pid as i64)
                            .bytes(b"): No such process")
                            .finish();
                        shell.diagnostic(1, &msg);
                        status = 1;
                    } else if signal == sys::SIGCONT && resolved_id.is_some() {
                        let id = resolved_id.unwrap();
                        if let Some(j) = shell.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = crate::shell::JobState::Running;
                        }
                    }
                }
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"kill: ")
                    .bytes(operand)
                    .bytes(b": no such job")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        } else if let Some(pid) = bstr::parse_i64(operand).map(|v| v as sys::Pid) {
            let effective_target = if pid > 0 {
                shell
                    .jobs
                    .iter()
                    .find(|j| j.children.iter().any(|c| c.pid == pid) || j.last_pid == Some(pid))
                    .and_then(|j| j.pgid)
                    .map(|pgid| -pgid)
                    .unwrap_or(pid)
            } else {
                pid
            };
            if sys::send_signal(effective_target, signal).is_err() {
                let msg = ByteWriter::new()
                    .bytes(b"kill: (")
                    .i64_val(pid as i64)
                    .bytes(b"): No such process")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        } else {
            let msg = ByteWriter::new()
                .bytes(b"kill: invalid pid: ")
                .bytes(operand)
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn parse_kill_signal(shell: &Shell, spec: &[u8]) -> Result<i32, ShellError> {
    if let Some(num) = bstr::parse_i64(spec) {
        return Ok(num as i32);
    }
    let upper = spec.to_ascii_uppercase();
    let name = if upper.starts_with(b"SIG") {
        &upper[3..]
    } else {
        &upper
    };
    for (n, sig) in sys::all_signal_names() {
        let cmp_name = if n.starts_with(b"SIG") { &n[3..] } else { *n };
        if cmp_name == name {
            return Ok(*sig);
        }
    }
    if name == b"0" {
        return Ok(0);
    }
    let msg = ByteWriter::new()
        .bytes(b"kill: unknown signal: ")
        .bytes(spec)
        .finish();
    Err(shell.diagnostic(1, &msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;

    #[test]
    fn kill_no_args() {
        let msg = diag(b"kill: usage: kill [-s sigspec | -signum] pid... | -l [exit_status]");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"kill".to_vec()]).expect("kill");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn kill_dash_l_lists_signals() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes({
                        let names: Vec<&[u8]> = sys::all_signal_names()
                            .iter()
                            .map(|(n, _)| {
                                let n = *n;
                                if n.starts_with(b"SIG") { &n[3..] } else { n }
                            })
                            .collect();
                        let mut line = bstr::join_bytes(&names, b' ');
                        line.push(b'\n');
                        line
                    }),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"kill".to_vec(), b"-l".to_vec()]).expect("kill -l");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_dash_l_exit_code() {
        let sig_name = sys::signal_name(9);
        let mut expected = sig_name[3..].to_vec();
        expected.push(b'\n');
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(expected)],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"kill".to_vec(), b"-l".to_vec(), b"9".to_vec()],
                )
                .expect("kill -l 9");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_dash_l_exit_code_above_128() {
        let sig_name = sys::signal_name(9);
        let mut expected = sig_name[3..].to_vec();
        expected.push(b'\n');
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(expected)],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"kill".to_vec(), b"-l".to_vec(), b"137".to_vec()],
                )
                .expect("kill -l 137");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_dash_l_unknown_signal() {
        let msg = diag(b"kill: unknown signal: 999");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[b"kill".to_vec(), b"-l".to_vec(), b"999".to_vec()],
            )
            .expect("kill -l 999");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn kill_dash_l_invalid_exit_status() {
        let msg = diag(b"kill: invalid exit status: abc");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[b"kill".to_vec(), b"-l".to_vec(), b"abc".to_vec()],
            )
            .expect("kill -l abc");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn kill_dash_s_no_signal() {
        let msg = diag(b"kill: -s requires a signal name");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"kill".to_vec(), b"-s".to_vec()]).expect("kill -s");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn kill_no_pid_after_signal() {
        let msg = diag(b"kill: no process id specified");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[b"kill".to_vec(), b"-9".to_vec(), b"--".to_vec()],
            )
            .expect("kill -9 --");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn kill_invalid_pid() {
        let msg = diag(b"kill: invalid pid: abc");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome =
                invoke(&mut shell, &[b"kill".to_vec(), b"abc".to_vec()]).expect("kill abc");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn kill_job_not_found() {
        let msg = diag(b"kill: %99: no such job");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome =
                invoke(&mut shell, &[b"kill".to_vec(), b"%99".to_vec()]).expect("kill %99");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn parse_kill_signal_numeric() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert_eq!(parse_kill_signal(&shell, b"9").unwrap(), 9);
        });
    }

    #[test]
    fn parse_kill_signal_sig_prefix() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert_eq!(parse_kill_signal(&shell, b"SIGTERM").unwrap(), sys::SIGTERM);
        });
    }

    #[test]
    fn parse_kill_signal_unknown() {
        let msg = diag(b"kill: unknown signal: NOSUCHSIG");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let shell = test_shell();
            assert!(parse_kill_signal(&shell, b"NOSUCHSIG").is_err());
        });
    }

    #[test]
    fn parse_kill_signal_zero() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert_eq!(parse_kill_signal(&shell, b"0").unwrap(), 0);
        });
    }
}
