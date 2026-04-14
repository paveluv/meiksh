use super::*;

pub(super) fn jobs(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
    let (mode, index) = match parse_jobs_options(argv) {
        Ok(value) => value,
        Err(message) => return diag_status(shell, 1, &message),
    };
    let selected = match parse_jobs_operands(&argv[index..], shell) {
        Ok(value) => value,
        Err(message) => return diag_status(shell, 1, &message),
    };
    let finished = shell.reap_jobs();
    let current_id = shell.current_job_id();
    let previous_id = shell.previous_job_id();
    let selected_contains = |id: usize| selected.as_ref().map_or(true, |ids| ids.contains(&id));

    if mode != JobsMode::PidOnly {
        for (id, state) in &finished {
            if !selected_contains(*id) {
                continue;
            }
            let marker = job_current_marker(*id, current_id, previous_id);
            match state {
                crate::shell::ReapedJobState::Done(status, cmd) => {
                    let state_bytes = if *status == 0 {
                        b"Done".to_vec()
                    } else {
                        ByteWriter::new()
                            .bytes(b"Done(")
                            .i32_val(*status)
                            .byte(b')')
                            .finish()
                    };
                    let line = ByteWriter::new()
                        .byte(b'[')
                        .usize_val(*id)
                        .bytes(b"] ")
                        .byte(marker)
                        .byte(b' ')
                        .bytes(&state_bytes)
                        .byte(b'\t')
                        .bytes(cmd)
                        .finish();
                    write_stdout_line(&line);
                }
                crate::shell::ReapedJobState::Signaled(sig, cmd) => {
                    let line = ByteWriter::new()
                        .byte(b'[')
                        .usize_val(*id)
                        .bytes(b"] ")
                        .byte(marker)
                        .bytes(b" Terminated (")
                        .bytes(sys::signal_name(*sig))
                        .bytes(b")\t")
                        .bytes(cmd)
                        .finish();
                    write_stdout_line(&line);
                }
                crate::shell::ReapedJobState::Stopped(..) => {}
            }
        }
    }

    for job in &shell.jobs {
        if !selected_contains(job.id) {
            continue;
        }
        match mode {
            JobsMode::PidOnly => {
                if let Some(pid) = job_display_pid(job) {
                    let line = bstr::i64_to_bytes(pid as i64);
                    write_stdout_line(&line);
                }
            }
            _ => {
                let marker = job_current_marker(job.id, current_id, previous_id);
                let (state_bytes, pid_bytes) = format_job_state(job);
                if mode == JobsMode::Long {
                    let line = ByteWriter::new()
                        .byte(b'[')
                        .usize_val(job.id)
                        .bytes(b"] ")
                        .byte(marker)
                        .byte(b' ')
                        .bytes(&pid_bytes)
                        .byte(b' ')
                        .bytes(&state_bytes)
                        .byte(b' ')
                        .bytes(&job.command)
                        .finish();
                    write_stdout_line(&line);
                } else {
                    let line = ByteWriter::new()
                        .byte(b'[')
                        .usize_val(job.id)
                        .bytes(b"] ")
                        .byte(marker)
                        .byte(b' ')
                        .bytes(&state_bytes)
                        .byte(b' ')
                        .bytes(&job.command)
                        .finish();
                    write_stdout_line(&line);
                }
            }
        }
    }
    BuiltinOutcome::Status(0)
}

pub(super) fn job_current_marker(id: usize, current: Option<usize>, previous: Option<usize>) -> u8 {
    if Some(id) == current {
        b'+'
    } else if Some(id) == previous {
        b'-'
    } else {
        b' '
    }
}

pub(super) fn format_job_state(job: &crate::shell::Job) -> (Vec<u8>, Vec<u8>) {
    let pid_str = job_display_pid(job)
        .map(|p| bstr::i64_to_bytes(p as i64))
        .unwrap_or_default();
    let state = match job.state {
        crate::shell::JobState::Running => b"Running".to_vec(),
        crate::shell::JobState::Stopped(sig) => ByteWriter::new()
            .bytes(b"Stopped (")
            .bytes(sys::signal_name(sig))
            .byte(b')')
            .finish(),
        crate::shell::JobState::Done(status) => {
            if status == 0 {
                b"Done".to_vec()
            } else {
                ByteWriter::new()
                    .bytes(b"Done(")
                    .i32_val(status)
                    .byte(b')')
                    .finish()
            }
        }
    };
    (state, pid_str)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum JobsMode {
    Normal,
    Long,
    PidOnly,
}

pub(super) fn parse_jobs_options(argv: &[Vec<u8>]) -> Result<(JobsMode, usize), Vec<u8>> {
    let mut mode = JobsMode::Normal;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        if arg.first() != Some(&b'-') || arg == b"-" {
            break;
        }
        if arg == b"--" {
            index += 1;
            break;
        }
        match arg.as_slice() {
            b"-p" => mode = JobsMode::PidOnly,
            b"-l" => mode = JobsMode::Long,
            _ => {
                return Err(ByteWriter::new()
                    .bytes(b"jobs: invalid option: ")
                    .bytes(arg)
                    .finish());
            }
        }
        index += 1;
    }
    Ok((mode, index))
}

pub(super) fn parse_jobs_operands(
    operands: &[Vec<u8>],
    shell: &Shell,
) -> Result<Option<Vec<usize>>, Vec<u8>> {
    if operands.is_empty() {
        return Ok(None);
    }
    let mut ids = Vec::new();
    for operand in operands {
        let Some(id) = resolve_job_id(shell, Some(operand.as_slice())) else {
            return Err(ByteWriter::new()
                .bytes(b"jobs: invalid job id: ")
                .bytes(operand)
                .finish());
        };
        ids.push(id);
    }
    Ok(Some(ids))
}

pub(super) fn job_display_pid(job: &crate::shell::Job) -> Option<sys::Pid> {
    job.pgid
        .or_else(|| job.children.first().map(|child| child.pid))
        .or_else(|| job.last_pid)
}

pub(super) fn fg(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if !shell.options.monitor {
        return Ok(diag_status(shell, 1, b"fg: no job control"));
    }
    let id = resolve_job_id(shell, argv.get(1).map(|v| v.as_slice()))
        .or_else(|| shell.current_job_id())
        .ok_or_else(|| shell.diagnostic(1, b"fg: no current job"))?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        write_stdout_line(&job.command);
    }
    shell.continue_job(id, true)?;
    let status = shell.wait_for_job(id)?;
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn bg(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if !shell.options.monitor {
        return Ok(diag_status(shell, 1, b"bg: no job control"));
    }
    let id = resolve_job_id(shell, argv.get(1).map(|v| v.as_slice()))
        .or_else(|| {
            shell
                .jobs
                .iter()
                .rev()
                .find(|j| matches!(j.state, crate::shell::JobState::Stopped(_)))
                .map(|j| j.id)
        })
        .ok_or_else(|| shell.diagnostic(1, b"bg: no current job"))?;
    shell.continue_job(id, false)?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        let line = ByteWriter::new()
            .byte(b'[')
            .usize_val(id)
            .bytes(b"] ")
            .bytes(&job.command)
            .finish();
        write_stdout_line(&line);
    }
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn wait(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        return Ok(BuiltinOutcome::Status(shell.wait_for_all_jobs()?));
    }
    let mut status = 0;
    for operand in &argv[1..] {
        status = match parse_wait_operand(operand, shell) {
            Ok(WaitOperand::Job(id)) => shell.wait_for_job_operand(id)?,
            Ok(WaitOperand::Pid(pid)) => shell.wait_for_pid_operand(pid)?,
            Err(message) => {
                shell.diagnostic(1, &message);
                1
            }
        };
    }
    Ok(BuiltinOutcome::Status(status))
}

#[derive(Clone, Copy)]
pub(super) enum WaitOperand {
    Job(usize),
    Pid(sys::Pid),
}

pub(super) fn resolve_job_id(shell: &Shell, operand: Option<&[u8]>) -> Option<usize> {
    let operand = operand?;
    let spec = if operand.first() == Some(&b'%') {
        &operand[1..]
    } else {
        operand
    };
    match spec {
        b"%" | b"+" | b"" => shell.current_job_id(),
        b"-" => shell.previous_job_id(),
        _ => {
            if let Some(rest) = spec.strip_prefix(b"?") {
                return shell.find_job_by_substring(rest);
            }
            if let Some(n) = parse_usize(spec) {
                if shell.jobs.iter().any(|j| j.id == n) {
                    return Some(n);
                }
                return None;
            }
            shell.find_job_by_prefix(spec)
        }
    }
}

pub(super) fn parse_wait_operand(operand: &[u8], shell: &Shell) -> Result<WaitOperand, Vec<u8>> {
    if operand.first() == Some(&b'%') {
        return resolve_job_id(shell, Some(operand))
            .map(WaitOperand::Job)
            .ok_or_else(|| {
                ByteWriter::new()
                    .bytes(b"wait: invalid job id: ")
                    .bytes(operand)
                    .finish()
            });
    }
    bstr::parse_i64(operand)
        .map(|v| WaitOperand::Pid(v as sys::Pid))
        .ok_or_else(|| {
            ByteWriter::new()
                .bytes(b"wait: invalid process id: ")
                .bytes(operand)
                .finish()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;

    #[test]
    fn jobs_stopped_job_output() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"[1] + Stopped (SIGTSTP) vim\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"vim"[..].into(),
                    pgid: Some(111),
                    last_pid: Some(111),
                    last_status: None,
                    children: vec![],
                    state: crate::shell::JobState::Stopped(sys::SIGTSTP),
                    saved_termios: None,
                });
                invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
            },
        );
    }

    #[test]
    fn jobs_invalid_option_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: jobs: invalid option: -z\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"-z".to_vec()]).expect("jobs -z");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn format_job_state_coverage() {
        assert_no_syscalls(|| {
            let running_job = crate::shell::Job {
                id: 1,
                command: b"cmd"[..].into(),
                pgid: Some(999),
                last_pid: Some(999),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            };
            let (state, pid) = format_job_state(&running_job);
            assert_eq!(state, b"Running");
            assert_eq!(pid, b"999");

            let done_job = crate::shell::Job {
                id: 2,
                command: b"cmd"[..].into(),
                pgid: Some(888),
                last_pid: Some(888),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Done(0),
                saved_termios: None,
            };
            let (state, _) = format_job_state(&done_job);
            assert_eq!(state, b"Done");

            let done_fail = crate::shell::Job {
                id: 3,
                command: b"cmd"[..].into(),
                pgid: Some(777),
                last_pid: Some(777),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Done(42),
                saved_termios: None,
            };
            let (state, _) = format_job_state(&done_fail);
            assert_eq!(state, b"Done(42)");
        });
    }

    #[test]
    fn parse_jobs_options_coverage() {
        assert_no_syscalls(|| {
            let (mode, idx) = parse_jobs_options(&[b"jobs".to_vec(), b"-p".to_vec()]).expect("-p");
            assert_eq!(mode, JobsMode::PidOnly);
            assert_eq!(idx, 2);

            let (mode, idx) = parse_jobs_options(&[b"jobs".to_vec(), b"-l".to_vec()]).expect("-l");
            assert_eq!(mode, JobsMode::Long);
            assert_eq!(idx, 2);

            let (mode, idx) =
                parse_jobs_options(&[b"jobs".to_vec(), b"--".to_vec(), b"%1".to_vec()])
                    .expect("--");
            assert_eq!(mode, JobsMode::Normal);
            assert_eq!(idx, 2);

            let err = parse_jobs_options(&[b"jobs".to_vec(), b"-z".to_vec()]);
            assert!(err.is_err());
        });
    }

    #[test]
    fn fg_no_job_control_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: fg: no job control\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"fg".to_vec()]).expect("fg");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn bg_no_job_control_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: bg: no job control\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"bg".to_vec()]).expect("bg");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn fg_no_current_job_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: fg: no current job\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let _ = invoke(&mut shell, &[b"fg".to_vec()]);
            },
        );
    }

    #[test]
    fn bg_no_current_job_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: bg: no current job\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let _ = invoke(&mut shell, &[b"bg".to_vec()]);
            },
        );
    }

    #[test]
    fn wait_invalid_job_id() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: wait: invalid job id: %nosuch\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"wait".to_vec(), b"%nosuch".to_vec()])
                    .expect("wait %nosuch");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn wait_invalid_pid() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: wait: invalid process id: abc\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"wait".to_vec(), b"abc".to_vec()]).expect("wait abc");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn parse_wait_operand_variants() {
        assert_no_syscalls(|| {
            let shell = test_shell();

            let result = parse_wait_operand(b"12345", &shell);
            assert!(result.is_ok());
            assert!(matches!(result.unwrap(), WaitOperand::Pid(12345)));

            let result = parse_wait_operand(b"abc", &shell);
            assert!(result.is_err());

            let result = parse_wait_operand(b"%nosuch", &shell);
            assert!(result.is_err());
        });
    }

    #[test]
    fn resolve_job_id_variants() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(crate::shell::Job {
                id: 1,
                command: b"sleep"[..].into(),
                pgid: Some(100),
                last_pid: Some(100),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            });
            shell.jobs.push(crate::shell::Job {
                id: 2,
                command: b"vim"[..].into(),
                pgid: Some(200),
                last_pid: Some(200),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            });

            assert_eq!(resolve_job_id(&shell, Some(b"%+")), shell.current_job_id());
            assert_eq!(resolve_job_id(&shell, Some(b"%%")), shell.current_job_id());
            assert_eq!(resolve_job_id(&shell, Some(b"%-")), shell.previous_job_id());
            assert_eq!(resolve_job_id(&shell, Some(b"%1")), Some(1));
            assert_eq!(resolve_job_id(&shell, Some(b"%2")), Some(2));
            assert_eq!(resolve_job_id(&shell, Some(b"%99")), None);
            assert_eq!(resolve_job_id(&shell, None), None);
        });
    }

    #[test]
    fn job_current_marker_variants() {
        assert_no_syscalls(|| {
            assert_eq!(job_current_marker(1, Some(1), Some(2)), b'+');
            assert_eq!(job_current_marker(2, Some(1), Some(2)), b'-');
            assert_eq!(job_current_marker(3, Some(1), Some(2)), b' ');
        });
    }

    #[test]
    fn parse_jobs_operands_invalid() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = parse_jobs_operands(&[b"%99".to_vec()], &shell);
            assert!(result.is_err());
        });
    }

    #[test]
    fn fg_no_job_control() {
        let msg = diag(b"fg: no job control");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"fg".to_vec()]).expect("fg");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn bg_no_job_control() {
        let msg = diag(b"bg: no job control");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"bg".to_vec()]).expect("bg");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }
}
