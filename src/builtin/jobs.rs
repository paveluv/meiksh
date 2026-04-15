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
    use crate::trace_entries;

    fn fake_handle(pid: sys::Pid) -> sys::ChildHandle {
        sys::ChildHandle {
            pid,
            stdout_fd: None,
        }
    }

    #[test]
    fn jobs_stopped_job_output() {
        run_trace(
            trace_entries![write(
                fd(crate::sys::STDOUT_FILENO),
                bytes(b"[1] + Stopped (SIGTSTP) vim\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: jobs: invalid option: -z\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: fg: no job control\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: bg: no job control\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: fg: no current job\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: bg: no current job\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: wait: invalid job id: %nosuch\n"),
            ) -> auto,],
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
            trace_entries![write(
                fd(crate::sys::STDERR_FILENO),
                bytes(b"meiksh: wait: invalid process id: abc\n"),
            ) -> auto,],
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
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"fg".to_vec()]).expect("fg");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn bg_no_job_control() {
        let msg = diag(b"bg: no job control");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"bg".to_vec()]).expect("bg");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn jobs_reaped_done_zero_status() {
        run_trace(
            trace_entries![
                waitpid(1001, _) -> status(0),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"[1]   Done\tsleep 10\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 10"[..].into(),
                    pgid: Some(1001),
                    last_pid: Some(1001),
                    last_status: None,
                    children: vec![fake_handle(1001)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_reaped_done_nonzero_status() {
        run_trace(
            trace_entries![
                waitpid(1002, _) -> status(42),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"[1]   Done(42)\texit 42\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"exit 42"[..].into(),
                    pgid: Some(1002),
                    last_pid: Some(1002),
                    last_status: None,
                    children: vec![fake_handle(1002)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_reaped_signaled() {
        run_trace(
            trace_entries![
                waitpid(1003, _) -> signaled_sig(sys::SIGTERM),
                write(
                    fd(crate::sys::STDOUT_FILENO),
                    bytes(b"[1]   Terminated (SIGTERM)\tkilled\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"killed"[..].into(),
                    pgid: Some(1003),
                    last_pid: Some(1003),
                    last_status: None,
                    children: vec![fake_handle(1003)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_running_pid_only_mode() {
        run_trace(
            trace_entries![
                waitpid(555, _) -> pid(0),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"555\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 60"[..].into(),
                    pgid: Some(555),
                    last_pid: Some(555),
                    last_status: None,
                    children: vec![fake_handle(555)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"-p".to_vec()]).expect("jobs -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_running_long_mode() {
        run_trace(
            trace_entries![
                waitpid(777, _) -> pid(0),
                write(
                    fd(crate::sys::STDOUT_FILENO),
                    bytes(b"[1] + 777 Running sleep 99\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 99"[..].into(),
                    pgid: Some(777),
                    last_pid: Some(777),
                    last_status: None,
                    children: vec![fake_handle(777)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"-l".to_vec()]).expect("jobs -l");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_running_normal_mode() {
        run_trace(
            trace_entries![
                waitpid(777, _) -> pid(0),
                write(
                    fd(crate::sys::STDOUT_FILENO),
                    bytes(b"[1] + Running sleep 99\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 99"[..].into(),
                    pgid: Some(777),
                    last_pid: Some(777),
                    last_status: None,
                    children: vec![fake_handle(777)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_reaped_skips_unselected() {
        run_trace(
            trace_entries![
                waitpid(2001, _) -> status(0),
                waitpid(2002, _) -> status(0),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"[2]   Done\techo b\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"echo a"[..].into(),
                    pgid: Some(2001),
                    last_pid: Some(2001),
                    last_status: None,
                    children: vec![fake_handle(2001)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                shell.jobs.push(crate::shell::Job {
                    id: 2,
                    command: b"echo b"[..].into(),
                    pgid: Some(2002),
                    last_pid: Some(2002),
                    last_status: None,
                    children: vec![fake_handle(2002)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"%2".to_vec()]).expect("jobs %2");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn parse_jobs_operands_valid_id() {
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
            let result = parse_jobs_operands(&[b"%1".to_vec()], &shell);
            assert_eq!(result, Ok(Some(vec![1])));
        });
    }

    #[test]
    fn parse_jobs_options_bare_dash_stops() {
        assert_no_syscalls(|| {
            let (mode, idx) =
                parse_jobs_options(&[b"jobs".to_vec(), b"-".to_vec()]).expect("bare dash");
            assert_eq!(mode, JobsMode::Normal);
            assert_eq!(idx, 1);
        });
    }

    #[test]
    fn resolve_job_id_bare_operand_without_percent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(crate::shell::Job {
                id: 3,
                command: b"sleep"[..].into(),
                pgid: Some(100),
                last_pid: Some(100),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            });
            assert_eq!(resolve_job_id(&shell, Some(b"3")), Some(3));
            assert_eq!(resolve_job_id(&shell, Some(b"99")), None);
        });
    }

    #[test]
    fn resolve_job_id_substring_search() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(crate::shell::Job {
                id: 1,
                command: b"sleep 999"[..].into(),
                pgid: Some(100),
                last_pid: Some(100),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            });
            assert_eq!(resolve_job_id(&shell, Some(b"%?999")), Some(1));
            assert_eq!(resolve_job_id(&shell, Some(b"%?zzz")), None);
        });
    }

    #[test]
    fn fg_with_valid_job() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"sleep 10\n")) -> auto,
                kill(int(-500), int(sys::SIGCONT)) -> 0,
                waitpid(int(500), _, int(sys::WUNTRACED)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 10"[..].into(),
                    pgid: Some(500),
                    last_pid: Some(500),
                    last_status: None,
                    children: vec![fake_handle(500)],
                    state: crate::shell::JobState::Stopped(sys::SIGTSTP),
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"fg".to_vec(), b"%1".to_vec()]).expect("fg %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn bg_with_valid_job() {
        run_trace(
            trace_entries![
                kill(int(-600), int(sys::SIGCONT)) -> 0,
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"[1] sleep 20\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 20"[..].into(),
                    pgid: Some(600),
                    last_pid: Some(600),
                    last_status: None,
                    children: vec![fake_handle(600)],
                    state: crate::shell::JobState::Stopped(sys::SIGTSTP),
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"bg".to_vec(), b"%1".to_vec()]).expect("bg %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn wait_with_job_operand() {
        run_trace(
            trace_entries![
                waitpid(int(700), _, int(sys::WUNTRACED)) -> status(5),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"exit 5"[..].into(),
                    pgid: Some(700),
                    last_pid: Some(700),
                    last_status: None,
                    children: vec![fake_handle(700)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"wait".to_vec(), b"%1".to_vec()]).expect("wait %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(5)));
            },
        );
    }

    #[test]
    fn job_display_pid_fallbacks() {
        assert_no_syscalls(|| {
            let job_with_pgid = crate::shell::Job {
                id: 1,
                command: b"cmd"[..].into(),
                pgid: Some(100),
                last_pid: Some(200),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            };
            assert_eq!(job_display_pid(&job_with_pgid), Some(100));

            let job_with_children = crate::shell::Job {
                id: 2,
                command: b"cmd"[..].into(),
                pgid: None,
                last_pid: Some(300),
                last_status: None,
                children: vec![fake_handle(250)],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            };
            assert_eq!(job_display_pid(&job_with_children), Some(250));

            let job_with_last_pid = crate::shell::Job {
                id: 3,
                command: b"cmd"[..].into(),
                pgid: None,
                last_pid: Some(400),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            };
            assert_eq!(job_display_pid(&job_with_last_pid), Some(400));

            let job_no_pid = crate::shell::Job {
                id: 4,
                command: b"cmd"[..].into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            };
            assert_eq!(job_display_pid(&job_no_pid), None);
        });
    }

    #[test]
    fn format_job_state_stopped() {
        assert_no_syscalls(|| {
            let job = crate::shell::Job {
                id: 1,
                command: b"vim"[..].into(),
                pgid: Some(999),
                last_pid: Some(999),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Stopped(sys::SIGTSTP),
                saved_termios: None,
            };
            let (state, pid) = format_job_state(&job);
            assert_eq!(state, b"Stopped (SIGTSTP)");
            assert_eq!(pid, b"999");
        });
    }

    #[test]
    fn jobs_pid_only_skips_reaped() {
        run_trace(
            trace_entries![
                waitpid(3001, _) -> status(0),
                waitpid(444, _) -> pid(0),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"444\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"done"[..].into(),
                    pgid: Some(3001),
                    last_pid: Some(3001),
                    last_status: None,
                    children: vec![fake_handle(3001)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                shell.jobs.push(crate::shell::Job {
                    id: 2,
                    command: b"running"[..].into(),
                    pgid: Some(444),
                    last_pid: Some(444),
                    last_status: None,
                    children: vec![fake_handle(444)],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"-p".to_vec()]).expect("jobs -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn parse_jobs_operands_error_via_invoke() {
        run_trace(
            trace_entries![
                write(
                    fd(crate::sys::STDERR_FILENO),
                    bytes(b"meiksh: jobs: invalid job id: %nosuch\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"jobs".to_vec(), b"%nosuch".to_vec()])
                    .expect("jobs %nosuch");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn resolve_job_id_prefix_search() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(crate::shell::Job {
                id: 1,
                command: b"sleep 10"[..].into(),
                pgid: Some(100),
                last_pid: Some(100),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Running,
                saved_termios: None,
            });
            assert_eq!(resolve_job_id(&shell, Some(b"%sleep")), Some(1));
            assert_eq!(resolve_job_id(&shell, Some(b"sleep")), Some(1));
        });
    }

    #[test]
    fn jobs_reaped_stopped_is_silent() {
        run_trace(
            trace_entries![
                waitpid(300, _) -> stopped_sig(crate::sys::SIGTSTP),
                waitpid(300, _) -> pid(0),
                write(fd(1), bytes(b"[1] + Stopped (SIGTSTP) stopped-cmd\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"stopped-cmd"[..].into(),
                    pgid: Some(300),
                    last_pid: Some(300),
                    last_status: None,
                    children: vec![crate::sys::ChildHandle {
                        pid: 300,
                        stdout_fd: None,
                    }],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome = invoke(&mut shell, &[b"jobs".to_vec()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_skips_unselected_running_jobs() {
        run_trace(
            trace_entries![
                waitpid(100, _) -> pid(0),
                waitpid(200, _) -> pid(0),
                write(fd(1), bytes(b"[1] - Running sleep 1\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: b"sleep 1"[..].into(),
                    pgid: Some(100),
                    last_pid: Some(100),
                    last_status: None,
                    children: vec![crate::sys::ChildHandle {
                        pid: 100,
                        stdout_fd: None,
                    }],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                shell.jobs.push(crate::shell::Job {
                    id: 2,
                    command: b"sleep 2"[..].into(),
                    pgid: Some(200),
                    last_pid: Some(200),
                    last_status: None,
                    children: vec![crate::sys::ChildHandle {
                        pid: 200,
                        stdout_fd: None,
                    }],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                let outcome =
                    invoke(&mut shell, &[b"jobs".to_vec(), b"%1".to_vec()]).expect("jobs %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }
}
