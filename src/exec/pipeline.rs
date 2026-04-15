#![allow(unused_imports)]

use std::collections::HashMap;

use crate::arena::ByteArena;
use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand;
use crate::shell::{
    BlockingWaitOutcome, FlowSignal, JobState, PendingControl, Shell, ShellError, VarError,
};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};
use crate::sys;

use super::*;

pub(super) fn execute_pipeline(
    shell: &mut Shell,
    pipeline: &Pipeline,
    asynchronous: bool,
) -> Result<i32, ShellError> {
    let timer = if pipeline.timed != TimedMode::Off {
        Some(TimeSnapshot::now())
    } else {
        None
    };

    let status = execute_pipeline_inner(shell, pipeline, asynchronous)?;

    if let Some(before) = timer {
        write_time_report(&before, pipeline.timed);
    }

    Ok(status)
}

pub(super) fn execute_pipeline_inner(
    shell: &mut Shell,
    pipeline: &Pipeline,
    asynchronous: bool,
) -> Result<i32, ShellError> {
    if pipeline.commands.len() == 1 {
        if !asynchronous {
            let saved_suppressed = shell.errexit_suppressed;
            if pipeline.negated {
                shell.errexit_suppressed = true;
            }
            let status = execute_command(shell, &pipeline.commands[0])?;
            shell.errexit_suppressed = saved_suppressed;
            return Ok(if pipeline.negated {
                if status == 0 { 1 } else { 0 }
            } else {
                status
            });
        }
    }

    let pipefail = shell.options.pipefail;
    let spawned = spawn_pipeline(shell, pipeline, None)?;
    if asynchronous {
        return Ok(0);
    }

    let desc = render_pipeline(pipeline);
    let status = wait_for_pipeline(shell, spawned, Some(&desc), pipefail)?;

    if pipeline.negated {
        Ok(if status == 0 { 1 } else { 0 })
    } else {
        Ok(status)
    }
}

pub(super) struct TimeSnapshot {
    pub(super) wall_ns: u64,
    pub(super) user_ticks: u64,
    pub(super) sys_ticks: u64,
    pub(super) child_user_ticks: u64,
    pub(super) child_sys_ticks: u64,
    pub(super) ticks_per_sec: u64,
}

impl TimeSnapshot {
    fn now() -> Self {
        let wall_ns = sys::monotonic_clock_ns();
        let times = sys::process_times().unwrap_or(sys::ProcessTimes {
            user_ticks: 0,
            system_ticks: 0,
            child_user_ticks: 0,
            child_system_ticks: 0,
        });
        let ticks_per_sec = sys::clock_ticks_per_second().unwrap_or(100);
        Self {
            wall_ns,
            user_ticks: times.user_ticks,
            sys_ticks: times.system_ticks,
            child_user_ticks: times.child_user_ticks,
            child_sys_ticks: times.child_system_ticks,
            ticks_per_sec,
        }
    }
}

pub(super) fn write_time_report(before: &TimeSnapshot, mode: TimedMode) {
    let after = TimeSnapshot::now();
    let real_secs = (after.wall_ns.saturating_sub(before.wall_ns)) as f64 / 1_000_000_000.0;
    let tps = before.ticks_per_sec as f64;
    let user_secs = ((after.user_ticks + after.child_user_ticks)
        .saturating_sub(before.user_ticks + before.child_user_ticks)) as f64
        / tps;
    let sys_secs = ((after.sys_ticks + after.child_sys_ticks)
        .saturating_sub(before.sys_ticks + before.child_sys_ticks)) as f64
        / tps;
    match mode {
        TimedMode::Posix => {
            let posix_fmt = |label: &[u8], secs: f64| {
                ByteWriter::new()
                    .bytes(label)
                    .byte(b' ')
                    .f64_fixed(secs, 2)
                    .byte(b'\n')
                    .finish()
            };
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &posix_fmt(b"real", real_secs));
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &posix_fmt(b"user", user_secs));
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &posix_fmt(b"sys", sys_secs));
        }
        _ => {
            let fmt = |secs: f64| -> Vec<u8> {
                let mins = (secs / 60.0) as u64;
                let remainder = secs - (mins as f64 * 60.0);
                ByteWriter::new()
                    .u64_val(mins)
                    .byte(b'm')
                    .f64_fixed(remainder, 3)
                    .byte(b's')
                    .finish()
            };
            let mut msg = ByteWriter::new()
                .bytes(b"\nreal\t")
                .bytes(&fmt(real_secs))
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            msg = ByteWriter::new()
                .bytes(b"user\t")
                .bytes(&fmt(user_secs))
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            msg = ByteWriter::new()
                .bytes(b"sys\t")
                .bytes(&fmt(sys_secs))
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        }
    }
}

pub(super) fn fork_and_execute_command(
    shell: &mut Shell,
    command: &Command,
    stdin_fd: Option<sys::RawFd>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    let stdout_pipe = if pipe_stdout {
        let (r, w) = sys::create_pipe().map_err(|e| shell.diagnostic_syserr(1, &e))?;
        Some((r, w))
    } else {
        Option::None
    };

    let pid = sys::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
    if pid == 0 {
        if let Some(fd) = stdin_fd {
            let _ = sys::duplicate_fd(fd, sys::STDIN_FILENO);
            let _ = sys::close_fd(fd);
        }
        if let Some((r, w)) = stdout_pipe {
            let _ = sys::close_fd(r);
            let _ = sys::duplicate_fd(w, sys::STDOUT_FILENO);
            let _ = sys::close_fd(w);
        }
        match process_group {
            ProcessGroupPlan::NewGroup => {
                let _ = sys::set_process_group(0, 0);
            }
            ProcessGroupPlan::Join(pgid) => {
                let _ = sys::set_process_group(0, pgid);
            }
            _ => {}
        }
        let mut child_shell = shell.clone();
        child_shell.owns_terminal = false;
        child_shell.in_subshell = true;
        child_shell.restore_signals_for_child();
        let _ = child_shell.reset_traps_for_subshell();
        let status = execute_command_in_pipeline_child(&mut child_shell, command).unwrap_or(1);
        let status = child_shell.run_exit_trap(status).unwrap_or(status);
        sys::exit_process(status as sys::RawFd);
    }

    if let Some(fd) = stdin_fd {
        let _ = sys::close_fd(fd);
    }
    let stdout_read = stdout_pipe.map(|(r, w)| {
        let _ = sys::close_fd(w);
        r
    });

    Ok(sys::ChildHandle {
        pid,
        stdout_fd: stdout_read,
    })
}

pub(super) fn spawn_pipeline(
    shell: &mut Shell,
    pipeline: &Pipeline,
    stdin_override: Option<i32>,
) -> Result<SpawnedProcesses, ShellError> {
    let mut previous_stdout_fd: Option<i32> = stdin_override;
    let mut children = Vec::new();
    let mut pgid = None;

    for (index, command) in pipeline.commands.iter().enumerate() {
        let is_last = index + 1 == pipeline.commands.len();

        let plan = match pgid {
            Some(pgid) => ProcessGroupPlan::Join(pgid),
            None => ProcessGroupPlan::NewGroup,
        };

        let handle =
            fork_and_execute_command(shell, command, previous_stdout_fd.take(), !is_last, plan)?;

        if pgid.is_none() {
            let child_pgid = handle.pid;
            let _ = sys::set_process_group(child_pgid, child_pgid);
            pgid = Some(child_pgid);
        } else if let Some(job_pgid) = pgid {
            let _ = sys::set_process_group(handle.pid, job_pgid);
        }
        previous_stdout_fd = handle.stdout_fd;
        children.push(sys::ChildHandle {
            pid: handle.pid,
            stdout_fd: None,
        });
    }

    Ok(SpawnedProcesses { children, pgid })
}

pub(super) fn wait_for_pipeline(
    shell: &mut Shell,
    spawned: SpawnedProcesses,
    command_desc: Option<&[u8]>,
    pipefail: bool,
) -> Result<i32, ShellError> {
    let (last_status, rightmost_nonzero) = wait_for_children_inner(shell, spawned, command_desc)?;
    if pipefail {
        Ok(rightmost_nonzero)
    } else {
        Ok(last_status)
    }
}

#[cfg(test)]
pub(super) fn wait_for_children(
    shell: &mut Shell,
    spawned: SpawnedProcesses,
    command_desc: Option<&[u8]>,
) -> Result<i32, ShellError> {
    let (last_status, _) = wait_for_children_inner(shell, spawned, command_desc)?;
    Ok(last_status)
}

pub(super) fn wait_for_children_inner(
    shell: &mut Shell,
    mut spawned: SpawnedProcesses,
    command_desc: Option<&[u8]>,
) -> Result<(i32, i32), ShellError> {
    let saved_foreground = if shell.owns_terminal {
        handoff_foreground(spawned.pgid)
    } else {
        None
    };
    let mut last_status = 0;
    let mut rightmost_nonzero = 0;
    for i in 0..spawned.children.len() {
        match shell.wait_for_child_blocking(spawned.children[i].pid, !shell.in_subshell)? {
            BlockingWaitOutcome::Exited(code) => {
                last_status = code;
                if code != 0 {
                    rightmost_nonzero = code;
                }
            }
            BlockingWaitOutcome::Signaled(sig) => {
                let code = 128 + sig;
                last_status = code;
                rightmost_nonzero = code;
            }
            BlockingWaitOutcome::Stopped(sig) => {
                restore_foreground(saved_foreground);
                let desc: Box<[u8]> = command_desc.unwrap_or(b"").into();
                let children = std::mem::take(&mut spawned.children);
                let id = shell.register_background_job(desc, spawned.pgid, children);
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = JobState::Stopped(sig);
                if shell.interactive {
                    shell.jobs[idx].saved_termios = sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                    let msg = ByteWriter::new()
                        .byte(b'[')
                        .usize_val(id)
                        .bytes(b"] Stopped (")
                        .bytes(sys::signal_name(sig))
                        .bytes(b")\t")
                        .bytes(&shell.jobs[idx].command)
                        .byte(b'\n')
                        .finish();
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
                }
                return Ok((128 + sig, 128 + sig));
            }
        }
    }
    restore_foreground(saved_foreground);
    Ok((last_status, rightmost_nonzero))
}

pub(super) fn wait_for_external_child(
    shell: &mut Shell,
    handle: &sys::ChildHandle,
    pgid: Option<sys::Pid>,
    command_desc: Option<&[u8]>,
) -> Result<i32, ShellError> {
    let saved_foreground = if shell.owns_terminal {
        handoff_foreground(pgid)
    } else {
        None
    };
    match shell.wait_for_child_blocking(handle.pid, !shell.in_subshell)? {
        BlockingWaitOutcome::Exited(status) => {
            restore_foreground(saved_foreground);
            Ok(status)
        }
        BlockingWaitOutcome::Signaled(sig) => {
            restore_foreground(saved_foreground);
            Ok(128 + sig)
        }
        BlockingWaitOutcome::Stopped(sig) => {
            restore_foreground(saved_foreground);
            let desc: Box<[u8]> = command_desc.unwrap_or(b"").into();
            let children = vec![handle.clone()];
            let id = shell.register_background_job(desc, pgid, children);
            let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
            shell.jobs[idx].state = JobState::Stopped(sig);
            if shell.interactive {
                shell.jobs[idx].saved_termios = sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                let msg = ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Stopped (")
                    .bytes(sys::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&shell.jobs[idx].command)
                    .byte(b'\n')
                    .finish();
                let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            }
            Ok(128 + sig)
        }
    }
}

pub(super) fn handoff_foreground(pgid: Option<sys::Pid>) -> Option<sys::Pid> {
    let Some(pgid) = pgid else {
        return None;
    };
    if !(sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO)) {
        return None;
    }
    let Ok(saved) = sys::current_foreground_pgrp(sys::STDIN_FILENO) else {
        return None;
    };
    let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
    Some(saved)
}

pub(super) fn restore_foreground(saved_foreground: Option<sys::Pid>) {
    if let Some(pgid) = saved_foreground {
        let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::test_support::*;
    use crate::shell::Shell;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::trace_entries;

    #[test]
    fn execute_pipeline_async_single_command() {
        run_trace(
            trace_entries![
                fork() -> pid(1000), child: [
                    setpgid(int(0), int(0)) -> 0,
                ],
                setpgid(int(1000), int(1000)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let pipeline = Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"true".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    })]
                    .into_boxed_slice(),
                };
                let status = execute_pipeline(&mut shell, &pipeline, true).expect("async");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn execute_pipeline_negated_multi_command() {
        run_trace(
            trace_entries![
                pipe() -> fds(200, 201),
                fork() -> pid(1000), child: [
                    close(fd(200)) -> 0,
                    dup2(fd(201), fd(1)) -> 0,
                    close(fd(201)) -> 0,
                    setpgid(int(0), int(0)) -> 0,
                    write(fd(1), bytes(b"ok")) -> 2,
                ],
                close(fd(201)) -> 0,
                setpgid(int(1000), int(1000)) -> 0,
                fork() -> pid(1001), child: [
                    dup2(fd(200), fd(0)) -> 0,
                    close(fd(200)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                    stat(str("/usr/bin/wc"), _) -> stat_file(0o755),
                    open(str("/usr/bin/wc"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"#!/bin/sh\n"),
                    close(fd(20)) -> 0,
                    execvp(str("/usr/bin/wc"), _) -> 0,
                ],
                close(fd(200)) -> 0,
                setpgid(int(1001), int(1000)) -> 0,
                waitpid(int(1000), _, int(sys::WUNTRACED as i64)) -> status(0),
                waitpid(int(1001), _, int(sys::WUNTRACED as i64)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let pipeline = Pipeline {
                    negated: true,
                    timed: TimedMode::Off,
                    commands: vec![
                        Command::Simple(SimpleCommand {
                            words: vec![
                                Word {
                                    raw: b"printf".to_vec().into(),
                                    line: 0,
                                },
                                Word {
                                    raw: b"ok".to_vec().into(),
                                    line: 0,
                                },
                            ]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        }),
                        Command::Simple(SimpleCommand {
                            words: vec![
                                Word {
                                    raw: b"wc".to_vec().into(),
                                    line: 0,
                                },
                                Word {
                                    raw: b"-c".to_vec().into(),
                                    line: 0,
                                },
                            ]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        }),
                    ]
                    .into_boxed_slice(),
                };
                let status =
                    execute_pipeline(&mut shell, &pipeline, false).expect("negated pipeline");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn spawn_pipeline_forks_compound_commands() {
        let program = Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: b"true".to_vec().into(),
                                line: 0,
                            }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: Vec::new().into_boxed_slice(),
                },
                asynchronous: false,
                line: 0,
            }]
            .into_boxed_slice(),
        };

        run_trace(
            trace_entries![
                pipe() -> fds(200, 201),
                fork() -> pid(1000), child: [
                    close(fd(200)) -> 0,
                    dup2(fd(201), fd(1)) -> 0,
                    close(fd(201)) -> 0,
                    setpgid(int(0), int(0)) -> 0,
                    fork() -> pid(2000), child: [],
                    waitpid(int(2000), _, int(0)) -> status(0),
                ],
                close(fd(201)) -> 0,
                setpgid(int(1000), int(1000)) -> 0,
                fork() -> pid(1001), child: [
                    dup2(fd(200), fd(0)) -> 0,
                    close(fd(200)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                ],
                close(fd(200)) -> 0,
                setpgid(int(1001), int(1000)) -> 0,
                waitpid(int(1000), _, int(0)) -> status(0),
                waitpid(int(1001), _, int(0)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                let spawned = spawn_pipeline(
                    &mut shell,
                    &Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![
                            Command::Subshell(program.clone()),
                            Command::Group(program.clone()),
                        ]
                        .into_boxed_slice(),
                    },
                    None,
                )
                .expect("spawn");
                for child in spawned.children {
                    let _ = child.wait().expect("wait");
                }
            },
        );
    }

    #[test]
    fn spawn_pipeline_covers_compound_command_variants() {
        let program = Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: b"true".to_vec().into(),
                                line: 0,
                            }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: Vec::new().into_boxed_slice(),
                },
                asynchronous: false,
                line: 0,
            }]
            .into_boxed_slice(),
        };
        let pipeline = Pipeline {
            negated: false,
            timed: TimedMode::Off,
            commands: vec![
                Command::FunctionDef(FunctionDef {
                    name: b"f".to_vec().into(),
                    body: Box::new(Command::Group(program.clone())),
                }),
                Command::If(IfCommand {
                    condition: program.clone(),
                    then_branch: program.clone(),
                    elif_branches: vec![crate::syntax::ElifBranch {
                        condition: program.clone(),
                        body: program.clone(),
                    }]
                    .into_boxed_slice(),
                    else_branch: Some(program.clone()),
                }),
                Command::Loop(LoopCommand {
                    kind: LoopKind::Until,
                    condition: program.clone(),
                    body: program,
                }),
                Command::For(ForCommand {
                    name: b"item".to_vec().into(),
                    items: Some(
                        vec![Word {
                            raw: b"a".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                    ),
                    body: Program::default(),
                }),
                Command::Case(CaseCommand {
                    word: Word {
                        raw: b"item".to_vec().into(),
                        line: 0,
                    },
                    arms: vec![crate::syntax::CaseArm {
                        patterns: vec![Word {
                            raw: b"item".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        body: Program::default(),
                        fallthrough: false,
                    }]
                    .into_boxed_slice(),
                }),
            ]
            .into_boxed_slice(),
        };
        run_trace(
            trace_entries![
                pipe() -> fds(200, 201),
                fork() -> pid(1000), child: [
                    close(fd(200)) -> 0,
                    dup2(fd(201), fd(1)) -> 0,
                    close(fd(201)) -> 0,
                    setpgid(int(0), int(0)) -> 0,
                ],
                close(fd(201)) -> 0,
                setpgid(int(1000), int(1000)) -> 0,
                pipe() -> fds(202, 203),
                fork() -> pid(1001), child: [
                    dup2(fd(200), fd(0)) -> 0,
                    close(fd(200)) -> 0,
                    close(fd(202)) -> 0,
                    dup2(fd(203), fd(1)) -> 0,
                    close(fd(203)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                ],
                close(fd(200)) -> 0,
                close(fd(203)) -> 0,
                setpgid(int(1001), int(1000)) -> 0,
                pipe() -> fds(204, 205),
                fork() -> pid(1002), child: [
                    dup2(fd(202), fd(0)) -> 0,
                    close(fd(202)) -> 0,
                    close(fd(204)) -> 0,
                    dup2(fd(205), fd(1)) -> 0,
                    close(fd(205)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                ],
                close(fd(202)) -> 0,
                close(fd(205)) -> 0,
                setpgid(int(1002), int(1000)) -> 0,
                pipe() -> fds(206, 207),
                fork() -> pid(1003), child: [
                    dup2(fd(204), fd(0)) -> 0,
                    close(fd(204)) -> 0,
                    close(fd(206)) -> 0,
                    dup2(fd(207), fd(1)) -> 0,
                    close(fd(207)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                ],
                close(fd(204)) -> 0,
                close(fd(207)) -> 0,
                setpgid(int(1003), int(1000)) -> 0,
                fork() -> pid(1004), child: [
                    dup2(fd(206), fd(0)) -> 0,
                    close(fd(206)) -> 0,
                    setpgid(int(0), int(1000)) -> 0,
                ],
                close(fd(206)) -> 0,
                setpgid(int(1004), int(1000)) -> 0,
                waitpid(int(1000), _, int(0)) -> status(0),
                waitpid(int(1001), _, int(0)) -> status(0),
                waitpid(int(1002), _, int(0)) -> status(0),
                waitpid(int(1003), _, int(0)) -> status(0),
                waitpid(int(1004), _, int(0)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                let children = spawn_pipeline(&mut shell, &pipeline, None).expect("spawn");
                for child in children.children {
                    let _ = child.wait().expect("wait");
                }
            },
        );
    }

    #[test]
    fn errexit_suppressed_in_negated_pipeline() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("! true; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn execute_and_or_negated_last_pipeline_suppresses_errexit() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true && ! false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);

            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true || ! true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn timed_pipeline_exercises_time_report() {
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                times(_) -> 0,
                sysconf(_) -> 100,
                monotonic_clock_ns() -> 2_000_000_000,
                times(_) -> 0,
                sysconf(_) -> 100,
                write(fd(sys::STDERR_FILENO), bytes(b"\nreal\t0m1.000s\n")) -> auto,
                write(fd(sys::STDERR_FILENO), bytes(b"user\t0m0.000s\n")) -> auto,
                write(fd(sys::STDERR_FILENO), bytes(b"sys\t0m0.000s\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let program = parse_test("time true").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn timed_pipeline_posix_mode() {
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                times(_) -> 0,
                sysconf(_) -> 100,
                monotonic_clock_ns() -> 2_500_000_000,
                times(_) -> 0,
                sysconf(_) -> 100,
                write(fd(sys::STDERR_FILENO), bytes(b"real 1.50\n")) -> auto,
                write(fd(sys::STDERR_FILENO), bytes(b"user 0.00\n")) -> auto,
                write(fd(sys::STDERR_FILENO), bytes(b"sys 0.00\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let program = parse_test("time -p true").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 0);
            },
        );
    }
}
