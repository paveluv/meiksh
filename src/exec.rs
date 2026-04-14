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

fn var_error_bytes(e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(name)
            .bytes(b": readonly variable")
            .finish(),
    }
}

#[derive(Clone, Copy, Debug)]
enum ProcessGroupPlan {
    #[cfg_attr(not(test), allow(dead_code))]
    None,
    NewGroup,
    Join(sys::Pid),
}

struct SpawnedProcesses {
    children: Vec<sys::ChildHandle>,
    pgid: Option<sys::Pid>,
}

pub fn execute_program(shell: &mut Shell, program: &Program) -> Result<i32, ShellError> {
    let mut status = 0;
    for item in &program.items {
        status = execute_list_item(shell, item)?;
        shell.last_status = status;
        shell.run_pending_traps()?;
        if !shell.running || shell.has_pending_control() {
            break;
        }
    }
    Ok(status)
}

fn check_errexit(shell: &mut Shell, status: i32) {
    if status != 0 && shell.options.errexit && !shell.errexit_suppressed {
        shell.running = false;
        shell.last_status = status;
    }
}

fn execute_list_item(shell: &mut Shell, item: &ListItem) -> Result<i32, ShellError> {
    shell.lineno = item.line;
    shell.env.insert(
        b"LINENO".to_vec(),
        crate::bstr::u64_to_bytes(item.line as u64),
    );
    if item.asynchronous {
        let stdin_override = if !shell.options.monitor {
            Some(
                sys::open_file(b"/dev/null", sys::O_RDONLY, 0)
                    .map_err(|e| shell.diagnostic_syserr(1, &e))?,
            )
        } else {
            None
        };
        let spawned = spawn_and_or(shell, &item.and_or, stdin_override)?;
        let last_pid = spawned.children.last().map(|c| c.pid).unwrap_or(0);
        let description = render_and_or(&item.and_or);
        let id = shell.register_background_job(description.into(), spawned.pgid, spawned.children);
        if shell.interactive {
            let msg = ByteWriter::new()
                .byte(b'[')
                .usize_val(id)
                .bytes(b"] ")
                .i64_val(last_pid as i64)
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        }
        Ok(0)
    } else {
        execute_and_or(shell, &item.and_or)
    }
}

fn execute_and_or(shell: &mut Shell, node: &AndOr) -> Result<i32, ShellError> {
    if node.rest.is_empty() {
        let saved_suppressed = shell.errexit_suppressed;
        if node.first.negated {
            shell.errexit_suppressed = true;
        }
        let status = execute_pipeline(shell, &node.first, false)?;
        shell.errexit_suppressed = saved_suppressed;
        if !node.first.negated {
            check_errexit(shell, status);
        }
        return Ok(status);
    }
    let saved_suppressed = shell.errexit_suppressed;
    shell.errexit_suppressed = true;
    let mut status = execute_pipeline(shell, &node.first, false)?;
    for (i, (op, pipeline)) in node.rest.iter().enumerate() {
        let is_last = i == node.rest.len() - 1;
        if is_last {
            shell.errexit_suppressed = saved_suppressed;
            if pipeline.negated {
                shell.errexit_suppressed = true;
            }
        }
        match op {
            LogicalOp::And if status == 0 => status = execute_pipeline(shell, pipeline, false)?,
            LogicalOp::Or if status != 0 => status = execute_pipeline(shell, pipeline, false)?,
            _ => {}
        }
    }
    shell.errexit_suppressed = saved_suppressed;
    let last_pipeline = node.rest.last().map(|(_, p)| p).unwrap_or(&node.first);
    if !last_pipeline.negated {
        check_errexit(shell, status);
    }
    Ok(status)
}

fn spawn_and_or(
    shell: &mut Shell,
    node: &AndOr,
    stdin_override: Option<i32>,
) -> Result<SpawnedProcesses, ShellError> {
    let ignore_int_quit = !shell.options.monitor;
    if node.rest.is_empty() && !ignore_int_quit {
        return spawn_pipeline(shell, &node.first, stdin_override);
    }
    let pid = sys::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
    if pid == 0 {
        if let Some(fd) = stdin_override {
            let _ = sys::duplicate_fd(fd, sys::STDIN_FILENO);
            let _ = sys::close_fd(fd);
        }
        let _ = sys::set_process_group(0, 0);
        if ignore_int_quit {
            let _ = sys::ignore_signal(sys::SIGINT);
            let _ = sys::ignore_signal(sys::SIGQUIT);
        }
        let mut child_shell = shell.clone();
        child_shell.owns_terminal = false;
        child_shell.in_subshell = true;
        child_shell.restore_signals_for_child();
        let _ = child_shell.reset_traps_for_subshell();
        let status = if node.rest.is_empty() {
            execute_pipeline(&mut child_shell, &node.first, false).unwrap_or(1)
        } else {
            execute_and_or(&mut child_shell, node).unwrap_or(1)
        };
        let status = child_shell.run_exit_trap(status).unwrap_or(status);
        sys::exit_process(status as sys::RawFd);
    }
    if let Some(fd) = stdin_override {
        let _ = sys::close_fd(fd);
    }
    let _ = sys::set_process_group(pid, pid);
    Ok(SpawnedProcesses {
        children: vec![sys::ChildHandle {
            pid,
            stdout_fd: None,
        }],
        pgid: Some(pid),
    })
}

fn execute_pipeline(
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

fn execute_pipeline_inner(
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

struct TimeSnapshot {
    wall_ns: u64,
    user_ticks: u64,
    sys_ticks: u64,
    child_user_ticks: u64,
    child_sys_ticks: u64,
    ticks_per_sec: u64,
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

fn write_time_report(before: &TimeSnapshot, mode: TimedMode) {
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

fn fork_and_execute_command(
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

fn spawn_pipeline(
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

fn wait_for_pipeline(
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
fn wait_for_children(
    shell: &mut Shell,
    spawned: SpawnedProcesses,
    command_desc: Option<&[u8]>,
) -> Result<i32, ShellError> {
    let (last_status, _) = wait_for_children_inner(shell, spawned, command_desc)?;
    Ok(last_status)
}

fn wait_for_children_inner(
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

fn wait_for_external_child(
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

fn execute_command(shell: &mut Shell, command: &Command) -> Result<i32, ShellError> {
    execute_command_inner(shell, command, false)
}

fn execute_command_in_pipeline_child(
    shell: &mut Shell,
    command: &Command,
) -> Result<i32, ShellError> {
    execute_command_inner(shell, command, true)
}

fn execute_command_inner(
    shell: &mut Shell,
    command: &Command,
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    match command {
        Command::Simple(simple) => execute_simple(shell, simple, allow_exec_in_place),
        Command::Subshell(program) => {
            let pid = sys::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
            if pid == 0 {
                let mut child_shell = shell.clone();
                child_shell.owns_terminal = false;
                child_shell.in_subshell = true;
                child_shell.restore_signals_for_child();
                let _ = child_shell.reset_traps_for_subshell();
                let status = match execute_nested_program(&mut child_shell, program) {
                    Ok(s) => s,
                    Err(error) => error.exit_status(),
                };
                let status = child_shell.run_exit_trap(status).unwrap_or(status);
                sys::exit_process(status as sys::RawFd);
            }
            let ws = loop {
                match sys::wait_pid(pid, false) {
                    Ok(Some(ws)) => break ws,
                    Ok(None) => continue,
                    Err(e) if e.is_eintr() => continue,
                    Err(e) => return Err(shell.diagnostic_syserr(1, &e)),
                }
            };
            Ok(sys::decode_wait_status(ws.status))
        }
        Command::Group(program) => execute_nested_program(shell, program),
        Command::FunctionDef(function) => {
            shell
                .functions
                .insert(function.name.to_vec(), (*function.body).clone());
            Ok(0)
        }
        Command::If(if_command) => execute_if(shell, if_command),
        Command::Loop(loop_command) => execute_loop(shell, loop_command),
        Command::For(for_command) => execute_for(shell, for_command),
        Command::Case(case_command) => execute_case(shell, case_command),
        Command::Redirected(command, redirections) => {
            execute_redirected(shell, command, redirections, allow_exec_in_place)
        }
    }
}

fn execute_redirected(
    shell: &mut Shell,
    command: &Command,
    redirections: &[crate::syntax::Redirection],
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    let arena = ByteArena::new();
    let expanded = expand_redirections(shell, redirections, &arena)?;
    if let Some(first) = expanded.first() {
        shell.lineno = first.line;
    }
    let guard = match apply_shell_redirections(&expanded, shell.options.noclobber) {
        Ok(guard) => guard,
        Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
    };
    let result = execute_command_inner(shell, command, allow_exec_in_place);
    drop(guard);
    result
}

fn execute_if(shell: &mut Shell, if_command: &IfCommand) -> Result<i32, ShellError> {
    let saved_suppressed = shell.errexit_suppressed;
    shell.errexit_suppressed = true;
    let cond = execute_nested_program(shell, &if_command.condition)?;
    shell.errexit_suppressed = saved_suppressed;
    if cond == 0 {
        return execute_nested_program(shell, &if_command.then_branch);
    }

    for branch in &if_command.elif_branches {
        shell.errexit_suppressed = true;
        let cond = execute_nested_program(shell, &branch.condition)?;
        shell.errexit_suppressed = saved_suppressed;
        if cond == 0 {
            return execute_nested_program(shell, &branch.body);
        }
    }

    if let Some(else_branch) = &if_command.else_branch {
        execute_nested_program(shell, else_branch)
    } else {
        Ok(0)
    }
}

fn execute_loop(shell: &mut Shell, loop_command: &LoopCommand) -> Result<i32, ShellError> {
    shell.loop_depth += 1;
    let result = (|| {
        let mut last_status = 0;
        loop {
            shell.run_pending_traps()?;
            let saved_suppressed = shell.errexit_suppressed;
            shell.errexit_suppressed = true;
            let condition_status = execute_nested_program(shell, &loop_command.condition)?;
            shell.errexit_suppressed = saved_suppressed;
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
            let should_run = match loop_command.kind {
                LoopKind::While => condition_status == 0,
                LoopKind::Until => condition_status != 0,
            };
            if !should_run {
                break;
            }
            last_status = execute_nested_program(shell, &loop_command.body)?;
            if !shell.running {
                break;
            }
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
        }
        Ok(last_status)
    })();
    shell.loop_depth = shell.loop_depth.saturating_sub(1);
    result
}

fn execute_for(shell: &mut Shell, for_command: &ForCommand) -> Result<i32, ShellError> {
    let arena = ByteArena::new();
    let values: Vec<Vec<u8>> = if let Some(items) = &for_command.items {
        let mut values = Vec::new();
        for item in items {
            for s in expand::expand_word(shell, item, &arena).map_err(|e| shell.expand_to_err(e))? {
                values.push(s.to_vec());
            }
        }
        values
    } else {
        shell.positional.clone()
    };

    shell.loop_depth += 1;
    let result = (|| {
        let mut last_status = 0;
        for value in values {
            shell.set_var(&for_command.name, value).map_err(|e| {
                let msg = var_error_bytes(&e);
                shell.diagnostic(1, &msg)
            })?;
            last_status = execute_nested_program(shell, &for_command.body)?;
            if !shell.running {
                break;
            }
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
        }
        Ok(last_status)
    })();
    shell.loop_depth = shell.loop_depth.saturating_sub(1);
    result
}

fn execute_case(shell: &mut Shell, case_command: &CaseCommand) -> Result<i32, ShellError> {
    let arena = ByteArena::new();
    let word = expand::expand_word_text(shell, &case_command.word, &arena)
        .map_err(|e| shell.expand_to_err(e))?;
    let arms = &case_command.arms;
    let mut matched = false;
    for (i, arm) in arms.iter().enumerate() {
        if !matched {
            for pattern in &arm.patterns {
                let pattern = expand::expand_word_pattern(shell, pattern, &arena)
                    .map_err(|e| shell.expand_to_err(e))?;
                if case_pattern_matches(word, pattern) {
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            let status = execute_nested_program(shell, &arm.body)?;
            if !arm.fallthrough {
                return Ok(status);
            }
            if i + 1 >= arms.len() {
                return Ok(status);
            }
        }
    }
    Ok(0)
}

struct SavedVar {
    name: Box<[u8]>,
    value: Option<Vec<u8>>,
    was_exported: bool,
}

fn save_vars(shell: &Shell, assignments: &[(Vec<u8>, Vec<u8>)]) -> Vec<SavedVar> {
    assignments
        .iter()
        .map(|(name, _)| SavedVar {
            name: name.clone().into(),
            value: shell.get_var(name).map(|s| s.to_vec()),
            was_exported: shell.exported.contains(name),
        })
        .collect()
}

fn apply_prefix_assignments(
    shell: &mut Shell,
    assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<(), ShellError> {
    for (name, value) in assignments {
        shell.set_var(name, value.clone()).map_err(|e| {
            let msg = var_error_bytes(&e);
            shell.diagnostic(1, &msg)
        })?;
    }
    Ok(())
}

fn restore_vars(shell: &mut Shell, saved: Vec<SavedVar>) {
    for entry in saved {
        let name: Vec<u8> = entry.name.into();
        match entry.value {
            Some(v) => {
                shell.env.insert(name.clone(), v);
            }
            None => {
                shell.env.remove(&name);
            }
        }
        if entry.was_exported {
            shell.exported.insert(name);
        } else {
            shell.exported.remove(&name);
        }
    }
}

enum BuiltinResult {
    Status(i32),
    UtilityError(i32),
}

fn run_builtin_flow(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinResult, ShellError> {
    match shell.run_builtin(argv, assignments)? {
        FlowSignal::Continue(status) => Ok(BuiltinResult::Status(status)),
        FlowSignal::UtilityError(status) => Ok(BuiltinResult::UtilityError(status)),
        FlowSignal::Exit(status) => {
            shell.running = false;
            Ok(BuiltinResult::Status(status))
        }
    }
}

fn write_xtrace(shell: &mut Shell, expanded: &ExpandedSimpleCommand<'_>) {
    if !shell.options.xtrace {
        return;
    }
    let arena = ByteArena::new();
    let ps4_raw = shell.get_var(b"PS4").unwrap_or(b"+ ").to_vec();
    let prefix = expand::expand_parameter_text(shell, &ps4_raw, &arena).unwrap_or(b"+ ");
    let mut line = prefix.to_vec();
    for (name, value) in &expanded.assignments {
        line.extend_from_slice(name);
        line.push(b'=');
        line.extend_from_slice(value);
        line.push(b' ');
    }
    for (i, word) in expanded.argv.iter().enumerate() {
        if i > 0 {
            line.push(b' ');
        }
        line.extend_from_slice(word);
    }
    line.push(b'\n');
    let _ = sys::write_all_fd(sys::STDERR_FILENO, &line);
}

fn has_command_substitution(simple: &SimpleCommand) -> bool {
    simple.assignments.iter().any(|a| {
        let raw: &[u8] = &a.value.raw;
        raw.windows(2).any(|w| w == b"$(") || raw.contains(&b'`')
    }) || simple.words.iter().any(|w| {
        let raw: &[u8] = &w.raw;
        raw.windows(2).any(|w| w == b"$(") || raw.contains(&b'`')
    })
}

fn execute_simple(
    shell: &mut Shell,
    simple: &SimpleCommand,
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    let arena = ByteArena::new();
    let expanded = expand_simple(shell, simple, &arena)?;

    if let Some(first_word) = simple.words.first() {
        shell.lineno = first_word.line;
    }

    if !expanded.argv.is_empty() || !expanded.assignments.is_empty() {
        write_xtrace(shell, &expanded);
    }

    let owned_argv: Vec<Vec<u8>> = expanded.argv.iter().map(|s| s.to_vec()).collect();
    let owned_assignments: Vec<(Vec<u8>, Vec<u8>)> = expanded
        .assignments
        .iter()
        .map(|&(n, v)| (n.to_vec(), v.to_vec()))
        .collect();

    if expanded.argv.is_empty() {
        let cmd_sub_status = if has_command_substitution(simple) {
            shell.last_status
        } else {
            0
        };
        let guard = match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
        {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        for (name, value) in &owned_assignments {
            shell.set_var(name, value.clone()).map_err(|e| {
                let msg = var_error_bytes(&e);
                shell.diagnostic(1, &msg)
            })?;
        }
        drop(guard);
        return Ok(cmd_sub_status);
    }

    let is_special_builtin = builtin::is_special_builtin(&owned_argv[0]);
    let is_exec_no_cmd = is_special_builtin
        && owned_argv[0] == b"exec"
        && !owned_argv.iter().skip(1).any(|a| a == b"--");

    if is_exec_no_cmd {
        for redir in &expanded.redirections {
            shell.lineno = redir.line;
            apply_shell_redirection(redir, shell.options.noclobber)
                .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        }
        return match run_builtin_flow(shell, &owned_argv, &owned_assignments) {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    } else if is_special_builtin {
        let _guard = apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
            .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        let result = run_builtin_flow(shell, &owned_argv, &owned_assignments);
        drop(_guard);
        return match result {
            Ok(BuiltinResult::UtilityError(status)) if !shell.interactive => {
                Err(ShellError::Status(status))
            }
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    }

    if let Some(function) = shell.functions.get(&owned_argv[0]).cloned() {
        let guard = match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
        {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        let saved_vars = save_vars(shell, &owned_assignments);
        if let Err(e) = apply_prefix_assignments(shell, &owned_assignments) {
            restore_vars(shell, saved_vars);
            drop(guard);
            return Err(e);
        }
        let saved = std::mem::replace(&mut shell.positional, owned_argv[1..].to_vec());
        shell.function_depth += 1;
        let status = execute_command(shell, &function);
        shell.function_depth = shell.function_depth.saturating_sub(1);
        shell.positional = saved;
        restore_vars(shell, saved_vars);
        drop(guard);
        match status {
            Ok(status) => match shell.pending_control {
                Some(PendingControl::Return(return_status)) => {
                    shell.pending_control = None;
                    Ok(return_status)
                }
                _ => Ok(status),
            },
            Err(error) => Err(error),
        }
    } else if builtin::is_builtin(&owned_argv[0]) {
        let saved_vars = save_vars(shell, &owned_assignments);
        let assign_result = apply_prefix_assignments(shell, &owned_assignments);
        let result = match assign_result {
            Ok(()) => {
                let r =
                    match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
                    {
                        Ok(guard) => {
                            let r = run_builtin_flow(shell, &owned_argv, &[]);
                            drop(guard);
                            r
                        }
                        Err(e) => Err(shell.diagnostic_syserr(1, &e)),
                    };
                restore_vars(shell, saved_vars);
                r
            }
            Err(error) => {
                restore_vars(shell, saved_vars);
                Err(error)
            }
        };
        match result {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Ok(error.exit_status()),
        }
    } else {
        for (name, _value) in &owned_assignments {
            if shell.readonly.contains(name) {
                return Err(shell.diagnostic(1, &{
                    let mut msg = name.clone();
                    msg.extend_from_slice(b": readonly variable");
                    msg
                }));
            }
        }
        let command_name = owned_argv[0].clone();
        let prepared = build_process_from_expanded(shell, expanded, owned_argv, owned_assignments)
            .expect("argv is non-empty");
        if !prepared.path_verified && !prepared.exec_path.contains(&b'/') {
            let _guard = apply_shell_redirections(&prepared.redirections, prepared.noclobber).ok();
            let msg = ByteWriter::new()
                .bytes(&command_name)
                .bytes(b": not found\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            return Ok(127);
        }
        let desc = join_boxed_bytes(&prepared.argv, b' ');
        if allow_exec_in_place {
            exec_prepared_in_current_process(shell, &prepared, ProcessGroupPlan::None)
        } else if shell.in_subshell {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::None) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let status = wait_for_external_child(shell, &handle, None, Some(&desc))?;
            Ok(status)
        } else {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::NewGroup) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let pgid = handle.pid;
            let _ = sys::set_process_group(pgid, pgid);
            let status = wait_for_external_child(shell, &handle, Some(pgid), Some(&desc))?;
            Ok(status)
        }
    }
}

fn join_boxed_bytes(parts: &[Box<[u8]>], sep: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.extend_from_slice(part);
    }
    out
}

#[derive(Debug)]
struct ExpandedSimpleCommand<'a> {
    assignments: Vec<(&'a [u8], &'a [u8])>,
    argv: Vec<&'a [u8]>,
    redirections: Vec<ExpandedRedirection<'a>>,
}

#[derive(Clone, Debug)]
struct ExpandedRedirection<'a> {
    fd: i32,
    kind: RedirectionKind,
    target: &'a [u8],
    here_doc_body: Option<&'a [u8]>,
    line: usize,
}

#[derive(Debug, Clone)]
struct ProcessRedirection {
    fd: i32,
    kind: RedirectionKind,
    target: Box<[u8]>,
    here_doc_body: Option<Box<[u8]>>,
}

#[derive(Debug, Clone)]
struct PreparedProcess {
    exec_path: Box<[u8]>,
    argv: Box<[Box<[u8]>]>,
    child_env: Box<[(Box<[u8]>, Box<[u8]>)]>,
    redirections: Vec<ProcessRedirection>,
    noclobber: bool,
    path_verified: bool,
}

trait RedirectionRef {
    fn fd(&self) -> i32;
    fn kind(&self) -> RedirectionKind;
    fn target(&self) -> &[u8];
    fn here_doc_body(&self) -> Option<&[u8]>;
}

impl<'a> RedirectionRef for ExpandedRedirection<'a> {
    fn fd(&self) -> i32 {
        self.fd
    }
    fn kind(&self) -> RedirectionKind {
        self.kind
    }
    fn target(&self) -> &[u8] {
        self.target
    }
    fn here_doc_body(&self) -> Option<&[u8]> {
        self.here_doc_body
    }
}

impl RedirectionRef for ProcessRedirection {
    fn fd(&self) -> i32 {
        self.fd
    }
    fn kind(&self) -> RedirectionKind {
        self.kind
    }
    fn target(&self) -> &[u8] {
        &self.target
    }
    fn here_doc_body(&self) -> Option<&[u8]> {
        self.here_doc_body.as_deref()
    }
}

#[derive(Debug)]
enum ChildFdAction {
    DupRawFd {
        fd: i32,
        target_fd: i32,
        close_source: bool,
    },
    DupFd {
        source_fd: i32,
        target_fd: i32,
    },
    CloseFd {
        target_fd: i32,
    },
}

#[derive(Debug, Default)]
struct PreparedRedirections {
    actions: Vec<ChildFdAction>,
}

#[derive(Debug)]
struct ShellRedirectionGuard {
    saved: Vec<(i32, Option<i32>)>,
}

fn is_declaration_utility(name: &[u8]) -> bool {
    name == b"export" || name == b"readonly"
}

fn find_declaration_context(words: &[crate::syntax::Word]) -> bool {
    let mut i = 0;
    while i < words.len() {
        let raw: &[u8] = &words[i].raw;
        if raw == b"command" {
            i += 1;
            while i < words.len() && words[i].raw.starts_with(b"-") {
                i += 1;
            }
            continue;
        }
        return is_declaration_utility(raw);
    }
    false
}

fn expand_simple<'a>(
    shell: &mut Shell,
    simple: &SimpleCommand,
    arena: &'a ByteArena,
) -> Result<ExpandedSimpleCommand<'a>, ShellError> {
    let mut assignments = Vec::new();
    for assignment in &simple.assignments {
        let value = expand::expand_assignment_value(shell, &assignment.value, arena)
            .map_err(|e| shell.expand_to_err(e))?;
        assignments.push((arena.intern_bytes(&assignment.name), value));
    }

    let declaration_ctx = find_declaration_context(&simple.words);
    let argv = if declaration_ctx {
        expand_words_declaration(shell, &simple.words, arena)?
    } else {
        expand::expand_words(shell, &simple.words, arena).map_err(|e| shell.expand_to_err(e))?
    };
    let mut redirections = Vec::new();
    for redirection in &simple.redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                expand::expand_here_document(shell, &here_doc.body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_bytes(&here_doc.body)
            };
            (arena.intern_bytes(&here_doc.delimiter), Some(body))
        } else {
            let target = expand::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        redirections.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }

    Ok(ExpandedSimpleCommand {
        assignments,
        argv,
        redirections,
    })
}

fn parse_i32_bytes(s: &[u8]) -> Option<i32> {
    crate::bstr::parse_i64(s).and_then(|v| i32::try_from(v).ok())
}

fn expand_words_declaration<'a>(
    shell: &mut Shell,
    words: &[crate::syntax::Word],
    arena: &'a ByteArena,
) -> Result<Vec<&'a [u8]>, ShellError> {
    let mut result = Vec::new();
    let mut found_cmd = false;
    for word in words {
        if !found_cmd {
            result.extend(
                expand::expand_word(shell, word, arena).map_err(|e| shell.expand_to_err(e))?,
            );
            if result
                .last()
                .is_some_and(|s: &&[u8]| !s.is_empty() && *s != b"command")
            {
                found_cmd = true;
            }
        } else if expand::word_is_assignment(&word.raw) {
            result.push(
                expand::expand_word_as_declaration_assignment(shell, word, arena)
                    .map_err(|e| shell.expand_to_err(e))?,
            );
        } else {
            result.extend(
                expand::expand_word(shell, word, arena).map_err(|e| shell.expand_to_err(e))?,
            );
        }
    }
    Ok(result)
}

fn expand_redirections<'a>(
    shell: &mut Shell,
    redirections: &[crate::syntax::Redirection],
    arena: &'a ByteArena,
) -> Result<Vec<ExpandedRedirection<'a>>, ShellError> {
    let mut expanded_vec = Vec::new();
    for redirection in redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                expand::expand_here_document(shell, &here_doc.body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_bytes(&here_doc.body)
            };
            (arena.intern_bytes(&here_doc.delimiter), Some(body))
        } else {
            let target = expand::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        expanded_vec.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }
    Ok(expanded_vec)
}

fn build_process_from_expanded(
    shell: &Shell,
    expanded: ExpandedSimpleCommand<'_>,
    owned_argv: Vec<Vec<u8>>,
    owned_assignments: Vec<(Vec<u8>, Vec<u8>)>,
) -> Result<PreparedProcess, ShellError> {
    let program = expanded
        .argv
        .first()
        .ok_or_else(|| shell.diagnostic(1, b"empty command" as &[u8]))?;
    let prefix_path = expanded
        .assignments
        .iter()
        .find(|&&(name, _)| name == b"PATH")
        .map(|&(_, value)| value);
    let resolved = resolve_command_path(shell, program, prefix_path);
    let path_verified = resolved.is_some();
    let exec_path: Vec<u8> = resolved.unwrap_or_else(|| program.to_vec());
    let mut child_env = shell.env_for_child();
    child_env.extend(owned_assignments);
    let redirections = expanded
        .redirections
        .into_iter()
        .map(|r| ProcessRedirection {
            fd: r.fd,
            kind: r.kind,
            target: r.target.to_vec().into(),
            here_doc_body: r.here_doc_body.map(|s| s.to_vec().into()),
        })
        .collect();
    Ok(PreparedProcess {
        exec_path: exec_path.into(),
        argv: owned_argv
            .into_iter()
            .map(Vec::into_boxed_slice)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        child_env: child_env
            .into_iter()
            .map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        redirections,
        noclobber: shell.options.noclobber,
        path_verified,
    })
}

fn file_needs_binary_rejection(path: &[u8]) -> bool {
    let fd = match sys::open_file(path, sys::O_RDONLY | sys::O_CLOEXEC, 0) {
        Ok(fd) => fd,
        Err(_) => return false,
    };
    let mut buf = [0u8; 256];
    let n = match sys::read_fd(fd, &mut buf) {
        Ok(n) => n,
        Err(_) => {
            let _ = sys::close_fd(fd);
            return false;
        }
    };
    let _ = sys::close_fd(fd);
    if n == 0 {
        return false;
    }
    let prefix = &buf[..n];
    if n >= 4 && prefix[0] == 0x7f && prefix[1] == b'E' {
        return false;
    }
    if n >= 2 && prefix[0] == b'#' && prefix[1] == b'!' {
        return false;
    }
    let nl_pos = prefix.iter().position(|&b| b == b'\n').unwrap_or(n);
    prefix[..nl_pos].contains(&0)
}

fn prepare_prepared_process(
    shell: &Shell,
    prepared: &PreparedProcess,
) -> Result<PreparedRedirections, ShellError> {
    if !prepared.path_verified
        && !prepared.exec_path.is_empty()
        && prepared.exec_path.contains(&b'/')
    {
        if sys::access_path(&prepared.exec_path, sys::F_OK).is_err() {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": not found\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            return Err(ShellError::Status(127));
        }
        if sys::access_path(&prepared.exec_path, sys::X_OK).is_err() {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": Permission denied\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            return Err(ShellError::Status(126));
        }
    }

    prepare_redirections(&prepared.redirections, prepared.noclobber)
        .map_err(|e| shell.diagnostic_syserr(1, &e))
}

fn run_prepared_process(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
    prepared_redirections: &PreparedRedirections,
) -> ! {
    match process_group {
        ProcessGroupPlan::NewGroup => {
            let _ = sys::set_process_group(0, 0);
        }
        ProcessGroupPlan::Join(pgid) => {
            let _ = sys::set_process_group(0, pgid);
        }
        ProcessGroupPlan::None => {}
    }
    if let Err(err) = apply_child_fd_actions(&prepared_redirections.actions) {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": ")
            .bytes(&err.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        sys::exit_process(1);
    }

    for (key, value) in &prepared.child_env {
        let _ = sys::env_set_var(key, value);
    }

    if file_needs_binary_rejection(&prepared.exec_path) {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": cannot execute binary file\n")
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        sys::exit_process(126);
    }

    shell.restore_signals_for_child();
    match sys::exec_replace(&prepared.exec_path, &prepared.argv) {
        Err(err) if err.is_enoexec() => {
            let mut child_shell = shell.clone();
            child_shell.owns_terminal = false;
            child_shell.in_subshell = true;
            let _ = child_shell.reset_traps_for_subshell();
            child_shell.shell_name = prepared.argv[0].clone();
            child_shell.positional = prepared.argv[1..].iter().map(|s| s.to_vec()).collect();
            let status = child_shell.source_path(&prepared.exec_path).unwrap_or(126);
            sys::exit_process(status as sys::RawFd);
        }
        Err(err) if err.is_enoent() => {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": not found\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            sys::exit_process(127);
        }
        Err(err) => {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": ")
                .bytes(&err.strerror())
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            sys::exit_process(126);
        }
        Ok(()) => sys::exit_process(0),
    }
}

fn exec_prepared_in_current_process(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
) -> ! {
    let prepared_redirections = match prepare_prepared_process(shell, prepared) {
        Ok(prepared_redirections) => prepared_redirections,
        Err(error) => sys::exit_process(error.exit_status() as sys::RawFd),
    };
    run_prepared_process(shell, prepared, process_group, &prepared_redirections)
}

fn close_parent_redirection_fds(prepared_redirections: &PreparedRedirections) {
    for action in &prepared_redirections.actions {
        if let ChildFdAction::DupRawFd {
            fd,
            close_source: true,
            ..
        } = action
        {
            let _ = sys::close_fd(*fd);
        }
    }
}

fn spawn_prepared(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    let prepared_redirections = prepare_prepared_process(shell, prepared)?;

    let pid = sys::fork_process().map_err(|e| {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": ")
            .bytes(&e.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        ShellError::Status(1)
    })?;
    if pid == 0 {
        run_prepared_process(shell, prepared, process_group, &prepared_redirections);
    }

    close_parent_redirection_fds(&prepared_redirections);

    Ok(sys::ChildHandle {
        pid,
        stdout_fd: None,
    })
}

fn prepare_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<PreparedRedirections, sys::SysError> {
    let mut prepared = PreparedRedirections::default();
    for redirection in redirections {
        match redirection.kind() {
            RedirectionKind::Read => {
                let fd = sys::open_file(redirection.target(), sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::Write | RedirectionKind::ClobberWrite => {
                let fd = if noclobber && redirection.kind() == RedirectionKind::Write {
                    open_for_write_noclobber(redirection.target())?
                } else {
                    let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC;
                    sys::open_file(redirection.target(), flags, 0o666)?
                };
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::Append => {
                let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
                let fd = sys::open_file(redirection.target(), flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::HereDoc => {
                let (read_fd, write_fd) = sys::create_pipe()?;
                let body = redirection.here_doc_body().unwrap_or(b"");
                sys::write_all_fd(write_fd, body)?;
                sys::close_fd(write_fd)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd: read_fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::ReadWrite => {
                let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
                let fd = sys::open_file(redirection.target(), flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::DupInput | RedirectionKind::DupOutput => {
                if redirection.target() == b"-" {
                    prepared.actions.push(ChildFdAction::CloseFd {
                        target_fd: redirection.fd(),
                    });
                } else {
                    let source_fd =
                        parse_i32_bytes(redirection.target()).expect("validated at expansion");
                    prepared.actions.push(ChildFdAction::DupFd {
                        source_fd,
                        target_fd: redirection.fd(),
                    });
                }
            }
        }
    }
    Ok(prepared)
}

fn resolve_command_path(
    shell: &Shell,
    program: &[u8],
    path_override: Option<&[u8]>,
) -> Option<Vec<u8>> {
    if program.contains(&b'/') {
        return Some(program.to_vec());
    }

    if path_override.is_none()
        && let Some(cached) = shell.path_cache.get(program)
    {
        return Some(cached.clone());
    }

    let path = path_override
        .map(|s| s.to_vec())
        .or_else(|| shell.get_var(b"PATH").map(|s| s.to_vec()))
        .or_else(|| sys::env_var(b"PATH"))
        .unwrap_or_default();

    split_bytes(&path, b':')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut candidate = segment.to_vec();
            candidate.push(b'/');
            candidate.extend_from_slice(program);
            candidate
        })
        .find(|candidate| {
            sys::stat_path(candidate)
                .map(|stat| stat.is_regular_file() && stat.is_executable())
                .unwrap_or(false)
        })
}

fn split_bytes(data: &[u8], sep: u8) -> impl Iterator<Item = &[u8]> {
    data.split(move |&b| b == sep)
}

fn apply_child_fd_actions(actions: &[ChildFdAction]) -> sys::SysResult<()> {
    for action in actions {
        match action {
            ChildFdAction::DupRawFd { fd, target_fd, .. } => {
                sys::duplicate_fd(*fd, *target_fd)?;
            }
            ChildFdAction::DupFd {
                source_fd,
                target_fd,
            } => {
                sys::duplicate_fd(*source_fd, *target_fd)?;
            }
            ChildFdAction::CloseFd { target_fd } => {
                if let Err(error) = sys::close_fd(*target_fd) {
                    if !error.is_ebadf() {
                        return Err(error);
                    }
                }
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn apply_child_setup(
    actions: &[ChildFdAction],
    process_group: ProcessGroupPlan,
) -> sys::SysResult<()> {
    apply_child_fd_actions(actions)?;
    match process_group {
        ProcessGroupPlan::None => {}
        ProcessGroupPlan::NewGroup => sys::set_process_group(0, 0)?,
        ProcessGroupPlan::Join(pgid) => sys::set_process_group(0, pgid)?,
    }
    Ok(())
}

fn handoff_foreground(pgid: Option<sys::Pid>) -> Option<sys::Pid> {
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

fn restore_foreground(saved_foreground: Option<sys::Pid>) {
    if let Some(pgid) = saved_foreground {
        let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
    }
}

impl Drop for ShellRedirectionGuard {
    fn drop(&mut self) {
        for (target_fd, saved_fd) in self.saved.iter().rev() {
            match saved_fd {
                Some(saved_fd) => {
                    let _ = sys::duplicate_fd(*saved_fd, *target_fd);
                    let _ = sys::close_fd(*saved_fd);
                }
                None => {
                    let _ = sys::close_fd(*target_fd);
                }
            }
        }
    }
}

fn apply_shell_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<ShellRedirectionGuard, sys::SysError> {
    let mut guard = ShellRedirectionGuard { saved: Vec::new() };
    let mut saved = HashMap::new();

    for redirection in redirections {
        if let std::collections::hash_map::Entry::Vacant(entry) = saved.entry(redirection.fd()) {
            let original = match sys::duplicate_fd_to_new(redirection.fd()) {
                Ok(fd) => Some(fd),
                Err(error) if error.is_ebadf() => None,
                Err(error) => return Err(error),
            };
            entry.insert(original);
            guard.saved.push((redirection.fd(), original));
        }
        apply_shell_redirection(redirection, noclobber)?;
    }

    Ok(guard)
}

fn open_for_write_noclobber(path: &[u8]) -> sys::SysResult<i32> {
    let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC;
    match sys::open_file(path, flags, 0o666) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == Some(sys::EEXIST) => {
            if sys::is_regular_file(path) {
                Err(e)
            } else {
                sys::open_file(path, sys::O_WRONLY | sys::O_CLOEXEC, 0o666)
            }
        }
        Err(e) => Err(e),
    }
}

fn apply_shell_redirection<R: RedirectionRef>(
    redirection: &R,
    noclobber: bool,
) -> sys::SysResult<()> {
    match redirection.kind() {
        RedirectionKind::Read => {
            let fd = sys::open_file(redirection.target(), sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Write | RedirectionKind::ClobberWrite => {
            let fd = if noclobber && redirection.kind() == RedirectionKind::Write {
                open_for_write_noclobber(redirection.target())?
            } else {
                let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC;
                sys::open_file(redirection.target(), flags, 0o666)?
            };
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Append => {
            let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::create_pipe()?;
            let body = redirection.here_doc_body().unwrap_or(b"");
            sys::write_all_fd(write_fd, body)?;
            sys::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd())?;
        }
        RedirectionKind::ReadWrite => {
            let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::DupInput | RedirectionKind::DupOutput => {
            if redirection.target() == b"-" {
                close_shell_fd(redirection.fd())?;
            } else {
                let source_fd =
                    parse_i32_bytes(redirection.target()).expect("validated at expansion");
                sys::duplicate_fd(source_fd, redirection.fd())?;
            }
        }
    }
    Ok(())
}

fn replace_shell_fd(fd: i32, target_fd: i32) -> sys::SysResult<()> {
    if fd == target_fd {
        return Ok(());
    }
    sys::duplicate_fd(fd, target_fd)?;
    sys::close_fd(fd)?;
    Ok(())
}

fn close_shell_fd(target_fd: i32) -> sys::SysResult<()> {
    if let Err(error) = sys::close_fd(target_fd) {
        if !error.is_ebadf() {
            return Err(error);
        }
    }
    Ok(())
}

fn default_fd_for_redirection(kind: RedirectionKind) -> i32 {
    match kind {
        RedirectionKind::Read
        | RedirectionKind::HereDoc
        | RedirectionKind::ReadWrite
        | RedirectionKind::DupInput => 0,
        RedirectionKind::Write
        | RedirectionKind::ClobberWrite
        | RedirectionKind::Append
        | RedirectionKind::DupOutput => 1,
    }
}

fn case_pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
    expand::pattern_matches(text, pattern)
}

#[cfg(test)]
fn render_program(program: &Program) -> Vec<u8> {
    let mut buf = Vec::new();
    render_program_into(program, &mut buf);
    buf
}

fn render_program_into(program: &Program, buf: &mut Vec<u8>) {
    for (index, item) in program.items.iter().enumerate() {
        if index > 0 {
            buf.push(b'\n');
        }
        render_list_item_into(item, buf);
    }
}

#[cfg(test)]
fn render_list_item(item: &ListItem) -> Vec<u8> {
    let mut buf = Vec::new();
    render_list_item_into(item, &mut buf);
    buf
}

fn render_list_item_into(item: &ListItem, buf: &mut Vec<u8>) {
    render_and_or_into(&item.and_or, buf);
    if item.asynchronous {
        buf.extend_from_slice(b" &");
    }
}

fn render_and_or(and_or: &AndOr) -> Vec<u8> {
    let mut buf = Vec::new();
    render_and_or_into(and_or, &mut buf);
    buf
}

fn render_and_or_into(and_or: &AndOr, buf: &mut Vec<u8>) {
    render_pipeline_into(&and_or.first, buf);
    for (op, pipeline) in &and_or.rest {
        match op {
            LogicalOp::And => buf.extend_from_slice(b" && "),
            LogicalOp::Or => buf.extend_from_slice(b" || "),
        }
        render_pipeline_into(pipeline, buf);
    }
}

fn execute_nested_program(shell: &mut Shell, program: &Program) -> Result<i32, ShellError> {
    let mut status = 0;
    for item in &program.items {
        shell.run_pending_traps()?;
        status = execute_list_item(shell, item)?;
        shell.last_status = status;
        if !shell.running || shell.has_pending_control() {
            break;
        }
    }
    Ok(status)
}

fn render_command(command: &Command) -> Vec<u8> {
    let mut buf = Vec::new();
    render_command_into(command, &mut buf);
    buf
}

fn render_command_into(command: &Command, buf: &mut Vec<u8>) {
    match command {
        Command::Simple(simple) => render_simple_into(simple, buf),
        Command::Subshell(program) => {
            buf.push(b'(');
            render_program_into(program, buf);
            buf.push(b')');
        }
        Command::Group(program) => {
            buf.extend_from_slice(b"{ ");
            render_program_into(program, buf);
            buf.extend_from_slice(b"; }");
        }
        Command::FunctionDef(function) => render_function_into(function, buf),
        Command::If(if_command) => render_if_into(if_command, buf),
        Command::Loop(loop_command) => render_loop_into(loop_command, buf),
        Command::For(for_command) => render_for_into(for_command, buf),
        Command::Case(case_command) => render_case_into(case_command, buf),
        Command::Redirected(command, redirections) => {
            render_redirected_command_into(command, redirections, buf);
        }
    }
}

fn render_pipeline(pipeline: &Pipeline) -> Vec<u8> {
    let mut buf = Vec::new();
    render_pipeline_into(pipeline, &mut buf);
    buf
}

fn render_pipeline_into(pipeline: &Pipeline, buf: &mut Vec<u8>) {
    if pipeline.negated {
        buf.extend_from_slice(b"! ");
    }
    for (i, command) in pipeline.commands.iter().enumerate() {
        if i > 0 {
            buf.extend_from_slice(b" | ");
        }
        render_command_into(command, buf);
    }
}

#[cfg(test)]
fn render_function(function: &FunctionDef) -> Vec<u8> {
    let mut buf = Vec::new();
    render_function_into(function, &mut buf);
    buf
}

fn render_function_into(function: &FunctionDef, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&function.name);
    buf.extend_from_slice(b"() ");
    render_pipeline_into(
        &Pipeline {
            negated: false,
            timed: TimedMode::Off,
            commands: vec![(*function.body).clone()].into_boxed_slice(),
        },
        buf,
    );
}

#[cfg(test)]
fn render_if(if_command: &IfCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_if_into(if_command, &mut buf);
    buf
}

fn render_if_into(if_command: &IfCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"if ");
    render_program_into(&if_command.condition, buf);
    buf.extend_from_slice(b"\nthen\n");
    render_program_into(&if_command.then_branch, buf);
    for branch in &if_command.elif_branches {
        buf.extend_from_slice(b"\nelif ");
        render_program_into(&branch.condition, buf);
        buf.extend_from_slice(b"\nthen\n");
        render_program_into(&branch.body, buf);
    }
    if let Some(else_branch) = &if_command.else_branch {
        buf.extend_from_slice(b"\nelse\n");
        render_program_into(else_branch, buf);
    }
    buf.extend_from_slice(b"\nfi");
}

#[cfg(test)]
fn render_loop(loop_command: &LoopCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_loop_into(loop_command, &mut buf);
    buf
}

fn render_loop_into(loop_command: &LoopCommand, buf: &mut Vec<u8>) {
    let keyword = match loop_command.kind {
        LoopKind::While => b"while" as &[u8],
        LoopKind::Until => b"until" as &[u8],
    };
    buf.extend_from_slice(keyword);
    buf.push(b' ');
    render_program_into(&loop_command.condition, buf);
    buf.extend_from_slice(b"\ndo\n");
    render_program_into(&loop_command.body, buf);
    buf.extend_from_slice(b"\ndone");
}

#[cfg(test)]
fn render_for(for_command: &ForCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_for_into(for_command, &mut buf);
    buf
}

fn render_for_into(for_command: &ForCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"for ");
    buf.extend_from_slice(&for_command.name);
    if let Some(items) = &for_command.items {
        buf.extend_from_slice(b" in");
        for item in items {
            buf.push(b' ');
            buf.extend_from_slice(&item.raw);
        }
    }
    buf.extend_from_slice(b"\ndo\n");
    render_program_into(&for_command.body, buf);
    buf.extend_from_slice(b"\ndone");
}

#[cfg(test)]
fn render_case(case_command: &CaseCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_case_into(case_command, &mut buf);
    buf
}

fn render_case_into(case_command: &CaseCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"case ");
    buf.extend_from_slice(&case_command.word.raw);
    buf.extend_from_slice(b" in");
    for arm in &case_command.arms {
        buf.push(b'\n');
        for (i, pattern) in arm.patterns.iter().enumerate() {
            if i > 0 {
                buf.extend_from_slice(b" | ");
            }
            buf.extend_from_slice(&pattern.raw);
        }
        buf.extend_from_slice(b")\n");
        render_program_into(&arm.body, buf);
        if arm.fallthrough {
            buf.extend_from_slice(b"\n;&");
        } else {
            buf.extend_from_slice(b"\n;;");
        }
    }
    buf.extend_from_slice(b"\nesac");
}

#[cfg(test)]
fn render_simple(simple: &SimpleCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_simple_into(simple, &mut buf);
    buf
}

fn render_simple_into(simple: &SimpleCommand, buf: &mut Vec<u8>) {
    let mut base = Vec::new();
    for (i, assignment) in simple.assignments.iter().enumerate() {
        if i > 0 {
            base.push(b' ');
        }
        base.extend_from_slice(&assignment.name);
        base.push(b'=');
        base.extend_from_slice(&assignment.value.raw);
    }
    for word in &simple.words {
        if !base.is_empty() {
            base.push(b' ');
        }
        base.extend_from_slice(&word.raw);
    }
    render_command_line_with_redirections_into(base, &simple.redirections, buf);
}

fn render_redirections_into(
    redirections: &[crate::syntax::Redirection],
    redir_buf: &mut Vec<u8>,
    heredocs: &mut Vec<Vec<u8>>,
) {
    for (i, redirection) in redirections.iter().enumerate() {
        if i > 0 {
            redir_buf.push(b' ');
        }
        render_redirection_operator_into(redirection, redir_buf);
        if let Some(here_doc) = &redirection.here_doc {
            heredocs.push(render_here_doc_body(here_doc));
        }
    }
}

fn render_redirection_operator_into(redirection: &crate::syntax::Redirection, buf: &mut Vec<u8>) {
    if let Some(fd) = redirection.fd {
        crate::bstr::push_i64(buf, fd as i64);
    }
    let op: &[u8] = match redirection.kind {
        RedirectionKind::Read => b"<",
        RedirectionKind::Write => b">",
        RedirectionKind::ClobberWrite => b">|",
        RedirectionKind::Append => b">>",
        RedirectionKind::HereDoc => {
            if redirection
                .here_doc
                .as_ref()
                .is_some_and(|here_doc| here_doc.strip_tabs)
            {
                b"<<-"
            } else {
                b"<<"
            }
        }
        RedirectionKind::ReadWrite => b"<>",
        RedirectionKind::DupInput => b"<&",
        RedirectionKind::DupOutput => b">&",
    };
    buf.extend_from_slice(op);
    buf.extend_from_slice(&redirection.target.raw);
}

fn render_here_doc_body(here_doc: &HereDoc) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&here_doc.body);
    if !here_doc.body.ends_with(b"\n") {
        out.push(b'\n');
    }
    out.extend_from_slice(&here_doc.delimiter);
    out
}

fn render_command_line_with_redirections_into(
    base: Vec<u8>,
    redirections: &[crate::syntax::Redirection],
    buf: &mut Vec<u8>,
) {
    let mut redir_text = Vec::new();
    let mut heredocs = Vec::new();
    render_redirections_into(redirections, &mut redir_text, &mut heredocs);
    buf.extend_from_slice(&base);
    if !redir_text.is_empty() {
        if !base.is_empty() {
            buf.push(b' ');
        }
        buf.extend_from_slice(&redir_text);
    }
    if !heredocs.is_empty() {
        buf.push(b'\n');
        for (i, hd) in heredocs.iter().enumerate() {
            if i > 0 {
                buf.push(b'\n');
            }
            buf.extend_from_slice(hd);
        }
    }
}

fn render_redirected_command_into(
    command: &Command,
    redirections: &[crate::syntax::Redirection],
    buf: &mut Vec<u8>,
) {
    let base = render_command(command);
    render_command_line_with_redirections_into(base, redirections, buf);
}

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{
        ArgMatcher, TraceEntry, TraceResult, assert_no_syscalls, run_trace, t, t_fork,
    };
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    fn parse_test(source: &str) -> Result<crate::syntax::Program, crate::syntax::ParseError> {
        crate::syntax::parse(source.as_bytes())
    }

    fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: b"meiksh".to_vec().into(),
            env: HashMap::new(),
            exported: BTreeSet::new(),
            readonly: BTreeSet::new(),
            aliases: HashMap::new(),
            functions: HashMap::new(),
            positional: Vec::new(),
            last_status: 0,
            last_background: None,
            running: true,
            jobs: Vec::new(),
            known_pid_statuses: HashMap::new(),
            known_job_statuses: HashMap::new(),
            trap_actions: BTreeMap::new(),
            ignored_on_entry: BTreeSet::new(),
            subshell_saved_traps: None,
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive: false,
            errexit_suppressed: false,
            owns_terminal: false,
            in_subshell: false,
            wait_was_interrupted: false,
            pid: 0,
            lineno: 0,
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
        }
    }

    #[test]
    fn execute_and_or_skips_rhs_when_guard_fails() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("true || false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);

            let program = parse_test("false && true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
        });
    }

    #[test]
    fn execute_pipeline_async_single_command() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(1000),
                    vec![t(
                        "setpgid",
                        vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                        TraceResult::Int(0),
                    )],
                ),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
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
            vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "write",
                            vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"ok".to_vec())],
                            TraceResult::Int(2),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1001),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(200), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "stat",
                            vec![ArgMatcher::Str("/usr/bin/wc".into()), ArgMatcher::Any],
                            TraceResult::StatFile(0o755),
                        ),
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/usr/bin/wc".into()),
                                ArgMatcher::Any,
                                ArgMatcher::Any,
                            ],
                            TraceResult::Fd(20),
                        ),
                        t(
                            "read",
                            vec![ArgMatcher::Fd(20), ArgMatcher::Any],
                            TraceResult::Bytes(b"#!/bin/sh\n".to_vec()),
                        ),
                        t("close", vec![ArgMatcher::Fd(20)], TraceResult::Int(0)),
                        t(
                            "execvp",
                            vec![ArgMatcher::Str("/usr/bin/wc".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(1000),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(1001),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
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
    fn build_process_from_expanded_covers_empty_and_assignment_env() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: empty command\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let arena = ByteArena::new();
                let shell = test_shell();
                let error = build_process_from_expanded(
                    &shell,
                    ExpandedSimpleCommand {
                        assignments: Vec::new(),

                        argv: Vec::new(),

                        redirections: Vec::new(),
                    },
                    Vec::new(),
                    Vec::new(),
                )
                .expect_err("empty command");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), Vec::new());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(
                        arena.intern_bytes(b"ASSIGN_VAR"),
                        arena.intern_bytes(b"works"),
                    )],
                    argv: vec![arena.intern_bytes(b"echo"), arena.intern_bytes(b"hello")],
                    redirections: Vec::new(),
                };
                let owned_argv: Vec<Vec<u8>> = expanded.argv.iter().map(|s| s.to_vec()).collect();
                let owned_assignments: Vec<(Vec<u8>, Vec<u8>)> = expanded
                    .assignments
                    .iter()
                    .map(|&(n, v)| (n.to_vec(), v.to_vec()))
                    .collect();
                let prepared =
                    build_process_from_expanded(&shell, expanded, owned_argv, owned_assignments)
                        .expect("process");
                assert_eq!(
                    &*prepared.child_env,
                    &[(
                        Box::from(b"ASSIGN_VAR" as &[u8]),
                        Box::from(b"works" as &[u8])
                    )] as &[(Box<[u8]>, Box<[u8]>)]
                );
                assert_eq!(
                    &*prepared.argv,
                    &[Box::from(b"echo" as &[u8]), Box::from(b"hello" as &[u8])] as &[Box<[u8]>]
                );
            },
        );
    }

    #[test]
    fn spawn_prepared_enoexec_falls_back_to_source() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/tmp/script.sh".into()),
                                ArgMatcher::Any,
                                ArgMatcher::Any,
                            ],
                            TraceResult::Fd(20),
                        ),
                        t(
                            "read",
                            vec![ArgMatcher::Fd(20), ArgMatcher::Any],
                            TraceResult::Bytes(b"echo hello\n".to_vec()),
                        ),
                        t("close", vec![ArgMatcher::Fd(20)], TraceResult::Int(0)),
                        t(
                            "execvp",
                            vec![ArgMatcher::Str("/tmp/script.sh".into()), ArgMatcher::Any],
                            TraceResult::Err(sys::ENOEXEC),
                        ),
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/tmp/script.sh".into()),
                                ArgMatcher::Any,
                                ArgMatcher::Any,
                            ],
                            TraceResult::Fd(10),
                        ),
                        t(
                            "read",
                            vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                            TraceResult::Bytes(b"true\n".to_vec()),
                        ),
                        t(
                            "read",
                            vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                    ],
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: b"/tmp/script.sh".to_vec().into(),
                    argv: vec![b"/tmp/script.sh".to_vec().into(), b"arg1".to_vec().into()]
                        .into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: true,
                };
                let child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("enoexec fallback spawn");
                let output = child.wait_with_output().expect("output");
                assert!(output.status.success());
            },
        );
    }

    #[test]
    fn spawn_prepared_errors_for_missing_executable() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/nonexistent/missing".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"missing: not found\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let missing = PreparedProcess {
                    exec_path: b"/nonexistent/missing".to_vec().into(),
                    argv: vec![b"missing".to_vec().into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: false,
                };
                let shell = test_shell();
                assert!(spawn_prepared(&shell, &missing, ProcessGroupPlan::None).is_err());
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_applies_dup_close() {
        run_trace(
            vec![
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(90)],
                    TraceResult::Int(90),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(90), ArgMatcher::Fd(91)],
                    TraceResult::Int(91),
                ),
                t("close", vec![ArgMatcher::Fd(91)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(123_456)], TraceResult::Int(0)),
            ],
            || {
                apply_child_fd_actions(&[
                    ChildFdAction::DupRawFd {
                        fd: 10,
                        target_fd: 90,
                        close_source: true,
                    },
                    ChildFdAction::DupFd {
                        source_fd: 90,
                        target_fd: 91,
                    },
                    ChildFdAction::CloseFd { target_fd: 91 },
                    ChildFdAction::CloseFd { target_fd: 123_456 },
                ])
                .expect("apply child actions");
            },
        );
    }

    #[test]
    fn render_simple_handles_redirection_syntax() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: Some(5),
                        kind: RedirectionKind::ReadWrite,
                        target: Word {
                            raw: b"rw".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(0),
                        kind: RedirectionKind::DupInput,
                        target: Word {
                            raw: b"5".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(1),
                        kind: RedirectionKind::DupOutput,
                        target: Word {
                            raw: b"-".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            assert!(rendered.windows(4).any(|w| w == b"5<>r"));
            assert!(rendered.windows(4).any(|w| w == b"0<&5"));
            assert!(rendered.windows(4).any(|w| w == b"1>&-"));
        });
    }

    #[test]
    fn prepare_redirections_produces_correct_fd_actions() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/rw.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(100),
            )],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::DupOutput,
                            target: b"-",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::ReadWrite,
                            target: b"/tmp/rw.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                    ],
                    false,
                )
                .expect("prepare");
                assert_eq!(prepared.actions.len(), 2);
            },
        );
    }

    #[test]
    fn heredoc_expansion_error_paths() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: missing here-document body\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: redirection target must be a file descriptor or '-'\n"
                                .to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: missing here-document body\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: redirection target must be a file descriptor or '-'\n"
                                .to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let arena = ByteArena::new();
                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"cat".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: None,
                            kind: RedirectionKind::HereDoc,
                            target: Word {
                                raw: b"EOF".to_vec().into(),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
                    &arena,
                )
                .expect_err("expected missing here-document body");
                assert_eq!(error.exit_status(), 2);

                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"echo".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: Some(1),
                            kind: RedirectionKind::DupOutput,
                            target: Word {
                                raw: b"bad".to_vec().into(),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
                    &arena,
                )
                .expect_err("bad dup target");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                let expanded = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello $USER".to_vec().into(),
                            expand: true,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("expand heredoc redirection");
                assert_eq!(expanded[0].target, b"EOF");
                assert_eq!(expanded[0].here_doc_body, Some(b"hello " as &[u8]));

                let mut shell = test_shell();
                let literal = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello $USER".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("literal heredoc redirection");
                assert_eq!(literal[0].here_doc_body, Some(b"hello $USER" as &[u8]));

                let mut shell = test_shell();
                let error = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    }],
                    &arena,
                )
                .expect_err("missing expanded heredoc body");
                assert_eq!(error.exit_status(), 2);

                let mut shell = test_shell();
                let stripped = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello\nworld\n".to_vec().into(),
                            expand: false,
                            strip_tabs: true,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("strip tabs expand");
                assert_eq!(stripped[0].here_doc_body, Some(b"hello\nworld\n" as &[u8]));

                let mut shell = test_shell();
                let dup_err = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: Some(2),
                        kind: RedirectionKind::DupOutput,
                        target: Word {
                            raw: b"abc".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    }],
                    &arena,
                )
                .expect_err("bad dup target in standalone");
                assert_eq!(dup_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn prepare_redirections_creates_heredoc_pipe() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            ],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF",
                        here_doc_body: Some(b"body\n"),
                        line: 0,
                    }],
                    false,
                )
                .expect("prepare heredoc");
                assert_eq!(prepared.actions.len(), 1);
            },
        );
    }

    #[test]
    fn prepare_redirections_heredoc_write_error() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Err(sys::EIO),
                ),
            ],
            || {
                let err = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF",
                        here_doc_body: Some(b"body\n"),
                        line: 0,
                    }],
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.strerror().is_empty());
            },
        );
    }

    #[test]
    fn render_helpers_cover_program_function_if_loop_simple_pipeline() {
        assert_no_syscalls(|| {
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

            let function = FunctionDef {
                name: b"greet".to_vec().into(),
                body: Box::new(Command::Group(program.clone())),
            };
            let if_command = IfCommand {
                condition: program.clone(),
                then_branch: program.clone(),
                elif_branches: Vec::new().into_boxed_slice(),

                else_branch: None,
            };
            let loop_command = LoopCommand {
                kind: LoopKind::While,
                condition: program.clone(),
                body: program.clone(),
            };
            assert!(render_program(&program).windows(4).any(|w| w == b"true"));
            assert!(
                render_function(&function)
                    .windows(7)
                    .any(|w| w == b"greet()")
            );
            assert!(render_if(&if_command).starts_with(b"if "));
            assert!(render_loop(&loop_command).starts_with(b"while "));

            let simple = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"1".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word {
                        raw: b"out".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            };
            assert_eq!(render_simple(&simple), b"X=1 echo >out");

            let multi_assign = SimpleCommand {
                assignments: vec![
                    Assignment {
                        name: b"A".to_vec().into(),
                        value: Word {
                            raw: b"1".to_vec().into(),
                            line: 0,
                        },
                    },
                    Assignment {
                        name: b"B".to_vec().into(),
                        value: Word {
                            raw: b"2".to_vec().into(),
                            line: 0,
                        },
                    },
                ]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),

                redirections: vec![].into_boxed_slice(),
            };
            assert_eq!(render_simple(&multi_assign), b"A=1 B=2");

            let pipeline = Pipeline {
                negated: true,
                timed: TimedMode::Off,
                commands: vec![
                    Command::Subshell(program.clone()),
                    Command::Group(program.clone()),
                    Command::FunctionDef(function),
                    Command::If(if_command),
                    Command::Loop(loop_command),
                ]
                .into_boxed_slice(),
            };
            assert!(render_pipeline(&pipeline).starts_with(b"! "));
        });
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
            vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t_fork(TraceResult::Pid(2000), vec![]),
                        t(
                            "waitpid",
                            vec![ArgMatcher::Int(2000), ArgMatcher::Any, ArgMatcher::Int(0)],
                            TraceResult::Status(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1001),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(200), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
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
    fn render_program_handles_async_and_heredoc_items() {
        assert_no_syscalls(|| {
            let async_program = Program {
                items: vec![
                    ListItem {
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
                        asynchronous: true,
                        line: 0,
                    },
                    ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: b"false".to_vec().into(),
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
                    },
                ]
                .into_boxed_slice(),
            };
            assert_eq!(render_list_item(&async_program.items[0]), b"true &");
            assert_eq!(render_program(&async_program), b"true &\nfalse");

            let heredoc_program = parse_test(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
            assert_eq!(render_program(&heredoc_program), b": <<EOF\nhello\nEOF");
        });
    }

    #[test]
    fn execute_nested_program_sets_up_heredoc_fd() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(0),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Err(sys::EBADF),
                ),
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"hello\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            ],
            || {
                let heredoc_program = parse_test(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
                let mut shell = test_shell();
                let status = execute_nested_program(&mut shell, &heredoc_program)
                    .expect("execute heredoc nested");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn execute_if_and_loop_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = parse_test(
                "if false; then VALUE=no; elif true; then VALUE=yes; else VALUE=bad; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let while_program = parse_test(
                "COUNTER=1; while case $COUNTER in 0) false ;; *) true ;; esac; do COUNTER=0; FLAG=done; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &while_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FLAG"), Some(b"done" as &[u8]));

            let mut shell = test_shell();
            let until_program = parse_test(
                "READY=; until case $READY in yes) true ;; *) false ;; esac; do READY=yes; VALUE=ready; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &until_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"ready" as &[u8]));
        });
    }

    #[test]
    fn execute_for_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("for item in a b c; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"LAST"), Some(b"c" as &[u8]));

            let mut shell = test_shell();
            shell.positional = vec![b"alpha".to_vec(), b"beta".to_vec()];
            let program = parse_test("for item; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"LAST"), Some(b"beta" as &[u8]));
        });
    }

    #[test]
    fn execute_case_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program =
                parse_test("name=beta; case $name in alpha) VALUE=no ;; b*) VALUE=yes ;; esac")
                    .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let program = parse_test("name=zeta; case $name in alpha|beta) VALUE=hit ;; esac")
                .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case a in a) A=1 ;& b) B=2 ;; c) C=3 ;; esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"A"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"B"), Some(b"2" as &[u8]));
            assert_eq!(shell.get_var(b"C"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case x in x) V=one ;& y) V=two ;& z) V=three ;& esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), Some(b"three" as &[u8]));
        });
    }

    #[test]
    fn execute_if_covers_then_and_else_branches() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .env
                .insert(b"PATH".to_vec(), b"/usr/bin:/bin".to_vec());
            shell.exported.insert(b"PATH".to_vec());

            let if_program =
                parse_test("if true; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let if_program =
                parse_test("if false; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"no" as &[u8]));

            let mut shell = test_shell();
            let if_program = parse_test(
                "if false; then VALUE=yes; elif false; then VALUE=maybe; else VALUE=no; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"no" as &[u8]));
        });
    }

    #[test]
    fn render_and_or_produces_correct_output() {
        assert_no_syscalls(|| {
            let render = render_and_or(&AndOr {
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
                rest: vec![(
                    LogicalOp::And,
                    Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: b"false".to_vec().into(),
                                line: 0,
                            }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                )]
                .into_boxed_slice(),
            });
            assert!(render.windows(2).any(|w| w == b"&&"));
        });
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
            vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t("pipe", vec![], TraceResult::Fds(202, 203)),
                t_fork(
                    TraceResult::Pid(1001),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(200), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t("close", vec![ArgMatcher::Fd(202)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(203), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(203)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(203)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t("pipe", vec![], TraceResult::Fds(204, 205)),
                t_fork(
                    TraceResult::Pid(1002),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(202), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(202)], TraceResult::Int(0)),
                        t("close", vec![ArgMatcher::Fd(204)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(205), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(205)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(202)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(205)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1002), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t("pipe", vec![], TraceResult::Fds(206, 207)),
                t_fork(
                    TraceResult::Pid(1003),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(204), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(204)], TraceResult::Int(0)),
                        t("close", vec![ArgMatcher::Fd(206)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(207), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(207)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(204)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(207)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1003), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1004),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(206), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(206)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(206)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1004), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1002), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1003), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1004), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
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

    // The remaining tests are structurally identical to the originals but with
    // byte-typed literals.  Since the file is very large we include them by
    // delegating to `parse_test` (which wraps `syntax::parse` on `&[u8]`) and
    // the `test_shell()` helper which already returns byte-typed fields.

    #[test]
    fn loop_and_function_exit_behavior() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = parse_test("if false; then VALUE=yes; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), None);

            let mut shell = test_shell();
            let for_program = parse_test("for item in a b; do exit 9; done").expect("parse");
            let status = execute_program(&mut shell, &for_program).expect("exec");
            assert_eq!(status, 9);
            assert!(!shell.running);
            assert_eq!(shell.get_var(b"item"), Some(b"a" as &[u8]));

            let mut shell = test_shell();
            let loop_program = parse_test("while true; do exit 7; done").expect("parse");
            let status = execute_program(&mut shell, &loop_program).expect("exec");
            assert_eq!(status, 7);
            assert!(!shell.running);

            let mut shell = test_shell();
            let program = parse_test("greet() { RESULT=$X; }; X=ok greet").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"RESULT"), Some(b"ok" as &[u8]));
        });
    }

    #[test]
    fn control_flow_propagates_across_functions_and_loops() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(
                        b"meiksh: line 1: break: only meaningful in a loop\n".to_vec(),
                    ),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let program = parse_test("f() { return 6; VALUE=bad; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 6);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.pending_control, None);

                let mut shell = test_shell();
                let program = parse_test(
                "for outer in x y; do for inner in a b; do continue 2; VALUE=bad; done; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.get_var(b"outer"), Some(b"y" as &[u8]));

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x y; do for inner in a b; do break 2; done; VALUE=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.get_var(b"outer"), Some(b"x" as &[u8]));

                let mut shell = test_shell();
                let program =
                    parse_test("f() { while true; do return 4; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 4);
                assert_eq!(shell.pending_control, None);

                let mut shell = test_shell();
                let program = parse_test("g() { break; }; g").expect("parse");
                let error = execute_program(&mut shell, &program).expect_err("function error");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do while break 2; do printf no; done; AFTER=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"AFTER"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do while continue 2; do printf no; done; AFTER=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"AFTER"), None);

                let mut shell = test_shell();
                let program =
                    parse_test("f() { while return 3; do printf no; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 3);

                let mut shell = test_shell();
                let program = parse_test(
                "COUNT=1; while case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; do printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"COUNT"), Some(b"0" as &[u8]));

                let mut shell = test_shell();
                let program = parse_test(
                "COUNT=1; while true; do case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"COUNT"), Some(b"0" as &[u8]));

                let mut shell = test_shell();
                let program =
                    parse_test("f() { for item in a; do return 5; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 5);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do for inner in y; do break 2; done; DONE=no; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"DONE"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do for inner in y; do continue 2; done; DONE=no; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"DONE"), None);

                let mut shell = test_shell();
                shell.loop_depth = 1;
                let loop_command = LoopCommand {
                    kind: LoopKind::While,
                    condition: parse_test("true").expect("parse"),
                    body: parse_test("break 2").expect("parse"),
                };
                let status = execute_loop(&mut shell, &loop_command).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.pending_control, Some(PendingControl::Break(1)));

                let mut shell = test_shell();
                shell.loop_depth = 1;
                let loop_command = LoopCommand {
                    kind: LoopKind::While,
                    condition: parse_test("true").expect("parse"),
                    body: parse_test("continue 2").expect("parse"),
                };
                let status = execute_loop(&mut shell, &loop_command).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.pending_control, Some(PendingControl::Continue(1)));
            },
        );
    }

    #[test]
    fn save_restore_vars_restores_previous_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            shell.exported.insert(b"FOO".to_vec());

            let assignments = vec![
                (b"FOO".to_vec(), b"temp".to_vec()),
                (b"BAR".to_vec(), b"new".to_vec()),
            ];
            let saved = save_vars(&shell, &assignments);

            shell.set_var(b"FOO", b"temp".to_vec()).unwrap();
            shell.set_var(b"BAR", b"new".to_vec()).unwrap();
            assert_eq!(shell.get_var(b"FOO"), Some(b"temp" as &[u8]));
            assert_eq!(shell.get_var(b"BAR"), Some(b"new" as &[u8]));

            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
            assert!(shell.exported.contains(&b"FOO".to_vec()));
            assert_eq!(shell.get_var(b"BAR"), None);
            assert!(!shell.exported.contains(&b"BAR".to_vec()));
        });
    }

    #[test]
    fn non_special_builtin_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("FOO=temp true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn special_builtin_prefix_assignments_are_permanent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=permanent :").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"permanent" as &[u8]));
        });
    }

    #[test]
    fn function_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("myfn() { :; }; FOO=temp myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn non_special_builtin_exit_with_temp_assignments() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=bar exit 0").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(!shell.running);
        });
    }

    #[test]
    fn assignment_expansion_does_not_field_split() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"IFS".to_vec(), b" ".to_vec());
            shell.env.insert(b"X".to_vec(), b"a b c".to_vec());
            let program = parse_test("Y=$X").expect("parse");
            let _status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(shell.get_var(b"Y"), Some(b"a b c" as &[u8]));
        });
    }

    #[test]
    fn xtrace_writes_trace_to_stderr() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"+ echo hello\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],

                    argv: vec![b"echo", b"hello"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn xtrace_skipped_when_disabled() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.xtrace = false;
            let expanded = ExpandedSimpleCommand {
                assignments: vec![],

                argv: vec![b"echo"],
                redirections: vec![],
            };
            write_xtrace(&mut shell, &expanded);
        });
    }

    #[test]
    fn xtrace_includes_assignments() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"+ FOO=bar cmd\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(b"FOO", b"bar")],
                    argv: vec![b"cmd"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn has_command_substitution_detects_backtick_in_words() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                words: vec![Word {
                    raw: b"echo `date`".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd));

            let cmd_no_sub = SimpleCommand {
                words: vec![Word {
                    raw: b"plain".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_no_sub));
        });
    }

    #[test]
    fn case_pattern_matching_covers_wildcards_and_classes() {
        assert_no_syscalls(|| {
            assert!(case_pattern_matches(b"beta", b"b*"));
            assert!(case_pattern_matches(b"beta", b"b?t[ab]"));
            assert!(case_pattern_matches(b"x", b"[!ab]"));
            assert!(case_pattern_matches(b"*", b"\\*"));
            assert!(case_pattern_matches(b"-", b"[\\-]"));
            assert!(case_pattern_matches(b"b", b"[a-c]"));
            assert!(!case_pattern_matches(b"[", b"[a"));
            assert!(!case_pattern_matches(b"x", b"["));
            assert!(!case_pattern_matches(b"beta", b"a*"));
            assert!(!case_pattern_matches(b"a", b"[!ab]"));

            assert!(case_pattern_matches(b"a", b"[[:alpha:]]"));
            assert!(case_pattern_matches(b"Z", b"[[:alpha:]]"));
            assert!(!case_pattern_matches(b"5", b"[[:alpha:]]"));
            assert!(case_pattern_matches(b"3", b"[[:alnum:]]"));
            assert!(!case_pattern_matches(b"!", b"[[:alnum:]]"));
            assert!(case_pattern_matches(b" ", b"[[:blank:]]"));
            assert!(case_pattern_matches(b"\t", b"[[:blank:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:blank:]]"));
            assert!(case_pattern_matches(b"\x01", b"[[:cntrl:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:cntrl:]]"));
            assert!(case_pattern_matches(b"9", b"[[:digit:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:digit:]]"));
            assert!(case_pattern_matches(b"!", b"[[:graph:]]"));
            assert!(!case_pattern_matches(b" ", b"[[:graph:]]"));
            assert!(case_pattern_matches(b"a", b"[[:lower:]]"));
            assert!(!case_pattern_matches(b"A", b"[[:lower:]]"));
            assert!(case_pattern_matches(b" ", b"[[:print:]]"));
            assert!(case_pattern_matches(b"a", b"[[:print:]]"));
            assert!(!case_pattern_matches(b"\x01", b"[[:print:]]"));
            assert!(case_pattern_matches(b".", b"[[:punct:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:punct:]]"));
            assert!(case_pattern_matches(b"\n", b"[[:space:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:space:]]"));
            assert!(case_pattern_matches(b"A", b"[[:upper:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:upper:]]"));
            assert!(case_pattern_matches(b"f", b"[[:xdigit:]]"));
            assert!(!case_pattern_matches(b"g", b"[[:xdigit:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:bogus:]]"));
            assert!(case_pattern_matches(b"x", b"[[:x]"));
        });
    }

    #[test]
    fn errexit_exits_on_nonzero_simple_command() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
            assert!(!shell.running);
        });
    }

    #[test]
    fn errexit_does_not_exit_on_zero_status() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_suppressed_in_if_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("if false; then :; fi; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
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
    fn check_errexit_does_nothing_when_disabled() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = false;
            check_errexit(&mut shell, 1);
            assert!(shell.running);
        });
    }

    #[test]
    fn check_errexit_does_nothing_when_suppressed() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            shell.errexit_suppressed = true;
            check_errexit(&mut shell, 1);
            assert!(shell.running);
        });
    }

    #[test]
    fn check_errexit_stops_shell_on_failure() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            check_errexit(&mut shell, 1);
            assert!(!shell.running);
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn replace_shell_fd_same_fd() {
        assert_no_syscalls(|| {
            replace_shell_fd(42, 42).expect("same-fd replacement");
        });
    }

    fn t_stderr(msg: &str) -> TraceEntry {
        t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(format!("{msg}\n").into_bytes()),
            ],
            TraceResult::Auto,
        )
    }

    #[test]
    fn lineno_parse_error_unterminated_single_quote() {
        run_trace(
            vec![t_stderr("meiksh: line 3: unterminated single quote")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\ntrue\necho '");
            },
        );
    }

    #[test]
    fn lineno_parse_error_unterminated_double_quote() {
        run_trace(
            vec![t_stderr("meiksh: line 2: unterminated double quote")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\necho \"hello");
            },
        );
    }

    #[test]
    fn lineno_parse_error_empty_if_condition() {
        run_trace(
            vec![t_stderr("meiksh: line 3: expected command list after 'if'")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\nif\nthen true; fi");
            },
        );
    }

    #[test]
    fn lineno_expand_nounset_on_line_2() {
        run_trace(
            vec![t_stderr("meiksh: line 2: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string(b"true\necho $MISSING");
            },
        );
    }

    #[test]
    fn lineno_expand_error_on_line_3() {
        run_trace(vec![t_stderr("meiksh: line 3: must be set")], || {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"true\ntrue\n: ${NOVAR?must be set}");
        });
    }

    #[test]
    fn lineno_runtime_break_outside_loop() {
        run_trace(
            vec![t_stderr("meiksh: line 2: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\nbreak");
            },
        );
    }

    #[test]
    fn lineno_runtime_readonly_assignment() {
        run_trace(
            vec![t_stderr("meiksh: line 2: X: readonly variable")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"readonly X=1\nX=2");
            },
        );
    }

    #[test]
    fn lineno_env_var_matches_shell_lineno() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"true\ntrue\ntrue");
            assert_eq!(shell.get_var(b"LINENO"), Some(b"3" as &[u8]));
        });
    }

    #[test]
    fn lineno_env_var_updates_per_list_item() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"A=$LINENO\ntrue\nB=$LINENO");
            assert_eq!(shell.get_var(b"A"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"B"), Some(b"3" as &[u8]));
        });
    }

    #[test]
    fn special_builtin_utility_error_exits_noninteractive() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: set: invalid option: Z\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let err = shell.execute_string(b"set -Z").expect_err("sbi error");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn special_builtin_utility_error_continues_interactive() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: set: invalid option: Z\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let status = shell.execute_string(b"set -Z").expect("sbi interactive");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn declaration_builtin_expands_assignments_and_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let status = shell
                .execute_string(b"command export FOO=bar BAZ")
                .expect("export");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"bar" as &[u8]));
        });
    }

    #[test]
    fn file_needs_binary_rejection_handles_errors_and_empty() {
        run_trace(
            vec![t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(sys::EACCES),
            )],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Err(libc::EIO),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_new_file_succeeds() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/new/file.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Int(10),
            )],
            || {
                let fd = open_for_write_noclobber(b"/new/file.txt").expect("new file");
                assert_eq!(fd, 10);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_non_regular_reopens() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EEXIST),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/dev/null".into()), ArgMatcher::Any],
                    TraceResult::StatFifo,
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(11),
                ),
            ],
            || {
                let fd = open_for_write_noclobber(b"/dev/null").expect("fifo reopen");
                assert_eq!(fd, 11);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_other_error() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/noperm".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Err(libc::EACCES),
            )],
            || {
                let err = open_for_write_noclobber(b"/noperm").expect_err("eacces");
                assert_eq!(err.errno(), Some(libc::EACCES));
            },
        );
    }

    #[test]
    fn render_and_or_with_logical_or() {
        assert_no_syscalls(|| {
            let rendered = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"false".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    })]
                    .into_boxed_slice(),
                },
                rest: vec![(
                    LogicalOp::Or,
                    Pipeline {
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
                )]
                .into_boxed_slice(),
            });
            assert_eq!(rendered, b"false || true");
        });
    }

    #[test]
    fn render_command_for_and_case() {
        assert_no_syscalls(|| {
            let for_cmd = Command::For(ForCommand {
                name: b"x".to_vec().into(),
                items: Some(
                    vec![Word {
                        raw: b"a".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                ),
                body: Program {
                    items: vec![ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: b"echo".to_vec().into(),
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
                },
            });
            let rendered = render_command(&for_cmd);
            assert_eq!(rendered, b"for x in a\ndo\necho\ndone");

            let case_cmd = Command::Case(CaseCommand {
                word: Word {
                    raw: b"val".to_vec().into(),
                    line: 0,
                },
                arms: vec![crate::syntax::CaseArm {
                    patterns: vec![Word {
                        raw: b"a".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                    body: Program {
                        items: vec![ListItem {
                            and_or: AndOr {
                                first: Pipeline {
                                    negated: false,
                                    timed: TimedMode::Off,
                                    commands: vec![Command::Simple(SimpleCommand {
                                        words: vec![Word {
                                            raw: b"echo".to_vec().into(),
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
                    },
                    fallthrough: false,
                }]
                .into_boxed_slice(),
            });
            let rendered = render_command(&case_cmd);
            assert!(rendered.starts_with(b"case val in"));
        });
    }

    #[test]
    fn render_if_with_elif_and_else() {
        assert_no_syscalls(|| {
            let true_program = Program {
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
            let if_cmd = IfCommand {
                condition: true_program.clone(),
                then_branch: true_program.clone(),
                elif_branches: vec![crate::syntax::ElifBranch {
                    condition: true_program.clone(),
                    body: true_program.clone(),
                }]
                .into_boxed_slice(),
                else_branch: Some(true_program),
            };
            let rendered = render_if(&if_cmd);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.contains("elif "));
            assert!(text.contains("\nthen\n"));
            assert!(text.contains("\nelse\n"));
            assert!(text.ends_with("\nfi"));
        });
    }

    #[test]
    fn render_loop_until() {
        assert_no_syscalls(|| {
            let prog = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"false".to_vec().into(),
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
            let loop_cmd = LoopCommand {
                kind: LoopKind::Until,
                condition: prog.clone(),
                body: prog,
            };
            let rendered = render_loop(&loop_cmd);
            assert!(rendered.starts_with(b"until "));
            assert!(rendered.ends_with(b"\ndone"));
        });
    }

    #[test]
    fn render_for_with_items_and_without() {
        assert_no_syscalls(|| {
            let body = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"echo".to_vec().into(),
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

            let with_items = ForCommand {
                name: b"item".to_vec().into(),
                items: Some(
                    vec![
                        Word {
                            raw: b"a".to_vec().into(),
                            line: 0,
                        },
                        Word {
                            raw: b"b".to_vec().into(),
                            line: 0,
                        },
                        Word {
                            raw: b"c".to_vec().into(),
                            line: 0,
                        },
                    ]
                    .into_boxed_slice(),
                ),
                body: body.clone(),
            };
            let rendered = render_for(&with_items);
            assert_eq!(rendered, b"for item in a b c\ndo\necho\ndone");

            let without_items = ForCommand {
                name: b"arg".to_vec().into(),
                items: None,
                body,
            };
            let rendered = render_for(&without_items);
            assert_eq!(rendered, b"for arg\ndo\necho\ndone");
        });
    }

    #[test]
    fn render_case_with_fallthrough_and_multi_pattern() {
        assert_no_syscalls(|| {
            let body = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"echo".to_vec().into(),
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
            let case_cmd = CaseCommand {
                word: Word {
                    raw: b"val".to_vec().into(),
                    line: 0,
                },
                arms: vec![
                    crate::syntax::CaseArm {
                        patterns: vec![
                            Word {
                                raw: b"a".to_vec().into(),
                                line: 0,
                            },
                            Word {
                                raw: b"b".to_vec().into(),
                                line: 0,
                            },
                        ]
                        .into_boxed_slice(),
                        body: body.clone(),
                        fallthrough: true,
                    },
                    crate::syntax::CaseArm {
                        patterns: vec![Word {
                            raw: b"c".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        body,
                        fallthrough: false,
                    },
                ]
                .into_boxed_slice(),
            };
            let rendered = render_case(&case_cmd);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.starts_with("case val in"));
            assert!(text.contains("a | b)"));
            assert!(text.contains("\n;&"));
            assert!(text.contains("c)"));
            assert!(text.contains("\n;;"));
            assert!(text.ends_with("\nesac"));
        });
    }

    #[test]
    fn render_redirection_operators_read_clobber_append_heredoc_strip() {
        assert_no_syscalls(|| {
            let read_redir = Redirection {
                fd: None,
                kind: RedirectionKind::Read,
                target: Word {
                    raw: b"input.txt".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            let mut buf = Vec::new();
            render_redirection_operator_into(&read_redir, &mut buf);
            assert_eq!(buf, b"<input.txt");

            let clobber_redir = Redirection {
                fd: Some(1),
                kind: RedirectionKind::ClobberWrite,
                target: Word {
                    raw: b"out.txt".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            buf.clear();
            render_redirection_operator_into(&clobber_redir, &mut buf);
            assert_eq!(buf, b"1>|out.txt");

            let append_redir = Redirection {
                fd: Some(2),
                kind: RedirectionKind::Append,
                target: Word {
                    raw: b"log".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            buf.clear();
            render_redirection_operator_into(&append_redir, &mut buf);
            assert_eq!(buf, b"2>>log");

            let heredoc_strip = Redirection {
                fd: None,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: b"EOF".to_vec().into(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: b"EOF".to_vec().into(),
                    body: b"content\n".to_vec().into(),
                    expand: false,
                    strip_tabs: true,
                    body_line: 0,
                }),
            };
            buf.clear();
            render_redirection_operator_into(&heredoc_strip, &mut buf);
            assert_eq!(buf, b"<<-EOF");

            let heredoc_no_strip = Redirection {
                fd: None,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: b"END".to_vec().into(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: b"END".to_vec().into(),
                    body: b"stuff\n".to_vec().into(),
                    expand: false,
                    strip_tabs: false,
                    body_line: 0,
                }),
            };
            buf.clear();
            render_redirection_operator_into(&heredoc_no_strip, &mut buf);
            assert_eq!(buf, b"<<END");
        });
    }

    #[test]
    fn render_here_doc_body_appends_newline_when_missing() {
        assert_no_syscalls(|| {
            let with_newline = HereDoc {
                delimiter: b"EOF".to_vec().into(),
                body: b"hello\n".to_vec().into(),
                expand: false,
                strip_tabs: false,
                body_line: 0,
            };
            assert_eq!(render_here_doc_body(&with_newline), b"hello\nEOF");

            let without_newline = HereDoc {
                delimiter: b"EOF".to_vec().into(),
                body: b"hello".to_vec().into(),
                expand: false,
                strip_tabs: false,
                body_line: 0,
            };
            assert_eq!(render_here_doc_body(&without_newline), b"hello\nEOF");
        });
    }

    #[test]
    fn render_simple_with_multiple_heredocs() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word {
                    raw: b"cat".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF1".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF1".to_vec().into(),
                            body: b"first\n".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    },
                    Redirection {
                        fd: Some(3),
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF2".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF2".to_vec().into(),
                            body: b"second".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.starts_with("cat <<EOF1 3<<EOF2\n"));
            assert!(text.contains("first\nEOF1\n"));
            assert!(text.contains("second\nEOF2"));
        });
    }

    #[test]
    fn var_error_bytes_formats_readonly() {
        assert_no_syscalls(|| {
            let err = VarError::Readonly(b"HOME".to_vec().into());
            assert_eq!(var_error_bytes(&err), b"HOME: readonly variable");
        });
    }

    #[test]
    fn join_boxed_bytes_various_inputs() {
        assert_no_syscalls(|| {
            let empty: Vec<Box<[u8]>> = vec![];
            assert_eq!(join_boxed_bytes(&empty, b' '), b"");

            let single: Vec<Box<[u8]>> = vec![b"hello".to_vec().into()];
            assert_eq!(join_boxed_bytes(&single, b' '), b"hello");

            let multi: Vec<Box<[u8]>> = vec![
                b"a".to_vec().into(),
                b"bb".to_vec().into(),
                b"ccc".to_vec().into(),
            ];
            assert_eq!(join_boxed_bytes(&multi, b' '), b"a bb ccc");
            assert_eq!(join_boxed_bytes(&multi, b'/'), b"a/bb/ccc");
        });
    }

    #[test]
    fn parse_i32_bytes_various_inputs() {
        assert_no_syscalls(|| {
            assert_eq!(parse_i32_bytes(b"0"), Some(0));
            assert_eq!(parse_i32_bytes(b"42"), Some(42));
            assert_eq!(parse_i32_bytes(b"-1"), Some(-1));
            assert_eq!(parse_i32_bytes(b"2147483647"), Some(i32::MAX));
            assert_eq!(parse_i32_bytes(b"-2147483648"), Some(i32::MIN));
            assert_eq!(parse_i32_bytes(b"2147483648"), None);
            assert_eq!(parse_i32_bytes(b""), None);
            assert_eq!(parse_i32_bytes(b"abc"), None);
            assert_eq!(parse_i32_bytes(b"-"), None);
        });
    }

    #[test]
    fn default_fd_for_redirection_covers_all_kinds() {
        assert_no_syscalls(|| {
            assert_eq!(default_fd_for_redirection(RedirectionKind::Read), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::HereDoc), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::ReadWrite), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::DupInput), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::Write), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::ClobberWrite), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::Append), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::DupOutput), 1);
        });
    }

    #[test]
    fn is_declaration_utility_matches_export_and_readonly() {
        assert_no_syscalls(|| {
            assert!(is_declaration_utility(b"export"));
            assert!(is_declaration_utility(b"readonly"));
            assert!(!is_declaration_utility(b"echo"));
            assert!(!is_declaration_utility(b"command"));
            assert!(!is_declaration_utility(b""));
        });
    }

    #[test]
    fn find_declaration_context_with_command_prefix() {
        assert_no_syscalls(|| {
            let words = |strs: &[&[u8]]| -> Vec<Word> {
                strs.iter()
                    .map(|s| Word {
                        raw: s.to_vec().into(),
                        line: 0,
                    })
                    .collect()
            };

            assert!(find_declaration_context(&words(&[b"export"])));
            assert!(find_declaration_context(&words(&[b"readonly"])));
            assert!(find_declaration_context(&words(&[b"command", b"export"])));
            assert!(find_declaration_context(&words(&[
                b"command", b"-v", b"export"
            ])));
            assert!(!find_declaration_context(&words(&[b"echo"])));
            assert!(!find_declaration_context(&words(&[b"command"])));
            assert!(!find_declaration_context(&words(&[])));
        });
    }

    #[test]
    fn has_command_substitution_dollar_paren_in_assignments() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"$(date)".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(has_command_substitution(&cmd));

            let cmd_backtick_assign = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"`date`".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(has_command_substitution(&cmd_backtick_assign));

            let cmd_dollar_paren_word = SimpleCommand {
                words: vec![Word {
                    raw: b"echo $(date)".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd_dollar_paren_word));

            let cmd_none = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"plain".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(!has_command_substitution(&cmd_none));
        });
    }

    #[test]
    fn close_parent_redirection_fds_only_closes_marked() {
        run_trace(
            vec![t("close", vec![ArgMatcher::Fd(77)], TraceResult::Int(0))],
            || {
                let prepared = PreparedRedirections {
                    actions: vec![
                        ChildFdAction::DupRawFd {
                            fd: 77,
                            target_fd: 1,
                            close_source: true,
                        },
                        ChildFdAction::DupRawFd {
                            fd: 88,
                            target_fd: 2,
                            close_source: false,
                        },
                        ChildFdAction::DupFd {
                            source_fd: 3,
                            target_fd: 4,
                        },
                        ChildFdAction::CloseFd { target_fd: 5 },
                    ],
                };
                close_parent_redirection_fds(&prepared);
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_ebadf_ignored() {
        run_trace(
            vec![t(
                "close",
                vec![ArgMatcher::Fd(999)],
                TraceResult::Err(sys::EBADF),
            )],
            || {
                apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect("ebadf on close should be ignored");
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_non_ebadf_propagates() {
        run_trace(
            vec![t(
                "close",
                vec![ArgMatcher::Fd(999)],
                TraceResult::Err(sys::EIO),
            )],
            || {
                let err = apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect_err("non-ebadf close should fail");
                assert_eq!(err.errno(), Some(sys::EIO));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_elf_prefix_allowed() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Bytes(b"\x7fELF\x02\x01\x01\x00".to_vec()),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/elf"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_shebang_allowed() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Bytes(b"#!/bin/sh\necho hi\n".to_vec()),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/script"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_null_byte_triggers() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Bytes(b"binary\x00data\n".to_vec()),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(file_needs_binary_rejection(b"/some/binary"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_text_without_null_ok() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Fd(50),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Any],
                    TraceResult::Bytes(b"just plain text\n".to_vec()),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/text"));
            },
        );
    }

    #[test]
    fn readonly_var_blocks_external_cmd_prefix_assignment() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: X: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                shell.readonly.insert(b"X".to_vec());
                let err = shell
                    .execute_string(b"X=val /nonexistent/cmd")
                    .expect_err("readonly prefix");
                assert_ne!(err.exit_status(), 0);
            },
        );
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
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(100)),
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(2_000_000_000),
                ),
                t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(100)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\nreal\t0m1.000s\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"user\t0m0.000s\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"sys\t0m0.000s\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(100)),
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(2_500_000_000),
                ),
                t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(100)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"real 1.50\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"user 0.00\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"sys 0.00\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let program = parse_test("time -p true").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn render_pipeline_multiple_commands() {
        assert_no_syscalls(|| {
            let pipeline = Pipeline {
                negated: false,
                timed: TimedMode::Off,
                commands: vec![
                    Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"cat".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    }),
                    Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"grep".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    }),
                ]
                .into_boxed_slice(),
            };
            let rendered = render_pipeline(&pipeline);
            assert_eq!(rendered, b"cat | grep");
        });
    }

    #[test]
    fn write_xtrace_with_custom_ps4() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b">> echo hi\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                shell.env.insert(b"PS4".to_vec(), b">> ".to_vec());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],
                    argv: vec![b"echo", b"hi"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn write_xtrace_empty_argv_with_assignments_only() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"+ A=1 B=2 \n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(b"A" as &[u8], b"1" as &[u8]), (b"B", b"2")],
                    argv: vec![],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn render_redirected_command() {
        assert_no_syscalls(|| {
            let cmd = Command::Redirected(
                Box::new(Command::Simple(SimpleCommand {
                    words: vec![Word {
                        raw: b"echo".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                    ..SimpleCommand::default()
                })),
                vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word {
                        raw: b"out.txt".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            );
            let rendered = render_command(&cmd);
            assert_eq!(rendered, b"echo >out.txt");
        });
    }

    #[test]
    fn render_command_line_redirections_only() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![].into_boxed_slice(),
                assignments: vec![].into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: Some(2),
                    kind: RedirectionKind::Append,
                    target: Word {
                        raw: b"err.log".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            };
            let rendered = render_simple(&simple);
            assert_eq!(rendered, b"2>>err.log");
        });
    }

    #[test]
    fn check_errexit_zero_status_does_nothing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            check_errexit(&mut shell, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn prepare_redirections_read_write_append_noclobber() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/r.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/w.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(11),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/a.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(12),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/cw.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(13),
                ),
            ],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::Read,
                            target: b"/tmp/r.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::Write,
                            target: b"/tmp/w.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 2,
                            kind: RedirectionKind::Append,
                            target: b"/tmp/a.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::ClobberWrite,
                            target: b"/tmp/cw.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                    ],
                    false,
                )
                .expect("prepare");
                assert_eq!(prepared.actions.len(), 4);
            },
        );
    }

    #[test]
    fn prepare_redirections_dup_input_and_close() {
        assert_no_syscalls(|| {
            let prepared = prepare_redirections(
                &[
                    ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::DupInput,
                        target: b"3",
                        here_doc_body: None,
                        line: 0,
                    },
                    ExpandedRedirection {
                        fd: 5,
                        kind: RedirectionKind::DupOutput,
                        target: b"-",
                        here_doc_body: None,
                        line: 0,
                    },
                ],
                false,
            )
            .expect("prepare dup");
            assert_eq!(prepared.actions.len(), 2);
            match &prepared.actions[0] {
                ChildFdAction::DupFd {
                    source_fd,
                    target_fd,
                } => {
                    assert_eq!(*source_fd, 3);
                    assert_eq!(*target_fd, 0);
                }
                other => panic!("expected DupFd, got {other:?}"),
            }
            match &prepared.actions[1] {
                ChildFdAction::CloseFd { target_fd } => {
                    assert_eq!(*target_fd, 5);
                }
                other => panic!("expected CloseFd, got {other:?}"),
            }
        });
    }

    #[test]
    fn prepare_redirections_noclobber_uses_excl() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/nc.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(20),
            )],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 1,
                        kind: RedirectionKind::Write,
                        target: b"/tmp/nc.txt",
                        here_doc_body: None,
                        line: 0,
                    }],
                    true,
                )
                .expect("prepare noclobber");
                assert_eq!(prepared.actions.len(), 1);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_regular_file_fails() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/exists.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EEXIST),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/exists.txt".into()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let err = open_for_write_noclobber(b"/exists.txt").expect_err("should fail");
                assert_eq!(err.errno(), Some(sys::EEXIST));
            },
        );
    }

    #[test]
    fn case_fallthrough_stops_at_end() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("case a in a) V=one ;& esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), Some(b"one" as &[u8]));
        });
    }

    #[test]
    fn case_no_match_returns_zero() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("case z in a) V=bad ;; b) V=bad ;; esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), None);
        });
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn for_readonly_variable_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: item: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.readonly.insert(b"item".to_vec());
                let err = shell
                    .execute_string(b"for item in a b; do :; done")
                    .expect_err("readonly loop var");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn apply_prefix_assignments_readonly_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: RO: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.readonly.insert(b"RO".to_vec());
                let assignments = vec![(b"RO".to_vec(), b"newval".to_vec())];
                let err = apply_prefix_assignments(&mut shell, &assignments)
                    .expect_err("readonly should fail");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn group_command_executes_inline() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("{ X=hello; Y=world; }").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"X"), Some(b"hello" as &[u8]));
            assert_eq!(shell.get_var(b"Y"), Some(b"world" as &[u8]));
        });
    }

    #[test]
    fn function_def_registers_and_calls() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("myfn() { RESULT=ok; }; myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"RESULT"), Some(b"ok" as &[u8]));
            assert!(shell.functions.contains_key(&b"myfn".to_vec()));
        });
    }

    #[test]
    fn execute_program_stops_on_shell_not_running() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("exit 5; AFTER=bad").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 5);
            assert!(!shell.running);
            assert_eq!(shell.get_var(b"AFTER"), None);
        });
    }

    #[test]
    fn assignment_only_command_without_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("X=1 Y=2").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"X"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"Y"), Some(b"2" as &[u8]));
        });
    }

    #[test]
    fn split_bytes_helper() {
        assert_no_syscalls(|| {
            let parts: Vec<&[u8]> = split_bytes(b"a:b:c", b':').collect();
            assert_eq!(parts, vec![b"a" as &[u8], b"b", b"c"]);

            let single: Vec<&[u8]> = split_bytes(b"hello", b':').collect();
            assert_eq!(single, vec![b"hello" as &[u8]]);

            let empty: Vec<&[u8]> = split_bytes(b"", b':').collect();
            assert_eq!(empty, vec![b"" as &[u8]]);

            let trailing: Vec<&[u8]> = split_bytes(b"a:", b':').collect();
            assert_eq!(trailing, vec![b"a" as &[u8], b""]);
        });
    }
}
