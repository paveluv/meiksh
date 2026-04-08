use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::arena::StringArena;
use crate::builtin;
use crate::expand;
use crate::shell::{ChildWaitResult, FlowSignal, JobState, PendingControl, Shell, ShellError};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};
use crate::sys;

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
    shell.env.insert("LINENO".into(), item.line.to_string());
    if item.asynchronous {
        let stdin_override = if !shell.options.monitor {
            Some(
                sys::open_file("/dev/null", sys::O_RDONLY, 0)
                    .map_err(|e| shell.diagnostic(1, &e))?,
            )
        } else {
            None
        };
        let spawned = spawn_and_or(shell, &item.and_or, stdin_override)?;
        let last_pid = spawned.children.last().map(|c| c.pid).unwrap_or(0);
        let description = render_and_or(&item.and_or);
        let id = shell.register_background_job(description.into(), spawned.pgid, spawned.children);
        if shell.interactive {
            let msg = format!("[{id}] {last_pid}\n");
            let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
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
    let pid = sys::fork_process().map_err(|e| shell.diagnostic(1, &e))?;
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
        let _ = child_shell.reset_traps_for_subshell();
        let status = if node.rest.is_empty() {
            execute_pipeline(&mut child_shell, &node.first, false).unwrap_or(1)
        } else {
            execute_and_or(&mut child_shell, node).unwrap_or(1)
        };
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
            sys_eprintln!("real {:.2}", real_secs);
            sys_eprintln!("user {:.2}", user_secs);
            sys_eprintln!("sys {:.2}", sys_secs);
        }
        _ => {
            let fmt = |secs: f64| -> String {
                let mins = (secs / 60.0) as u64;
                let remainder = secs - (mins as f64 * 60.0);
                format!("{}m{:.3}s", mins, remainder)
            };
            sys_eprintln!("\nreal\t{}", fmt(real_secs));
            sys_eprintln!("user\t{}", fmt(user_secs));
            sys_eprintln!("sys\t{}", fmt(sys_secs));
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
        let (r, w) = sys::create_pipe().map_err(|e| shell.diagnostic(1, &e))?;
        Some((r, w))
    } else {
        Option::None
    };

    let pid = sys::fork_process().map_err(|e| shell.diagnostic(1, &e))?;
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
        let _ = child_shell.reset_traps_for_subshell();
        let status = execute_command(&mut child_shell, command).unwrap_or(1);
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
    command_desc: Option<&str>,
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
    command_desc: Option<&str>,
) -> Result<i32, ShellError> {
    let (last_status, _) = wait_for_children_inner(shell, spawned, command_desc)?;
    Ok(last_status)
}

fn wait_for_children_inner(
    shell: &mut Shell,
    mut spawned: SpawnedProcesses,
    command_desc: Option<&str>,
) -> Result<(i32, i32), ShellError> {
    let saved_foreground = handoff_foreground(spawned.pgid);
    let mut last_status = 0;
    let mut rightmost_nonzero = 0;
    for i in 0..spawned.children.len() {
        match shell.wait_for_child_pid(spawned.children[i].pid, false)? {
            ChildWaitResult::Exited(code) => {
                last_status = code;
                if code != 0 {
                    rightmost_nonzero = code;
                }
            }
            ChildWaitResult::Stopped(sig) => {
                restore_foreground(saved_foreground);
                let desc: Box<str> = command_desc.unwrap_or("").into();
                let children = std::mem::take(&mut spawned.children);
                let id = shell.register_background_job(desc, spawned.pgid, children);
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = JobState::Stopped(sig);
                if shell.interactive {
                    shell.jobs[idx].saved_termios = sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                    let msg = format!(
                        "[{id}] Stopped ({})\t{}\n",
                        sys::signal_name(sig),
                        shell.jobs[idx].command
                    );
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
                }
                return Ok((128 + sig, 128 + sig));
            }
            ChildWaitResult::Interrupted(_) => unreachable!("non-interruptible wait"),
        }
    }
    restore_foreground(saved_foreground);
    Ok((last_status, rightmost_nonzero))
}

fn wait_for_external_child(
    shell: &mut Shell,
    handle: &sys::ChildHandle,
    pgid: Option<sys::Pid>,
    command_desc: Option<&str>,
) -> Result<i32, ShellError> {
    let saved_foreground = handoff_foreground(pgid);
    match shell.wait_for_child_pid(handle.pid, false)? {
        ChildWaitResult::Exited(status) => {
            restore_foreground(saved_foreground);
            Ok(status)
        }
        ChildWaitResult::Stopped(sig) => {
            restore_foreground(saved_foreground);
            let desc: Box<str> = command_desc.unwrap_or("").into();
            let children = vec![handle.clone()];
            let id = shell.register_background_job(desc, pgid, children);
            let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
            shell.jobs[idx].state = JobState::Stopped(sig);
            if shell.interactive {
                shell.jobs[idx].saved_termios = sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                let msg = format!(
                    "[{id}] Stopped ({})\t{}\n",
                    sys::signal_name(sig),
                    shell.jobs[idx].command
                );
                let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
            }
            Ok(128 + sig)
        }
        ChildWaitResult::Interrupted(_) => unreachable!("non-interruptible wait"),
    }
}

fn execute_command(shell: &mut Shell, command: &Command) -> Result<i32, ShellError> {
    match command {
        Command::Simple(simple) => execute_simple(shell, simple),
        Command::Subshell(program) => {
            let pid = sys::fork_process().map_err(|e| shell.diagnostic(1, &e))?;
            if pid == 0 {
                let mut child_shell = shell.clone();
                let _ = child_shell.reset_traps_for_subshell();
                let status = match execute_nested_program(&mut child_shell, program) {
                    Ok(s) => s,
                    Err(error) => error.exit_status(),
                };
                sys::exit_process(status as sys::RawFd);
            }
            let ws = loop {
                match sys::wait_pid(pid, false) {
                    Ok(Some(ws)) => break ws,
                    Ok(None) => continue,
                    Err(e) if e.is_eintr() => continue,
                    Err(e) => return Err(shell.diagnostic(1, &e)),
                }
            };
            Ok(sys::decode_wait_status(ws.status))
        }
        Command::Group(program) => execute_nested_program(shell, program),
        Command::FunctionDef(function) => {
            shell.functions.insert(
                function.name.to_string(),
                (*function.body).clone(),
            );
            Ok(0)
        }
        Command::If(if_command) => execute_if(shell, if_command),
        Command::Loop(loop_command) => execute_loop(shell, loop_command),
        Command::For(for_command) => execute_for(shell, for_command),
        Command::Case(case_command) => execute_case(shell, case_command),
        Command::Redirected(command, redirections) => {
            execute_redirected(shell, command, redirections)
        }
    }
}

fn execute_redirected(
    shell: &mut Shell,
    command: &Command,
    redirections: &[crate::syntax::Redirection],
) -> Result<i32, ShellError> {
    let arena = StringArena::new();
    let expanded = expand_redirections(shell, redirections, &arena)?;
    if let Some(first) = expanded.first() {
        shell.lineno = first.line;
    }
    let guard = match apply_shell_redirections(&expanded, shell.options.noclobber) {
        Ok(guard) => guard,
        Err(error) => return Ok(shell.diagnostic(1, &error).exit_status()),
    };
    let result = execute_command(shell, command);
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
    let arena = StringArena::new();
    let values: Vec<String> = if let Some(items) = &for_command.items {
        let mut values = Vec::new();
        for item in items {
            for s in expand::expand_word(shell, item, &arena).map_err(|e| shell.expand_to_err(e))? {
                values.push(s.to_string());
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
            shell
                .set_var(&for_command.name, value)
                .map_err(|e| shell.diagnostic(1, &e))?;
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
    let arena = StringArena::new();
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
    name: Box<str>,
    value: Option<Box<str>>,
    was_exported: bool,
}

fn save_vars(shell: &Shell, assignments: &[(String, String)]) -> Vec<SavedVar> {
    assignments
        .iter()
        .map(|(name, _)| SavedVar {
            name: name.clone().into(),
            value: shell.get_var(name).map(|s| s.to_string().into()),
            was_exported: shell.exported.contains(name),
        })
        .collect()
}

fn apply_prefix_assignments(
    shell: &mut Shell,
    assignments: &[(String, String)],
) -> Result<(), ShellError> {
    for (name, value) in assignments {
        shell
            .set_var(name, value.clone())
            .map_err(|e| shell.diagnostic(1, &e))?;
    }
    Ok(())
}

fn restore_vars(shell: &mut Shell, saved: Vec<SavedVar>) {
    for entry in saved {
        let name: String = entry.name.into();
        match entry.value {
            Some(v) => {
                shell.env.insert(name.clone(), v.into());
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
    argv: &[String],
    assignments: &[(String, String)],
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
    let arena = StringArena::new();
    let ps4_raw = shell.get_var("PS4").unwrap_or("+ ").to_string();
    let prefix = expand::expand_parameter_text(shell, &ps4_raw, &arena).unwrap_or("+ ");
    let mut line = prefix.to_string();
    for (name, value) in &expanded.assignments {
        line.push_str(name);
        line.push('=');
        line.push_str(value);
        line.push(' ');
    }
    for (i, word) in expanded.argv.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        line.push_str(word);
    }
    line.push('\n');
    let _ = sys::write_all_fd(sys::STDERR_FILENO, line.as_bytes());
}

fn has_command_substitution(simple: &SimpleCommand) -> bool {
    simple.assignments.iter().any(|a| {
        let raw = &a.value.raw;
        raw.contains("$(") || raw.contains('`')
    }) || simple
        .words
        .iter()
        .any(|w| w.raw.contains("$(") || w.raw.contains('`'))
}

fn execute_simple(shell: &mut Shell, simple: &SimpleCommand) -> Result<i32, ShellError> {
    let arena = StringArena::new();
    let expanded = expand_simple(shell, simple, &arena)?;

    if let Some(first_word) = simple.words.first() {
        shell.lineno = first_word.line;
    }

    if !expanded.argv.is_empty() || !expanded.assignments.is_empty() {
        write_xtrace(shell, &expanded);
    }

    let owned_argv: Vec<String> = expanded.argv.iter().map(|s| s.to_string()).collect();
    let owned_assignments: Vec<(String, String)> = expanded
        .assignments
        .iter()
        .map(|&(n, v)| (n.to_string(), v.to_string()))
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
            Err(error) => return Ok(shell.diagnostic(1, &error).exit_status()),
        };
        for (name, value) in &owned_assignments {
            shell
                .set_var(name, value.clone())
                .map_err(|e| shell.diagnostic(1, &e))?;
        }
        drop(guard);
        return Ok(cmd_sub_status);
    }

    let is_special_builtin = builtin::is_special_builtin(&owned_argv[0]);
    let is_exec_no_cmd = is_special_builtin
        && owned_argv[0] == "exec"
        && !owned_argv.iter().skip(1).any(|a| a != "--");

    if is_exec_no_cmd {
        for redir in &expanded.redirections {
            shell.lineno = redir.line;
            apply_shell_redirection(redir, shell.options.noclobber)
                .map_err(|e| shell.diagnostic(1, &e))?;
        }
        return match run_builtin_flow(shell, &owned_argv, &owned_assignments) {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    } else if is_special_builtin {
        let _guard = apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
            .map_err(|e| shell.diagnostic(1, &e))?;
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
            Err(error) => return Ok(shell.diagnostic(1, &error).exit_status()),
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
                        Err(e) => Err(shell.diagnostic(1, &e)),
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
            if shell.readonly.contains(name.as_str()) {
                return Err(shell.diagnostic(1, format_args!("{name}: readonly variable")));
            }
        }
        let command_name = owned_argv[0].clone();
        let prepared = build_process_from_expanded(shell, expanded, owned_argv, owned_assignments)
            .expect("argv is non-empty");
        if !prepared.path_verified && !prepared.exec_path.contains('/') {
            let _guard = apply_shell_redirections(&prepared.redirections, prepared.noclobber).ok();
            sys_eprintln!("{}: not found", command_name);
            return Ok(127);
        }
        let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::NewGroup) {
            Ok(h) => h,
            Err(error) => return Ok(error.exit_status()),
        };
        let pgid = handle.pid;
        let _ = sys::set_process_group(pgid, pgid);
        let desc = prepared.argv.join(" ");
        let status = wait_for_external_child(shell, &handle, Some(pgid), Some(&desc))?;
        Ok(status)
    }
}

#[derive(Debug)]
struct ExpandedSimpleCommand<'a> {
    assignments: Vec<(&'a str, &'a str)>,
    argv: Vec<&'a str>,
    redirections: Vec<ExpandedRedirection<'a>>,
}

#[derive(Clone, Debug)]
struct ExpandedRedirection<'a> {
    fd: i32,
    kind: RedirectionKind,
    target: &'a str,
    here_doc_body: Option<&'a str>,
    line: usize,
}

#[derive(Debug, Clone)]
struct ProcessRedirection {
    fd: i32,
    kind: RedirectionKind,
    target: Box<str>,
    here_doc_body: Option<Box<str>>,
}

#[derive(Debug, Clone)]
struct PreparedProcess {
    exec_path: Box<str>,
    argv: Box<[Box<str>]>,
    child_env: Box<[(Box<str>, Box<str>)]>,
    redirections: Vec<ProcessRedirection>,
    noclobber: bool,
    path_verified: bool,
}

trait RedirectionRef {
    fn fd(&self) -> i32;
    fn kind(&self) -> RedirectionKind;
    fn target(&self) -> &str;
    fn here_doc_body(&self) -> Option<&str>;
}

impl<'a> RedirectionRef for ExpandedRedirection<'a> {
    fn fd(&self) -> i32 {
        self.fd
    }
    fn kind(&self) -> RedirectionKind {
        self.kind
    }
    fn target(&self) -> &str {
        self.target
    }
    fn here_doc_body(&self) -> Option<&str> {
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
    fn target(&self) -> &str {
        &self.target
    }
    fn here_doc_body(&self) -> Option<&str> {
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

fn is_declaration_utility(name: &str) -> bool {
    matches!(name, "export" | "readonly")
}

fn find_declaration_context(words: &[crate::syntax::Word]) -> bool {
    let mut i = 0;
    while i < words.len() {
        let raw = &*words[i].raw;
        if raw == "command" {
            i += 1;
            while i < words.len() && words[i].raw.starts_with('-') {
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
    arena: &'a StringArena,
) -> Result<ExpandedSimpleCommand<'a>, ShellError> {
    let mut assignments = Vec::new();
    for assignment in &simple.assignments {
        let value = expand::expand_assignment_value(shell, &assignment.value, arena)
            .map_err(|e| shell.expand_to_err(e))?;
        assignments.push((arena.intern_str(&assignment.name), value));
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
                .ok_or_else(|| shell.diagnostic(2, "missing here-document body"))?;
            let raw_body = if here_doc.strip_tabs {
                arena.intern(strip_heredoc_tabs(&here_doc.body))
            } else {
                &here_doc.body
            };
            let body = if here_doc.expand {
                expand::expand_here_document(shell, raw_body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_str(raw_body)
            };
            (arena.intern_str(&here_doc.delimiter), Some(body))
        } else {
            let target = expand::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != "-"
                && target.parse::<i32>().is_err()
            {
                return Err(
                    shell.diagnostic(1, "redirection target must be a file descriptor or '-'")
                );
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

fn expand_words_declaration<'a>(
    shell: &mut Shell,
    words: &[crate::syntax::Word],
    arena: &'a StringArena,
) -> Result<Vec<&'a str>, ShellError> {
    let mut result = Vec::new();
    let mut found_cmd = false;
    for word in words {
        if !found_cmd {
            result.extend(
                expand::expand_word(shell, word, arena).map_err(|e| shell.expand_to_err(e))?,
            );
            if result
                .last()
                .is_some_and(|s| !s.is_empty() && *s != "command")
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
    arena: &'a StringArena,
) -> Result<Vec<ExpandedRedirection<'a>>, ShellError> {
    let mut expanded = Vec::new();
    for redirection in redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, "missing here-document body"))?;
            let raw_body = if here_doc.strip_tabs {
                arena.intern(strip_heredoc_tabs(&here_doc.body))
            } else {
                &here_doc.body
            };
            let body = if here_doc.expand {
                expand::expand_here_document(shell, raw_body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_str(raw_body)
            };
            (arena.intern_str(&here_doc.delimiter), Some(body))
        } else {
            let target = expand::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != "-"
                && target.parse::<i32>().is_err()
            {
                return Err(
                    shell.diagnostic(1, "redirection target must be a file descriptor or '-'")
                );
            }
            (target, None)
        };
        expanded.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }
    Ok(expanded)
}

fn build_process_from_expanded(
    shell: &Shell,
    expanded: ExpandedSimpleCommand<'_>,
    owned_argv: Vec<String>,
    owned_assignments: Vec<(String, String)>,
) -> Result<PreparedProcess, ShellError> {
    let program = expanded
        .argv
        .first()
        .ok_or_else(|| shell.diagnostic(1, "empty command"))?;
    let prefix_path = expanded
        .assignments
        .iter()
        .find(|&&(name, _)| name == "PATH")
        .map(|&(_, value)| value);
    let resolved = resolve_command_path(shell, program, prefix_path);
    let path_verified = resolved.is_some();
    let exec_path = resolved
        .unwrap_or_else(|| PathBuf::from(program))
        .display()
        .to_string();
    let mut child_env = shell.env_for_child();
    child_env.extend(owned_assignments);
    let redirections = expanded
        .redirections
        .into_iter()
        .map(|r| ProcessRedirection {
            fd: r.fd,
            kind: r.kind,
            target: r.target.to_string().into(),
            here_doc_body: r.here_doc_body.map(|s| s.to_string().into()),
        })
        .collect();
    Ok(PreparedProcess {
        exec_path: exec_path.into(),
        argv: owned_argv
            .into_iter()
            .map(String::into_boxed_str)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        child_env: child_env
            .into_iter()
            .map(|(k, v)| (k.into_boxed_str(), v.into_boxed_str()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        redirections,
        noclobber: shell.options.noclobber,
        path_verified,
    })
}

fn file_needs_binary_rejection(path: &str) -> bool {
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

fn spawn_prepared(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    if !prepared.path_verified && !prepared.exec_path.is_empty() && prepared.exec_path.contains('/')
    {
        if sys::access_path(&prepared.exec_path, sys::F_OK).is_err() {
            sys_eprintln!("{}: not found", prepared.argv[0]);
            return Err(ShellError::Status(127));
        }
        if sys::access_path(&prepared.exec_path, sys::X_OK).is_err() {
            sys_eprintln!("{}: Permission denied", prepared.argv[0]);
            return Err(ShellError::Status(126));
        }
    }

    let prepared_redirections = prepare_redirections(&prepared.redirections, prepared.noclobber)
        .map_err(|e| shell.diagnostic(1, &e))?;

    let pid = sys::fork_process().map_err(|e| {
        sys_eprintln!("{}: {}", prepared.argv[0], e);
        ShellError::Status(1)
    })?;
    if pid == 0 {
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
            sys_eprintln!("{}: {}", prepared.argv[0], err);
            sys::exit_process(1);
        }

        for (key, value) in &prepared.child_env {
            let _ = sys::env_set_var(key, value);
        }

        if file_needs_binary_rejection(&prepared.exec_path) {
            sys_eprintln!("{}: cannot execute binary file", prepared.argv[0]);
            sys::exit_process(126);
        }

        match sys::exec_replace(&prepared.exec_path, &prepared.argv) {
            Err(err) if err.is_enoexec() => {
                let mut child_shell = shell.clone();
                let _ = child_shell.reset_traps_for_subshell();
                child_shell.shell_name = prepared.argv[0].clone();
                child_shell.positional = prepared.argv[1..]
                    .iter()
                    .map(|s| String::from(&**s))
                    .collect();
                let status = child_shell
                    .source_path(std::path::Path::new(&*prepared.exec_path))
                    .unwrap_or(126);
                sys::exit_process(status as sys::RawFd);
            }
            Err(err) if err.is_enoent() => {
                sys_eprintln!("{}: not found", prepared.argv[0]);
                sys::exit_process(127);
            }
            Err(err) => {
                sys_eprintln!("{}: {}", prepared.argv[0], err);
                sys::exit_process(126);
            }
            Ok(()) => sys::exit_process(0),
        }
    }

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
                let flags = if noclobber && redirection.kind() == RedirectionKind::Write {
                    sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC
                } else {
                    sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC
                };
                let fd = sys::open_file(redirection.target(), flags, 0o666)?;
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
                sys::write_all_fd(
                    write_fd,
                    redirection.here_doc_body().unwrap_or("").as_bytes(),
                )?;
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
                if redirection.target() == "-" {
                    prepared.actions.push(ChildFdAction::CloseFd {
                        target_fd: redirection.fd(),
                    });
                } else {
                    let source_fd = redirection
                        .target()
                        .parse::<i32>()
                        .expect("validated at expansion");
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
    program: &str,
    path_override: Option<&str>,
) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }

    let path = path_override
        .map(|s| s.to_string())
        .or_else(|| shell.get_var("PATH").map(|s| s.to_string()))
        .or_else(|| sys::env_var("PATH"))
        .unwrap_or_default();

    path.split(':')
        .filter(|segment| !segment.is_empty())
        .map(|segment| Path::new(segment).join(program))
        .find(|candidate| {
            sys::stat_path(&candidate.display().to_string())
                .map(|stat| stat.is_regular_file() && stat.is_executable())
                .unwrap_or(false)
        })
}

// PreparedProcess.build_command() is replaced by spawn_prepared_inner()
// PipelineInput is replaced by raw fd from ChildHandle.stdout_fd

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
            let flags = if noclobber && redirection.kind() == RedirectionKind::Write {
                sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC
            } else {
                sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC
            };
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Append => {
            let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::create_pipe()?;
            sys::write_all_fd(
                write_fd,
                redirection.here_doc_body().unwrap_or("").as_bytes(),
            )?;
            sys::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd())?;
        }
        RedirectionKind::ReadWrite => {
            let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::DupInput | RedirectionKind::DupOutput => {
            if redirection.target() == "-" {
                close_shell_fd(redirection.fd())?;
            } else {
                let source_fd = redirection
                    .target()
                    .parse::<i32>()
                    .expect("validated at expansion");
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

fn strip_heredoc_tabs(body: &str) -> String {
    body.lines()
        .map(|line| line.trim_start_matches('\t'))
        .collect::<Vec<_>>()
        .join("\n")
        + if body.ends_with('\n') { "\n" } else { "" }
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

fn case_pattern_matches(text: &str, pattern: &str) -> bool {
    expand::pattern_matches(text, pattern)
}

#[cfg(test)]
fn render_program(program: &Program) -> String {
    let mut buf = String::new();
    render_program_into(program, &mut buf);
    buf
}

fn render_program_into(program: &Program, buf: &mut String) {
    for (index, item) in program.items.iter().enumerate() {
        if index > 0 {
            buf.push('\n');
        }
        render_list_item_into(item, buf);
    }
}

#[cfg(test)]
fn render_list_item(item: &ListItem) -> String {
    let mut buf = String::new();
    render_list_item_into(item, &mut buf);
    buf
}

fn render_list_item_into(item: &ListItem, buf: &mut String) {
    render_and_or_into(&item.and_or, buf);
    if item.asynchronous {
        buf.push_str(" &");
    }
}

fn render_and_or(and_or: &AndOr) -> String {
    let mut buf = String::new();
    render_and_or_into(and_or, &mut buf);
    buf
}

fn render_and_or_into(and_or: &AndOr, buf: &mut String) {
    render_pipeline_into(&and_or.first, buf);
    for (op, pipeline) in &and_or.rest {
        match op {
            LogicalOp::And => buf.push_str(" && "),
            LogicalOp::Or => buf.push_str(" || "),
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

fn render_command(command: &Command) -> String {
    let mut buf = String::new();
    render_command_into(command, &mut buf);
    buf
}

fn render_command_into(command: &Command, buf: &mut String) {
    match command {
        Command::Simple(simple) => render_simple_into(simple, buf),
        Command::Subshell(program) => {
            buf.push('(');
            render_program_into(program, buf);
            buf.push(')');
        }
        Command::Group(program) => {
            buf.push_str("{ ");
            render_program_into(program, buf);
            buf.push_str("; }");
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

fn render_pipeline(pipeline: &Pipeline) -> String {
    let mut buf = String::new();
    render_pipeline_into(pipeline, &mut buf);
    buf
}

fn render_pipeline_into(pipeline: &Pipeline, buf: &mut String) {
    if pipeline.negated {
        buf.push_str("! ");
    }
    for (i, command) in pipeline.commands.iter().enumerate() {
        if i > 0 {
            buf.push_str(" | ");
        }
        render_command_into(command, buf);
    }
}

#[cfg(test)]
fn render_function(function: &FunctionDef) -> String {
    let mut buf = String::new();
    render_function_into(function, &mut buf);
    buf
}

fn render_function_into(function: &FunctionDef, buf: &mut String) {
    buf.push_str(&function.name);
    buf.push_str("() ");
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
fn render_if(if_command: &IfCommand) -> String {
    let mut buf = String::new();
    render_if_into(if_command, &mut buf);
    buf
}

fn render_if_into(if_command: &IfCommand, buf: &mut String) {
    buf.push_str("if ");
    render_program_into(&if_command.condition, buf);
    buf.push_str("\nthen\n");
    render_program_into(&if_command.then_branch, buf);
    for branch in &if_command.elif_branches {
        buf.push_str("\nelif ");
        render_program_into(&branch.condition, buf);
        buf.push_str("\nthen\n");
        render_program_into(&branch.body, buf);
    }
    if let Some(else_branch) = &if_command.else_branch {
        buf.push_str("\nelse\n");
        render_program_into(else_branch, buf);
    }
    buf.push_str("\nfi");
}

#[cfg(test)]
fn render_loop(loop_command: &LoopCommand) -> String {
    let mut buf = String::new();
    render_loop_into(loop_command, &mut buf);
    buf
}

fn render_loop_into(loop_command: &LoopCommand, buf: &mut String) {
    let keyword = match loop_command.kind {
        LoopKind::While => "while",
        LoopKind::Until => "until",
    };
    buf.push_str(keyword);
    buf.push(' ');
    render_program_into(&loop_command.condition, buf);
    buf.push_str("\ndo\n");
    render_program_into(&loop_command.body, buf);
    buf.push_str("\ndone");
}

#[cfg(test)]
fn render_for(for_command: &ForCommand) -> String {
    let mut buf = String::new();
    render_for_into(for_command, &mut buf);
    buf
}

fn render_for_into(for_command: &ForCommand, buf: &mut String) {
    buf.push_str("for ");
    buf.push_str(&for_command.name);
    if let Some(items) = &for_command.items {
        buf.push_str(" in");
        for item in items {
            buf.push(' ');
            buf.push_str(&item.raw);
        }
    }
    buf.push_str("\ndo\n");
    render_program_into(&for_command.body, buf);
    buf.push_str("\ndone");
}

#[cfg(test)]
fn render_case(case_command: &CaseCommand) -> String {
    let mut buf = String::new();
    render_case_into(case_command, &mut buf);
    buf
}

fn render_case_into(case_command: &CaseCommand, buf: &mut String) {
    buf.push_str("case ");
    buf.push_str(&case_command.word.raw);
    buf.push_str(" in");
    for arm in &case_command.arms {
        buf.push('\n');
        for (i, pattern) in arm.patterns.iter().enumerate() {
            if i > 0 {
                buf.push_str(" | ");
            }
            buf.push_str(&pattern.raw);
        }
        buf.push_str(")\n");
        render_program_into(&arm.body, buf);
        if arm.fallthrough {
            buf.push_str("\n;&");
        } else {
            buf.push_str("\n;;");
        }
    }
    buf.push_str("\nesac");
}

#[cfg(test)]
fn render_simple(simple: &SimpleCommand) -> String {
    let mut buf = String::new();
    render_simple_into(simple, &mut buf);
    buf
}

fn render_simple_into(simple: &SimpleCommand, buf: &mut String) {
    let mut base = String::new();
    for (i, assignment) in simple.assignments.iter().enumerate() {
        if i > 0 {
            base.push(' ');
        }
        base.push_str(&assignment.name);
        base.push('=');
        base.push_str(&assignment.value.raw);
    }
    for word in &simple.words {
        if !base.is_empty() {
            base.push(' ');
        }
        base.push_str(&word.raw);
    }
    render_command_line_with_redirections_into(base, &simple.redirections, buf);
}

fn render_redirections_into(
    redirections: &[crate::syntax::Redirection],
    redir_buf: &mut String,
    heredocs: &mut Vec<String>,
) {
    for (i, redirection) in redirections.iter().enumerate() {
        if i > 0 {
            redir_buf.push(' ');
        }
        render_redirection_operator_into(redirection, redir_buf);
        if let Some(here_doc) = &redirection.here_doc {
            heredocs.push(render_here_doc_body(here_doc));
        }
    }
}

fn render_redirection_operator_into(redirection: &crate::syntax::Redirection, buf: &mut String) {
    if let Some(fd) = redirection.fd {
        use std::fmt::Write;
        let _ = write!(buf, "{fd}");
    }
    let op = match redirection.kind {
        RedirectionKind::Read => "<",
        RedirectionKind::Write => ">",
        RedirectionKind::ClobberWrite => ">|",
        RedirectionKind::Append => ">>",
        RedirectionKind::HereDoc => {
            if redirection
                .here_doc
                .as_ref()
                .is_some_and(|here_doc| here_doc.strip_tabs)
            {
                "<<-"
            } else {
                "<<"
            }
        }
        RedirectionKind::ReadWrite => "<>",
        RedirectionKind::DupInput => "<&",
        RedirectionKind::DupOutput => ">&",
    };
    buf.push_str(op);
    buf.push_str(&redirection.target.raw);
}

fn render_here_doc_body(here_doc: &HereDoc) -> String {
    if here_doc.body.ends_with('\n') {
        format!("{}{}", here_doc.body, here_doc.delimiter)
    } else {
        format!("{}\n{}", here_doc.body, here_doc.delimiter)
    }
}

fn render_command_line_with_redirections_into(
    base: String,
    redirections: &[crate::syntax::Redirection],
    buf: &mut String,
) {
    let mut redir_text = String::new();
    let mut heredocs = Vec::new();
    render_redirections_into(redirections, &mut redir_text, &mut heredocs);
    buf.push_str(&base);
    if !redir_text.is_empty() {
        if !base.is_empty() {
            buf.push(' ');
        }
        buf.push_str(&redir_text);
    }
    if !heredocs.is_empty() {
        buf.push('\n');
        buf.push_str(&heredocs.join("\n"));
    }
}

fn render_redirected_command_into(
    command: &Command,
    redirections: &[crate::syntax::Redirection],
    buf: &mut String,
) {
    let base = render_command(command);
    render_command_line_with_redirections_into(base, redirections, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{
        ArgMatcher, TraceEntry, TraceResult, assert_no_syscalls, run_trace, t, t_fork,
    };
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    fn parse_test(source: &str) -> Result<crate::syntax::Program, crate::syntax::ParseError> {
        crate::syntax::parse(source)
    }

    fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: "meiksh".into(),
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
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive: false,
            errexit_suppressed: false,
            pid: 0,
            lineno: 0,
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
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let pipeline = Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
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
                            "stat",
                            vec![ArgMatcher::Str("/usr/bin/printf".into()), ArgMatcher::Any],
                            TraceResult::StatFile(0o755),
                        ),
                        t_fork(
                            TraceResult::Pid(1500),
                            vec![
                                t(
                                    "setpgid",
                                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                                    TraceResult::Int(0),
                                ),
                                t(
                                    "open",
                                    vec![
                                        ArgMatcher::Str("/usr/bin/printf".into()),
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
                                    vec![
                                        ArgMatcher::Str("/usr/bin/printf".into()),
                                        ArgMatcher::Any,
                                    ],
                                    TraceResult::Int(0),
                                ),
                            ],
                        ),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(1500), ArgMatcher::Int(1500)],
                            TraceResult::Int(0),
                        ),
                        t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                        t(
                            "waitpid",
                            vec![
                                ArgMatcher::Int(1500),
                                ArgMatcher::Any,
                                ArgMatcher::Int(sys::WUNTRACED as i64),
                            ],
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
                        t(
                            "stat",
                            vec![ArgMatcher::Str("/usr/bin/wc".into()), ArgMatcher::Any],
                            TraceResult::StatFile(0o755),
                        ),
                        t_fork(
                            TraceResult::Pid(1501),
                            vec![
                                t(
                                    "setpgid",
                                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                                    TraceResult::Int(0),
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
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(1501), ArgMatcher::Int(1501)],
                            TraceResult::Int(0),
                        ),
                        t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                        t(
                            "waitpid",
                            vec![
                                ArgMatcher::Int(1501),
                                ArgMatcher::Any,
                                ArgMatcher::Int(sys::WUNTRACED as i64),
                            ],
                            TraceResult::Status(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
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
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let pipeline = Pipeline {
                    negated: true,
                    timed: TimedMode::Off,
                    commands: vec![
                        Command::Simple(SimpleCommand {
                            words: vec![
                                Word {
                                    raw: "printf".into(), line: 0 },
                                Word { raw: "ok".into(), line: 0 },
                            ]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        }),
                        Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "wc".into(), line: 0 }, Word { raw: "-c".into(), line: 0 }]
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
                let arena = StringArena::new();
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
                shell.env.insert("PATH".into(), String::new());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(arena.intern_str("ASSIGN_VAR"), arena.intern_str("works"))],
                    argv: vec![arena.intern_str("echo"), arena.intern_str("hello")],
                    redirections: Vec::new(),
                };
                let owned_argv: Vec<String> = expanded.argv.iter().map(|s| s.to_string()).collect();
                let owned_assignments: Vec<(String, String)> = expanded
                    .assignments
                    .iter()
                    .map(|&(n, v)| (n.to_string(), v.to_string()))
                    .collect();
                let prepared =
                    build_process_from_expanded(&shell, expanded, owned_argv, owned_assignments)
                        .expect("process");
                assert_eq!(
                    &*prepared.child_env,
                    &[(Box::from("ASSIGN_VAR"), Box::from("works"))] as &[(Box<str>, Box<str>)]
                );
                assert_eq!(
                    &*prepared.argv,
                    &[Box::from("echo"), Box::from("hello")] as &[Box<str>]
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
                    exec_path: "/tmp/script.sh".into(),
                    argv: vec!["/tmp/script.sh".into(), "arg1".into()].into_boxed_slice(),
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
                    exec_path: "/nonexistent/missing".into(),
                    argv: vec!["missing".into()].into_boxed_slice(),
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
                words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: Some(5),
                        kind: RedirectionKind::ReadWrite,
                        target: Word { raw: "rw".into(), line: 0 },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(0),
                        kind: RedirectionKind::DupInput,
                        target: Word { raw: "5".into(), line: 0 },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(1),
                        kind: RedirectionKind::DupOutput,
                        target: Word { raw: "-".into(), line: 0 },
                        here_doc: None,
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            assert!(rendered.contains("5<>rw"));
            assert!(rendered.contains("0<&5"));
            assert!(rendered.contains("1>&-"));
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
                            target: "-",
                            here_doc_body: None, line: 0 },
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::ReadWrite,
                            target: "/tmp/rw.txt",
                            here_doc_body: None, line: 0 },
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
            ],
            || {
                let arena = StringArena::new();
                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word { raw: "cat".into(), line: 0 }].into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: None,
                            kind: RedirectionKind::HereDoc,
                            target: Word { raw: "EOF".into(), line: 0 },
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
                        words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: Some(1),
                            kind: RedirectionKind::DupOutput,
                            target: Word { raw: "bad".into(), line: 0 },
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
                        target: Word { raw: "EOF".into(), line: 0 },
                        here_doc: Some(HereDoc {
                            delimiter: "EOF".into(),
                            body: "hello $USER".into(),
                            expand: true,
                            strip_tabs: false, body_line: 0 }),
                    }],
                    &arena,
                )
                .expect("expand heredoc redirection");
                assert_eq!(expanded[0].target, "EOF");
                assert_eq!(expanded[0].here_doc_body, Some("hello "));

                let mut shell = test_shell();
                let literal = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word { raw: "EOF".into(), line: 0 },
                        here_doc: Some(HereDoc {
                            delimiter: "EOF".into(),
                            body: "hello $USER".into(),
                            expand: false,
                            strip_tabs: false, body_line: 0 }),
                    }],
                    &arena,
                )
                .expect("literal heredoc redirection");
                assert_eq!(literal[0].here_doc_body, Some("hello $USER"));

                let mut shell = test_shell();
                let error = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word { raw: "EOF".into(), line: 0 },
                        here_doc: None,
                    }],
                    &arena,
                )
                .expect_err("missing expanded heredoc body");
                assert_eq!(error.exit_status(), 2);
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
                        target: "EOF",
                        here_doc_body: Some("body\n"), line: 0 }],
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
                        target: "EOF",
                        here_doc_body: Some("body\n"), line: 0 }],
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.to_string().is_empty());
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
                                words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false, line: 0 }]
                .into_boxed_slice(),
            };

            let function = FunctionDef {
                name: "greet".into(),
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
            assert!(render_program(&program).contains("true"));
            assert!(render_function(&function).contains("greet()"));
            assert!(render_if(&if_command).starts_with("if "));
            assert!(render_loop(&loop_command).starts_with("while "));

            let simple = SimpleCommand {
                assignments: vec![Assignment {
                    name: "X".into(),
                    value: Word { raw: "1".into(), line: 0 },
                }]
                .into_boxed_slice(),
                words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word { raw: "out".into(), line: 0 },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            };
            assert_eq!(render_simple(&simple), "X=1 echo >out");

            let multi_assign = SimpleCommand {
                assignments: vec![
                    Assignment {
                        name: "A".into(),
                        value: Word { raw: "1".into(), line: 0 },
                    },
                    Assignment {
                        name: "B".into(),
                        value: Word { raw: "2".into(), line: 0 },
                    },
                ]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),

                redirections: vec![].into_boxed_slice(),
            };
            assert_eq!(render_simple(&multi_assign), "A=1 B=2");

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
            assert!(render_pipeline(&pipeline).starts_with("! "));
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
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: Vec::new().into_boxed_slice(),
                },
                asynchronous: false, line: 0 }]
            .into_boxed_slice(),
        };

        run_trace(
            vec![
                // Command 0 (Subshell): pipe_stdout=true, NewGroup
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
                // Command 1 (Group): stdin=200, pipe_stdout=false, Join(1000)
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
                // Wait
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
                                    words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                                    ..SimpleCommand::default()
                                })]
                                .into_boxed_slice(),
                            },
                            rest: Vec::new().into_boxed_slice(),
                        },
                        asynchronous: true, line: 0 },
                    ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: "false".into(), line: 0 }]
                                    .into_boxed_slice(),
                                    ..SimpleCommand::default()
                                })]
                                .into_boxed_slice(),
                            },
                            rest: Vec::new().into_boxed_slice(),
                        },
                        asynchronous: false, line: 0 },
                ]
                .into_boxed_slice(),
            };
            assert_eq!(render_list_item(&async_program.items[0]), "true &");
            assert_eq!(render_program(&async_program), "true &\nfalse");

            let heredoc_program = parse_test(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
            assert_eq!(render_program(&heredoc_program), ": <<EOF\nhello\nEOF");
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
            assert_eq!(shell.get_var("VALUE"), Some("yes"));

            let mut shell = test_shell();
            let while_program = parse_test(
                "COUNTER=1; while case $COUNTER in 0) false ;; *) true ;; esac; do COUNTER=0; FLAG=done; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &while_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FLAG"), Some("done"));

            let mut shell = test_shell();
            let until_program = parse_test(
                "READY=; until case $READY in yes) true ;; *) false ;; esac; do READY=yes; VALUE=ready; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &until_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("ready"));
        });
    }

    #[test]
    fn execute_for_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("for item in a b c; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("LAST"), Some("c"));

            let mut shell = test_shell();
            shell.positional = vec!["alpha".into(), "beta".into()];
            let program = parse_test("for item; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("LAST"), Some("beta"));
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
            assert_eq!(shell.get_var("VALUE"), Some("yes"));

            let mut shell = test_shell();
            let program = parse_test("name=zeta; case $name in alpha|beta) VALUE=hit ;; esac")
                .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case a in a) A=1 ;& b) B=2 ;; c) C=3 ;; esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("A"), Some("1"));
            assert_eq!(shell.get_var("B"), Some("2"));
            assert_eq!(shell.get_var("C"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case x in x) V=one ;& y) V=two ;& z) V=three ;& esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("V"), Some("three"));
        });
    }

    #[test]
    fn execute_if_covers_then_and_else_branches() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
            shell.exported.insert("PATH".into());

            let if_program =
                parse_test("if true; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("yes"));

            let mut shell = test_shell();
            let if_program =
                parse_test("if false; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("no"));

            let mut shell = test_shell();
            let if_program = parse_test(
                "if false; then VALUE=yes; elif false; then VALUE=maybe; else VALUE=no; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("no"));
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
                        words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
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
                                raw: "false".into(), line: 0 }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                )]
                .into_boxed_slice(),
            });
            assert!(render.contains("&&"));
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
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: Vec::new().into_boxed_slice(),
                },
                asynchronous: false, line: 0 }]
            .into_boxed_slice(),
        };
        let pipeline = Pipeline {
            negated: false,
            timed: TimedMode::Off,
            commands: vec![
                Command::FunctionDef(FunctionDef {
                    name: "f".into(),
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
                    name: "item".into(),
                    items: Some(vec![Word { raw: "a".into(), line: 0 }].into_boxed_slice()),
                    body: Program::default(),
                }),
                Command::Case(CaseCommand {
                    word: Word { raw: "item".into(), line: 0 },
                    arms: vec![crate::syntax::CaseArm {
                        patterns: vec![Word { raw: "item".into(), line: 0 }].into_boxed_slice(),
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
                // Command 0 (FunctionDef): pipe_stdout=true, NewGroup
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
                // Command 1 (If): stdin=200, pipe_stdout=true, Join(1000)
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
                // Command 2 (Loop): stdin=202, pipe_stdout=true, Join(1000)
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
                // Command 3 (For): stdin=204, pipe_stdout=true, Join(1000)
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
                // Command 4 (Case): stdin=206, pipe_stdout=false, Join(1000)
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
                // Wait for all children
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

    #[test]
    fn exec_render_helpers_cover_remaining_variants() {
        assert_no_syscalls(|| {
            let for_command = ForCommand {
                name: "item".into(),
                items: Some(
                    vec![Word { raw: "a".into(), line: 0 }, Word { raw: "b".into(), line: 0 }].into_boxed_slice(),
                ),
                body: Program::default(),
            };
            assert!(render_for(&for_command).contains("in a b"));
            assert!(
                render_for(&ForCommand {
                    name: "item".into(),
                    items: None,
                    body: Program::default()
                })
                .starts_with("for item\n")
            );

            let simple = SimpleCommand {
                words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::Read,
                        target: Word { raw: "in".into(), line: 0 },
                        here_doc: None,
                    },
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::Append,
                        target: Word { raw: "out".into(), line: 0 },
                        here_doc: None,
                    },
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word { raw: "EOF".into(), line: 0 },
                        here_doc: Some(crate::syntax::HereDoc {
                            delimiter: "EOF".into(),
                            body: "body\n".into(),
                            expand: false,
                            strip_tabs: false, body_line: 0 }),
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            assert!(rendered.contains("<in"));
            assert!(rendered.contains(">>out"));
            assert!(rendered.contains("<<EOF"));
            assert!(rendered.contains("body\nEOF"));

            let strip_tabs = SimpleCommand {
                words: vec![Word { raw: "cat".into(), line: 0 }].into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::HereDoc,
                    target: Word { raw: "EOF".into(), line: 0 },
                    here_doc: Some(crate::syntax::HereDoc {
                        delimiter: "EOF".into(),
                        body: "body".into(),
                        expand: false,
                        strip_tabs: true, body_line: 0 }),
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&strip_tabs);
            assert!(rendered.contains("<<-EOF"));
            assert!(rendered.contains("body\nEOF"));

            let case_command = CaseCommand {
                word: Word {
                    raw: "$item".into(), line: 0 },
                arms: vec![crate::syntax::CaseArm {
                    patterns: vec![Word { raw: "a*".into(), line: 0 }, Word { raw: "b".into(), line: 0 }]
                        .into_boxed_slice(),
                    body: Program::default(),
                    fallthrough: false,
                }]
                .into_boxed_slice(),
            };
            assert!(render_case(&case_command).contains("a* | b)"));
            let ft_case = CaseCommand {
                word: Word { raw: "x".into(), line: 0 },
                arms: vec![
                    crate::syntax::CaseArm {
                        patterns: vec![Word { raw: "a".into(), line: 0 }].into_boxed_slice(),
                        body: Program::default(),
                        fallthrough: true,
                    },
                    crate::syntax::CaseArm {
                        patterns: vec![Word { raw: "b".into(), line: 0 }].into_boxed_slice(),
                        body: Program::default(),
                        fallthrough: false,
                    },
                ]
                .into_boxed_slice(),
            };
            let rendered = render_case(&ft_case);
            assert!(rendered.contains(";&"));
            assert!(rendered.contains(";;"));
            assert!(
                render_pipeline(&Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Case(case_command)].into_boxed_slice(),
                })
                .contains("case ")
            );
        });
    }

    #[test]
    fn case_pattern_matching_covers_wildcards_and_classes() {
        assert_no_syscalls(|| {
            assert!(case_pattern_matches("beta", "b*"));
            assert!(case_pattern_matches("beta", "b?t[ab]"));
            assert!(case_pattern_matches("x", "[!ab]"));
            assert!(case_pattern_matches("*", "\\*"));
            assert!(case_pattern_matches("-", "[\\-]"));
            assert!(case_pattern_matches("b", "[a-c]"));
            assert!(!case_pattern_matches("[", "[a"));
            assert!(!case_pattern_matches("x", "["));
            assert!(!case_pattern_matches("beta", "a*"));
            assert!(!case_pattern_matches("a", "[!ab]"));

            assert!(case_pattern_matches("a", "[[:alpha:]]"));
            assert!(case_pattern_matches("Z", "[[:alpha:]]"));
            assert!(!case_pattern_matches("5", "[[:alpha:]]"));
            assert!(case_pattern_matches("3", "[[:alnum:]]"));
            assert!(!case_pattern_matches("!", "[[:alnum:]]"));
            assert!(case_pattern_matches(" ", "[[:blank:]]"));
            assert!(case_pattern_matches("\t", "[[:blank:]]"));
            assert!(!case_pattern_matches("a", "[[:blank:]]"));
            assert!(case_pattern_matches("\x01", "[[:cntrl:]]"));
            assert!(!case_pattern_matches("a", "[[:cntrl:]]"));
            assert!(case_pattern_matches("9", "[[:digit:]]"));
            assert!(!case_pattern_matches("a", "[[:digit:]]"));
            assert!(case_pattern_matches("!", "[[:graph:]]"));
            assert!(!case_pattern_matches(" ", "[[:graph:]]"));
            assert!(case_pattern_matches("a", "[[:lower:]]"));
            assert!(!case_pattern_matches("A", "[[:lower:]]"));
            assert!(case_pattern_matches(" ", "[[:print:]]"));
            assert!(case_pattern_matches("a", "[[:print:]]"));
            assert!(!case_pattern_matches("\x01", "[[:print:]]"));
            assert!(case_pattern_matches(".", "[[:punct:]]"));
            assert!(!case_pattern_matches("a", "[[:punct:]]"));
            assert!(case_pattern_matches("\n", "[[:space:]]"));
            assert!(!case_pattern_matches("a", "[[:space:]]"));
            assert!(case_pattern_matches("A", "[[:upper:]]"));
            assert!(!case_pattern_matches("a", "[[:upper:]]"));
            assert!(case_pattern_matches("f", "[[:xdigit:]]"));
            assert!(!case_pattern_matches("g", "[[:xdigit:]]"));
            assert!(!case_pattern_matches("a", "[[:bogus:]]"));
            assert!(case_pattern_matches("x", "[[:x]"));
        });
    }

    #[test]
    fn render_and_or_covers_or_and_for_variants() {
        assert_no_syscalls(|| {
            let render = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: "false".into(), line: 0 }]
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
                        commands: vec![Command::For(ForCommand {
                            name: "item".into(),
                            items: Some(vec![Word { raw: "a".into(), line: 0 }].into_boxed_slice()),
                            body: Program::default(),
                        })]
                        .into_boxed_slice(),
                    },
                )]
                .into_boxed_slice(),
            });
            assert!(render.contains("||"));
            assert!(render.contains("for item in a"));
        });
    }

    #[test]
    fn loop_and_function_exit_behavior() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = parse_test("if false; then VALUE=yes; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);

            let mut shell = test_shell();
            let for_program = parse_test("for item in a b; do exit 9; done").expect("parse");
            let status = execute_program(&mut shell, &for_program).expect("exec");
            assert_eq!(status, 9);
            assert!(!shell.running);
            assert_eq!(shell.get_var("item"), Some("a"));

            let mut shell = test_shell();
            let loop_program = parse_test("while true; do exit 7; done").expect("parse");
            let status = execute_program(&mut shell, &loop_program).expect("exec");
            assert_eq!(status, 7);
            assert!(!shell.running);

            let mut shell = test_shell();
            let program = parse_test("greet() { RESULT=$X; }; X=ok greet").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("RESULT"), Some("ok"));
        });
    }

    #[test]
    fn control_flow_propagates_across_functions_and_loops() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: break: only meaningful in a loop\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let program = parse_test("f() { return 6; VALUE=bad; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 6);
                assert_eq!(shell.get_var("VALUE"), None);
                assert_eq!(shell.pending_control, None);

                let mut shell = test_shell();
                let program = parse_test(
                "for outer in x y; do for inner in a b; do continue 2; VALUE=bad; done; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("VALUE"), None);
                assert_eq!(shell.get_var("outer"), Some("y"));

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x y; do for inner in a b; do break 2; done; VALUE=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("VALUE"), None);
                assert_eq!(shell.get_var("outer"), Some("x"));

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
                assert_eq!(shell.get_var("AFTER"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do while continue 2; do printf no; done; AFTER=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("AFTER"), None);

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
                assert_eq!(shell.get_var("COUNT"), Some("0"));

                let mut shell = test_shell();
                let program = parse_test(
                "COUNT=1; while true; do case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("COUNT"), Some("0"));

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
                assert_eq!(shell.get_var("DONE"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do for inner in y; do continue 2; done; DONE=no; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("DONE"), None);

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
    fn render_simple_handles_clobber_write() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: Some(1),
                    kind: RedirectionKind::ClobberWrite,
                    target: Word { raw: "out".into(), line: 0 },
                    here_doc: None,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(render_simple(&simple).contains(">|out"));
        });
    }

    #[test]
    fn current_shell_redirections_write_append_read() {
        run_trace(
            vec![
                // apply_shell_redirections: save fd 42, open write, dup2+close, open append, dup2+close
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Fd(92),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/redir/write.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(100),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(100), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(100)], TraceResult::Int(0)),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/redir/append.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(101),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(101), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(101)], TraceResult::Int(0)),
                // drop(guard): restore fd 42 from saved 92
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(92), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(92)], TraceResult::Int(0)),
                // apply_shell_redirection Read
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/redir/input.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(102),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(102), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(102)], TraceResult::Int(0)),
                // apply_shell_redirection ReadWrite
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/redir/rw.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(103),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(103), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(103)], TraceResult::Int(0)),
            ],
            || {
                let target_fd = 42;
                let guard = apply_shell_redirections(
                    &[
                        ExpandedRedirection {
                            fd: target_fd,
                            kind: RedirectionKind::Write,
                            target: "/redir/write.txt",
                            here_doc_body: None, line: 0 },
                        ExpandedRedirection {
                            fd: target_fd,
                            kind: RedirectionKind::Append,
                            target: "/redir/append.txt",
                            here_doc_body: None, line: 0 },
                    ],
                    false,
                )
                .expect("redir guard");
                drop(guard);

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::Read,
                        target: "/redir/input.txt",
                        here_doc_body: None, line: 0 },
                    false,
                )
                .expect("read redirection");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::ReadWrite,
                        target: "/redir/rw.txt",
                        here_doc_body: None, line: 0 },
                    false,
                )
                .expect("readwrite redirection");
            },
        );
    }

    #[test]
    fn current_shell_heredoc_redirections() {
        run_trace(
            vec![
                // apply_shell_redirection HereDoc
                t("pipe", vec![], TraceResult::Fds(104, 105)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(105), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(105)], TraceResult::Int(0)),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(104), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                t("close", vec![ArgMatcher::Fd(104)], TraceResult::Int(0)),
            ],
            || {
                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::HereDoc,
                        target: "EOF",
                        here_doc_body: Some("body\n"), line: 0 },
                    false,
                )
                .expect("heredoc redirection");
            },
        );
    }

    #[test]
    fn current_shell_heredoc_write_error() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(104, 105)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(105), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Err(sys::EIO),
                ),
            ],
            || {
                let err = apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::HereDoc,
                        target: "EOF",
                        here_doc_body: Some("body\n"), line: 0 },
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.to_string().is_empty());
            },
        );
    }

    #[test]
    fn current_shell_dup_and_close_redirections() {
        run_trace(
            vec![
                // apply_shell_redirection DupOutput "1"
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Fd(42)],
                    TraceResult::Int(42),
                ),
                // apply_shell_redirection DupOutput "-"
                t("close", vec![ArgMatcher::Fd(42)], TraceResult::Int(0)),
            ],
            || {
                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "1",
                        here_doc_body: None, line: 0 },
                    false,
                )
                .expect("dup output");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "-",
                        here_doc_body: None, line: 0 },
                    false,
                )
                .expect("close dup output");
            },
        );
    }

    #[test]
    fn current_shell_noclobber_and_guard_cleanup() {
        run_trace(
            vec![
                // Write with noclobber → open returns EEXIST
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/redir/noclobber.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EEXIST),
                ),
                // Guard drop with saved=(99, None) → close(99)
                t("close", vec![ArgMatcher::Fd(99)], TraceResult::Int(0)),
                // apply_shell_redirections with fcntl returning EBADF for high fd → treated as absent
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(123_456),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Err(sys::EBADF),
                ),
                t(
                    "close",
                    vec![ArgMatcher::Fd(123_456)],
                    TraceResult::Err(sys::EBADF),
                ),
                t(
                    "close",
                    vec![ArgMatcher::Fd(123_456)],
                    TraceResult::Err(sys::EBADF),
                ),
                // apply_shell_redirections with fcntl failure (errno 22)
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Err(sys::EINVAL),
                ),
            ],
            || {
                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::Write,
                        target: "/redir/noclobber.txt",
                        here_doc_body: None, line: 0 },
                    true,
                )
                .expect_err("noclobber");

                drop(ShellRedirectionGuard {
                    saved: vec![(99, None)],
                });

                let guard = apply_shell_redirections(
                    &[ExpandedRedirection {
                        fd: 123_456,
                        kind: RedirectionKind::DupOutput,
                        target: "-",
                        here_doc_body: None, line: 0 }],
                    false,
                )
                .expect("invalid fd is treated as absent");
                drop(guard);

                apply_shell_redirections(
                    &[ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "-",
                        here_doc_body: None, line: 0 }],
                    false,
                )
                .expect_err("dup failure");
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_dup_error() {
        run_trace(
            vec![t(
                "dup2",
                vec![ArgMatcher::Fd(-1), ArgMatcher::Fd(56)],
                TraceResult::Err(sys::EBADF),
            )],
            || {
                let error = apply_child_fd_actions(&[ChildFdAction::DupFd {
                    source_fd: -1,
                    target_fd: 56,
                }])
                .expect_err("child dup failure");
                assert!(!error.to_string().is_empty());
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_error() {
        run_trace(
            vec![
                t(
                    "close",
                    vec![ArgMatcher::Fd(56)],
                    TraceResult::Err(sys::EINVAL),
                ),
                t(
                    "close",
                    vec![ArgMatcher::Fd(57)],
                    TraceResult::Err(sys::EINVAL),
                ),
            ],
            || {
                let error = apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 56 }])
                    .expect_err("child close failure");
                assert!(!error.to_string().is_empty());

                let error = close_shell_fd(57).expect_err("close failure");
                assert!(!error.to_string().is_empty());
            },
        );
    }

    #[test]
    fn render_command_for_redirected() {
        assert_no_syscalls(|| {
            let command = Command::Redirected(
                Box::new(Command::Group(Program::default())),
                vec![Redirection {
                    fd: Some(1),
                    kind: RedirectionKind::Write,
                    target: Word { raw: "out".into(), line: 0 },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            );
            let rendered = render_command(&command);
            assert!(rendered.contains("{"));
            assert!(rendered.contains(">out"));

            replace_shell_fd(42, 42).expect("same-fd replacement");
        });
    }

    #[test]
    fn spawn_prepared_with_new_process_group() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/script.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/script.sh".into()), ArgMatcher::Int(1)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
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
                            TraceResult::Bytes(b"#!/bin/sh\n".to_vec()),
                        ),
                        t("close", vec![ArgMatcher::Fd(20)], TraceResult::Int(0)),
                        t(
                            "execvp",
                            vec![ArgMatcher::Str("/tmp/script.sh".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
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
                    exec_path: "/tmp/script.sh".into(),
                    argv: vec!["/tmp/script.sh".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: false,
                };
                let child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::NewGroup)
                    .expect("spawn newgroup");
                assert!(
                    child
                        .wait_with_output()
                        .expect("wait output")
                        .status
                        .success()
                );
            },
        );
    }

    #[test]
    fn spawn_prepared_with_stdout_redirect() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Int(1)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t("close", vec![ArgMatcher::Fd(1)], TraceResult::Int(0)),
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/bin/echo".into()),
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
                            vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
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
                    exec_path: "/bin/echo".into(),
                    argv: vec!["/bin/echo".into(), "hello".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: vec![ProcessRedirection {
                        fd: 1,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    }],
                    noclobber: false,
                    path_verified: false,
                };
                let child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("spawn with stdout redirect");
                assert!(child.wait().expect("wait").success());
            },
        );
    }

    #[test]
    fn spawn_prepared_with_join_process_group() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Int(1)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(42)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/bin/echo".into()),
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
                            vec![ArgMatcher::Str("/bin/echo".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
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
                    exec_path: "/bin/echo".into(),
                    argv: vec!["/bin/echo".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: false,
                };
                let child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::Join(42))
                    .expect("spawn join");
                assert!(child.wait().expect("wait").success());
            },
        );
    }

    #[test]
    fn handoff_foreground_switches_and_restores_pgrp() {
        run_trace(
            vec![
                // handoff_foreground(Some(77)): isatty(0), isatty(2), tcgetpgrp(0), tcsetpgrp(0, 77)
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(1)),
                t("isatty", vec![ArgMatcher::Fd(2)], TraceResult::Int(1)),
                t("tcgetpgrp", vec![ArgMatcher::Fd(0)], TraceResult::Pid(55)),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(77)],
                    TraceResult::Int(0),
                ),
                // handoff_foreground(Some(77)) again
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(1)),
                t("isatty", vec![ArgMatcher::Fd(2)], TraceResult::Int(1)),
                t("tcgetpgrp", vec![ArgMatcher::Fd(0)], TraceResult::Pid(55)),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(77)],
                    TraceResult::Int(0),
                ),
                // restore_foreground(Some(55)): tcsetpgrp(0, 55)
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(55)],
                    TraceResult::Int(0),
                ),
                // handoff_foreground(None): no syscalls
            ],
            || {
                assert_eq!(handoff_foreground(Some(77)), Some(55));
                assert_eq!(handoff_foreground(Some(77)), Some(55));
                restore_foreground(Some(55));
                assert_eq!(handoff_foreground(None), None);
            },
        );
    }

    #[test]
    fn handoff_foreground_returns_none_on_enotty() {
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(1)),
                t("isatty", vec![ArgMatcher::Fd(2)], TraceResult::Int(1)),
                t(
                    "tcgetpgrp",
                    vec![ArgMatcher::Fd(0)],
                    TraceResult::Err(sys::ENOTTY),
                ),
            ],
            || {
                assert_eq!(handoff_foreground(Some(77)), None);
            },
        );
    }

    #[test]
    fn apply_child_setup_for_process_groups() {
        run_trace(
            vec![
                // apply_child_setup([], None) → no OS calls
                // apply_child_setup([], NewGroup) → setpgid(0, 0)
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                // apply_child_setup([], Join(0)) → setpgid(0, 0)
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                apply_child_setup(&[], ProcessGroupPlan::None).expect("setup none");
                apply_child_setup(&[], ProcessGroupPlan::NewGroup).expect("setup newgroup");
                apply_child_setup(&[], ProcessGroupPlan::Join(0)).expect("setup join");
            },
        );
    }

    #[test]
    fn render_if_covers_elif_and_else_branches() {
        assert_no_syscalls(|| {
            let program = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false, line: 0 }]
                .into_boxed_slice(),
            };
            let if_command = IfCommand {
                condition: program.clone(),
                then_branch: program.clone(),
                elif_branches: vec![crate::syntax::ElifBranch {
                    condition: program.clone(),
                    body: program.clone(),
                }]
                .into_boxed_slice(),
                else_branch: Some(program),
            };
            let rendered = render_if(&if_command);
            assert!(rendered.contains("elif"));
            assert!(rendered.contains("else"));
            assert!(rendered.contains("fi"));
        });
    }

    #[test]
    fn render_loop_covers_until_keyword() {
        assert_no_syscalls(|| {
            let program = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false, line: 0 }]
                .into_boxed_slice(),
            };
            let rendered = render_loop(&LoopCommand {
                kind: LoopKind::Until,
                condition: program.clone(),
                body: program,
            });
            assert!(rendered.starts_with("until "));
        });
    }

    #[test]
    fn apply_shell_redirection_append_and_readwrite() {
        run_trace(
            vec![
                // Append: save fd 1, open file, dup2, close; then guard restores
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Fd(51),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/out".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(50),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(50), ArgMatcher::Fd(1)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(51), ArgMatcher::Fd(1)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(51)], TraceResult::Int(0)),
                // ReadWrite: save fd 0, open file, dup2, close; then guard restores
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(0),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Fd(53),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/rw".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(52),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(52), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(52)], TraceResult::Int(0)),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(53), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(53)], TraceResult::Int(0)),
            ],
            || {
                let redirections = vec![ExpandedRedirection {
                    fd: 1,
                    kind: RedirectionKind::Append,
                    target: "/tmp/out",
                    here_doc_body: None, line: 0 }];
                let _guard = apply_shell_redirections(&redirections, false).expect("append");
                drop(_guard);

                let redirections = vec![ExpandedRedirection {
                    fd: 0,
                    kind: RedirectionKind::ReadWrite,
                    target: "/tmp/rw",
                    here_doc_body: None, line: 0 }];
                let _guard = apply_shell_redirections(&redirections, false).expect("readwrite");
                drop(_guard);
            },
        );
    }

    #[test]
    fn prepare_redirections_covers_append_readwrite_and_dup() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/log".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(60),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/rw".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(61),
                ),
            ],
            || {
                let redirections = vec![
                    ExpandedRedirection {
                        fd: 1,
                        kind: RedirectionKind::Append,
                        target: "/tmp/log",
                        here_doc_body: None, line: 0 },
                    ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::ReadWrite,
                        target: "/tmp/rw",
                        here_doc_body: None, line: 0 },
                ];
                let prepared = prepare_redirections(&redirections, false).expect("prepare");
                assert_eq!(prepared.actions.len(), 2);
            },
        );
    }

    #[test]
    fn spawn_prepared_with_no_process_group() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/true".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/true".into()), ArgMatcher::Int(1)],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/bin/true".into()),
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
                            vec![ArgMatcher::Str("/bin/true".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/true".into(),
                    argv: vec!["true".into()].into_boxed_slice(),
                    redirections: vec![],

                    child_env: vec![].into_boxed_slice(),

                    path_verified: false,
                    noclobber: false,
                };
                let handle =
                    spawn_prepared(&mut shell, &prepared, ProcessGroupPlan::None).expect("spawn");
                let ws = sys::wait_pid(handle.pid, false)
                    .expect("wait")
                    .expect("status");
                assert_eq!(sys::decode_wait_status(ws.status), 0);
            },
        );
    }

    #[test]
    fn spawn_prepared_with_stdin_redirect() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/in".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(60),
                ),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(60), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "open",
                            vec![
                                ArgMatcher::Str("/bin/cat".into()),
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
                            vec![ArgMatcher::Str("/bin/cat".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(60)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/cat".into(),
                    argv: vec!["cat".into()].into_boxed_slice(),
                    redirections: vec![ProcessRedirection {
                        fd: 0,
                        kind: RedirectionKind::Read,
                        target: "/tmp/in".into(),
                        here_doc_body: None,
                    }],
                    child_env: vec![].into_boxed_slice(),

                    path_verified: true,
                    noclobber: false,
                };
                let _handle =
                    spawn_prepared(&mut shell, &prepared, ProcessGroupPlan::None).expect("spawn");
            },
        );
    }

    #[test]
    fn spawn_prepared_child_sets_env() {
        run_trace(
            vec![t_fork(
                TraceResult::Pid(1000),
                vec![
                    t(
                        "setenv",
                        vec![
                            ArgMatcher::Str("MEIKSH_TEST_COVERAGE".into()),
                            ArgMatcher::Str("1".into()),
                        ],
                        TraceResult::Int(0),
                    ),
                    t(
                        "open",
                        vec![
                            ArgMatcher::Str("/bin/true".into()),
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
                        vec![ArgMatcher::Str("/bin/true".into()), ArgMatcher::Any],
                        TraceResult::Int(0),
                    ),
                ],
            )],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/true".into(),
                    argv: vec!["true".into()].into_boxed_slice(),
                    redirections: vec![],

                    child_env: vec![("MEIKSH_TEST_COVERAGE".into(), "1".into())].into_boxed_slice(),
                    path_verified: true,
                    noclobber: false,
                };
                let _handle =
                    spawn_prepared(&mut shell, &prepared, ProcessGroupPlan::None).expect("spawn");
            },
        );
    }

    #[test]
    fn fork_and_execute_command_with_none_pgid() {
        run_trace(
            vec![
                t_fork(TraceResult::Pid(1000), vec![]),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let cmd = Command::Group(Program::default());
                let handle =
                    fork_and_execute_command(&mut shell, &cmd, None, false, ProcessGroupPlan::None)
                        .expect("fork");
                let ws = sys::wait_pid(handle.pid, false)
                    .expect("wait")
                    .expect("status");
                assert_eq!(sys::decode_wait_status(ws.status), 0);
            },
        );
    }

    #[test]
    fn apply_child_fd_close_ebadf_is_ignored() {
        run_trace(
            vec![t(
                "close",
                vec![ArgMatcher::Fd(99)],
                TraceResult::Err(sys::EBADF),
            )],
            || {
                let actions = vec![ChildFdAction::CloseFd { target_fd: 99 }];
                apply_child_fd_actions(&actions).expect("ebadf should be ignored");
            },
        );
    }

    #[test]
    fn save_restore_vars_restores_previous_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("FOO".into(), "original".into());
            shell.exported.insert("FOO".into());

            let assignments = vec![("FOO".into(), "temp".into()), ("BAR".into(), "new".into())];
            let saved = save_vars(&shell, &assignments);

            shell.set_var("FOO", "temp".into()).unwrap();
            shell.set_var("BAR", "new".into()).unwrap();
            assert_eq!(shell.get_var("FOO"), Some("temp"));
            assert_eq!(shell.get_var("BAR"), Some("new"));

            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var("FOO"), Some("original"));
            assert!(shell.exported.contains("FOO"));
            assert_eq!(shell.get_var("BAR"), None);
            assert!(!shell.exported.contains("BAR"));
        });
    }

    #[test]
    fn non_special_builtin_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("FOO".into(), "original".into());
            let program = parse_test("FOO=temp true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("original"));
        });
    }

    #[test]
    fn special_builtin_prefix_assignments_are_permanent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=permanent :").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("permanent"));
        });
    }

    #[test]
    fn function_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("FOO".into(), "original".into());
            let program = parse_test("myfn() { :; }; FOO=temp myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("original"));
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
            shell.env.insert("IFS".into(), " ".into());
            shell.env.insert("X".into(), "a b c".into());
            let program = parse_test("Y=$X").expect("parse");
            let _status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(shell.get_var("Y"), Some("a b c"));
        });
    }

    #[test]
    fn spawn_prepared_returns_eacces_for_non_executable_file() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/noexec.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/noexec.sh".into()), ArgMatcher::Int(1)],
                    TraceResult::Err(sys::EACCES),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"noexec.sh: Permission denied\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/tmp/noexec.sh".into(),
                    argv: vec!["noexec.sh".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: false,
                };
                let err = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None).unwrap_err();
                assert_eq!(err.exit_status(), 126);
            },
        );
    }

    #[test]
    fn child_exec_eacces_exits_126() {
        run_trace(
            vec![t_fork(
                TraceResult::Pid(1000),
                vec![
                    t(
                        "setpgid",
                        vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                        TraceResult::Int(0),
                    ),
                    t(
                        "open",
                        vec![
                            ArgMatcher::Str("/bin/noperm".into()),
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
                        vec![ArgMatcher::Str("/bin/noperm".into()), ArgMatcher::Any],
                        TraceResult::Err(sys::EACCES),
                    ),
                    t(
                        "write",
                        vec![
                            ArgMatcher::Fd(sys::STDERR_FILENO),
                            ArgMatcher::Bytes(b"noperm: Permission denied\n".to_vec()),
                        ],
                        TraceResult::Auto,
                    ),
                ],
            )],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/noperm".into(),
                    argv: vec!["noperm".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: true,
                };
                let _handle =
                    spawn_prepared(&shell, &prepared, ProcessGroupPlan::NewGroup).expect("spawn");
            },
        );
    }

    #[test]
    fn child_exec_enoent_exits_127() {
        run_trace(
            vec![t_fork(
                TraceResult::Pid(1000),
                vec![
                    t(
                        "setpgid",
                        vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                        TraceResult::Int(0),
                    ),
                    t(
                        "open",
                        vec![
                            ArgMatcher::Str("/bin/missing".into()),
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
                        vec![ArgMatcher::Str("/bin/missing".into()), ArgMatcher::Any],
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
            )],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/missing".into(),
                    argv: vec!["missing".into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: true,
                };
                let _handle =
                    spawn_prepared(&shell, &prepared, ProcessGroupPlan::NewGroup).expect("spawn");
            },
        );
    }

    #[test]
    fn spawn_and_or_with_and_or_list_forks_subshell() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(50), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "signal",
                            vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                        t(
                            "signal",
                            vec![ArgMatcher::Int(sys::SIGQUIT as i64), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(50)], TraceResult::Int(0)),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Int(1000)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let node = AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
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
                                words: vec![Word { raw: ":".into(), line: 0 }].into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                    )]
                    .into_boxed_slice(),
                };
                let spawned = spawn_and_or(&mut shell, &node, Some(50)).expect("spawn");
                assert_eq!(spawned.children.len(), 1);
                assert!(spawned.pgid.is_some());
            },
        );
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
    fn errexit_suppressed_in_elif_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program =
                parse_test("if false; then :; elif false; then :; fi; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_suppressed_in_while_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("while false; do :; done; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_suppressed_in_until_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("until true; do :; done; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_suppressed_in_non_final_and_or_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("false || true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_fires_on_final_and_or_command() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true && false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
            assert!(!shell.running);
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
    fn errexit_suppressed_in_negated_final_and_or_pipeline() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true && ! true; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_multi_step_and_or_suppresses_non_final() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("false || false || true").expect("parse");
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

                    argv: vec!["echo", "hello"],
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

                argv: vec!["echo"],
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
                    assignments: vec![("FOO", "bar")],
                    argv: vec!["cmd"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn wait_for_children_handles_stopped_child() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(9001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::StoppedSig(sys::SIGTSTP),
            )],
            || {
                let mut shell = test_shell();
                let spawned = SpawnedProcesses {
                    children: vec![sys::ChildHandle {
                        pid: 9001,
                        stdout_fd: None,
                    }],
                    pgid: None,
                };
                let status = wait_for_children(&mut shell, spawned, Some("sleep 100")).unwrap();
                assert_eq!(status, 128 + sys::SIGTSTP);
                assert_eq!(shell.jobs.len(), 1);
                assert!(matches!(
                    shell.jobs[0].state,
                    crate::shell::JobState::Stopped(s) if s == sys::SIGTSTP
                ));
            },
        );
    }

    #[test]
    fn wait_for_children_stopped_interactive_saves_termios() {
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(1)),
                t("isatty", vec![ArgMatcher::Fd(2)], TraceResult::Int(1)),
                t("tcgetpgrp", vec![ArgMatcher::Fd(0)], TraceResult::Pid(100)),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(9010)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(9010),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(100)],
                    TraceResult::Int(0),
                ),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] Stopped (SIGTSTP)\tvim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let spawned = SpawnedProcesses {
                    children: vec![sys::ChildHandle {
                        pid: 9010,
                        stdout_fd: None,
                    }],
                    pgid: Some(9010),
                };
                let status = wait_for_children(&mut shell, spawned, Some("vim")).unwrap();
                assert_eq!(status, 128 + sys::SIGTSTP);
                assert_eq!(shell.jobs.len(), 1);
                assert!(shell.jobs[0].saved_termios.is_some());
            },
        );
    }

    #[test]
    fn wait_for_external_child_handles_stopped() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(9020),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::StoppedSig(sys::SIGTSTP),
            )],
            || {
                let mut shell = test_shell();
                let handle = sys::ChildHandle {
                    pid: 9020,
                    stdout_fd: None,
                };
                let status =
                    wait_for_external_child(&mut shell, &handle, None, Some("cat")).unwrap();
                assert_eq!(status, 128 + sys::SIGTSTP);
                assert_eq!(shell.jobs.len(), 1);
                assert!(matches!(
                    shell.jobs[0].state,
                    crate::shell::JobState::Stopped(s) if s == sys::SIGTSTP
                ));
            },
        );
    }

    #[test]
    fn wait_for_external_child_stopped_interactive_saves_termios() {
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(1)),
                t("isatty", vec![ArgMatcher::Fd(2)], TraceResult::Int(1)),
                t("tcgetpgrp", vec![ArgMatcher::Fd(0)], TraceResult::Pid(200)),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(9030)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(9030),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Int(200)],
                    TraceResult::Int(0),
                ),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] Stopped (SIGTSTP)\tvim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let handle = sys::ChildHandle {
                    pid: 9030,
                    stdout_fd: None,
                };
                let status =
                    wait_for_external_child(&mut shell, &handle, Some(9030), Some("vim")).unwrap();
                assert_eq!(status, 128 + sys::SIGTSTP);
                assert!(shell.jobs[0].saved_termios.is_some());
            },
        );
    }

    #[test]
    fn execute_list_item_async_with_monitor_enabled() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(9100),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "stat",
                            vec![ArgMatcher::Str("/usr/bin/sleep".into()), ArgMatcher::Any],
                            TraceResult::StatFile(0o755),
                        ),
                        t_fork(
                            TraceResult::Pid(9101),
                            vec![
                                t(
                                    "setpgid",
                                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                                    TraceResult::Int(0),
                                ),
                                t(
                                    "open",
                                    vec![
                                        ArgMatcher::Str("/usr/bin/sleep".into()),
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
                                    vec![ArgMatcher::Str("/usr/bin/sleep".into()), ArgMatcher::Any],
                                    TraceResult::Int(0),
                                ),
                            ],
                        ),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(9101), ArgMatcher::Int(9101)],
                            TraceResult::Int(0),
                        ),
                        t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                        t(
                            "waitpid",
                            vec![
                                ArgMatcher::Int(9101),
                                ArgMatcher::Any,
                                ArgMatcher::Int(sys::WUNTRACED as i64),
                            ],
                            TraceResult::Status(0),
                        ),
                    ],
                ),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(9100), ArgMatcher::Int(9100)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let program = parse_test("sleep 10 &").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 0);
                assert_eq!(shell.jobs.len(), 1);
            },
        );
    }

    #[test]
    fn execute_list_item_async_interactive_prints_job_id() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(9200),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "stat",
                            vec![ArgMatcher::Str("/usr/bin/sleep".into()), ArgMatcher::Any],
                            TraceResult::StatFile(0o755),
                        ),
                        t_fork(
                            TraceResult::Pid(9201),
                            vec![
                                t(
                                    "setpgid",
                                    vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                                    TraceResult::Int(0),
                                ),
                                t(
                                    "open",
                                    vec![
                                        ArgMatcher::Str("/usr/bin/sleep".into()),
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
                                    vec![ArgMatcher::Str("/usr/bin/sleep".into()), ArgMatcher::Any],
                                    TraceResult::Int(0),
                                ),
                            ],
                        ),
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(9201), ArgMatcher::Int(9201)],
                            TraceResult::Int(0),
                        ),
                        t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                        t(
                            "waitpid",
                            vec![
                                ArgMatcher::Int(9201),
                                ArgMatcher::Any,
                                ArgMatcher::Int(sys::WUNTRACED as i64),
                            ],
                            TraceResult::Status(0),
                        ),
                    ],
                ),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(9200), ArgMatcher::Int(9200)],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] 9200\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                shell.interactive = true;
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let program = parse_test("sleep 10 &").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 0);
                assert_eq!(shell.jobs.len(), 1);
            },
        );
    }

    #[test]
    fn spawn_and_or_with_monitor_delegates_to_spawn_pipeline() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(9300),
                    vec![t(
                        "setpgid",
                        vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                        TraceResult::Int(0),
                    )],
                ),
                t(
                    "setpgid",
                    vec![ArgMatcher::Int(9300), ArgMatcher::Int(9300)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let node = AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                };
                let spawned = spawn_and_or(&mut shell, &node, None).expect("spawn");
                assert_eq!(spawned.children.len(), 1);
            },
        );
    }

    #[test]
    fn subshell_wait_retries_on_eintr() {
        let program = Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                },
                asynchronous: false, line: 0 }]
            .into_boxed_slice(),
        };
        run_trace(
            vec![
                t_fork(TraceResult::Pid(5000), vec![]),
                // first waitpid interrupted
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                // retry succeeds
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = execute_command(&mut shell, &Command::Subshell(program.clone()))
                    .expect("subshell");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn has_command_substitution_detects_backtick_in_words() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                words: vec![Word {
                    raw: "echo `date`".into(), line: 0 }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd));

            let cmd_no_sub = SimpleCommand {
                words: vec![Word {
                    raw: "plain".into(), line: 0 }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_no_sub));
        });
    }

    #[test]
    fn subshell_wait_retries_on_none() {
        let program = Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                },
                asynchronous: false, line: 0 }]
            .into_boxed_slice(),
        };
        run_trace(
            vec![
                t_fork(TraceResult::Pid(6000), vec![]),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(6000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Pid(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(6000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(3),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = execute_command(&mut shell, &Command::Subshell(program.clone()))
                    .expect("subshell");
                assert_eq!(status, 3);
            },
        );
    }

    #[test]
    fn subshell_wait_propagates_non_eintr_error() {
        let program = Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                },
                asynchronous: false, line: 0 }]
            .into_boxed_slice(),
        };
        run_trace(
            vec![
                t_fork(TraceResult::Pid(7000), vec![]),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Err(libc::EPERM),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: Operation not permitted\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let result = execute_command(&mut shell, &Command::Subshell(program.clone()));
                assert!(result.is_err());
            },
        );
    }

    fn time_snapshot_trace_pair(before_ns: i64, after_ns: i64) -> Vec<TraceEntry> {
        vec![
            t("monotonic_clock_ns", vec![], TraceResult::Int(before_ns)),
            t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
            t(
                "sysconf",
                vec![ArgMatcher::Int(libc::_SC_CLK_TCK as i64)],
                TraceResult::Int(100),
            ),
            t("monotonic_clock_ns", vec![], TraceResult::Int(after_ns)),
            t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
            t(
                "sysconf",
                vec![ArgMatcher::Int(libc::_SC_CLK_TCK as i64)],
                TraceResult::Int(100),
            ),
        ]
    }

    #[test]
    fn time_default_mode_reports_to_stderr() {
        let mut trace = time_snapshot_trace_pair(1_000_000_000, 2_000_000_000);
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));

        run_trace(trace, || {
            let mut shell = test_shell();
            let pipeline = Pipeline {
                negated: false,
                timed: TimedMode::Default,
                commands: vec![Command::Simple(SimpleCommand {
                    words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                    ..SimpleCommand::default()
                })]
                .into_boxed_slice(),
            };
            let status = execute_pipeline(&mut shell, &pipeline, false).expect("execute");
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn time_posix_mode_reports_to_stderr() {
        let mut trace = time_snapshot_trace_pair(1_000_000_000, 2_000_000_000);
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));
        trace.push(t(
            "write",
            vec![ArgMatcher::Int(2), ArgMatcher::Any],
            TraceResult::Auto,
        ));

        run_trace(trace, || {
            let mut shell = test_shell();
            let pipeline = Pipeline {
                negated: false,
                timed: TimedMode::Posix,
                commands: vec![Command::Simple(SimpleCommand {
                    words: vec![Word { raw: "true".into(), line: 0 }].into_boxed_slice(),
                    ..SimpleCommand::default()
                })]
                .into_boxed_slice(),
            };
            let status = execute_pipeline(&mut shell, &pipeline, false).expect("execute");
            assert_eq!(status, 0);
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
                let err = shell.execute_string("set -Z").expect_err("sbi error");
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
                let status = shell.execute_string("set -Z").expect("sbi interactive");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn compound_command_redirection_error_does_not_exit() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Int(100),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/nonexistent_redir_test".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Int(100), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Int(100)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 1: No such file or directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = shell
                    .execute_string("{ :; } < /nonexistent_redir_test")
                    .expect("redir");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn compound_command_error_respects_stderr_redirect() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(100),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(sys::STDERR_FILENO as i64),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: line 1: expected command list after 'if'\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Int(100), ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(sys::STDERR_FILENO as i64),
                ),
                t("close", vec![ArgMatcher::Int(100)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                let error = shell
                    .execute_string("{ eval 'if'; } 2>/dev/null")
                    .expect_err("syntax error");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn function_error_respects_stderr_redirect() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(100),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(sys::STDERR_FILENO as i64),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: line 1: expected command list after 'if'\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Int(100), ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(sys::STDERR_FILENO as i64),
                ),
                t("close", vec![ArgMatcher::Int(100)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.execute_string("f() { eval 'if'; }").expect("define");
                let error = shell
                    .execute_string("f 2>/dev/null")
                    .expect_err("syntax error");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn function_redirection_error_does_not_exit() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![ArgMatcher::Fd(0), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Int(100),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/nonexistent_func_redir".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Int(100), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Int(100)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 1: No such file or directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.execute_string("f() { :; }").expect("define fn");
                let status = shell
                    .execute_string("f < /nonexistent_func_redir")
                    .expect("redir fn");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn subshell_error_displays_message_in_child() {
        let program = parse_test("(readonly X; X=val)").expect("parse");
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(5000),
                    vec![t(
                        "write",
                        vec![
                            ArgMatcher::Fd(sys::STDERR_FILENO),
                            ArgMatcher::Bytes(b"meiksh: line 1: X: readonly variable\n".to_vec()),
                        ],
                        TraceResult::Auto,
                    )],
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(1),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = execute_program(&mut shell, &program).expect("subshell");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn empty_argv_redirect_error_returns_one() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Int(1030), ArgMatcher::Any],
                    TraceResult::Fd(92),
                ),
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(92), ArgMatcher::Fd(1)],
                    TraceResult::Int(1),
                ),
                t("close", vec![ArgMatcher::Fd(92)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 1: No such file or directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = shell.execute_string("> /bad").expect("redir error");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn non_special_builtin_prefix_readonly_restores_and_reports() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: RO: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.env.insert("RO".into(), "original".into());
                shell.readonly.insert("RO".into());
                let status = shell.execute_string("RO=val true").expect("builtin prefix");
                assert_ne!(status, 0);
                assert_eq!(shell.get_var("RO"), Some("original"));
            },
        );
    }

    #[test]
    fn readonly_prefix_on_external_command_is_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: RO: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.readonly.insert("RO".into());
                let err = shell
                    .execute_string("OK=1 RO=val ls")
                    .expect_err("readonly external");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn declaration_builtin_expands_assignments_and_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let status = shell
                .execute_string("command export FOO=bar BAZ")
                .expect("export");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("bar"));
        });
    }

    #[test]
    fn declaration_assignment_expansion_error_propagates() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: line 1: UNSET_NOUNSET_VAR: parameter not set\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let err = shell
                    .execute_string("export X=$UNSET_NOUNSET_VAR")
                    .expect_err("nounset");
                assert_ne!(err.exit_status(), 0);
            },
        );
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
                assert!(!file_needs_binary_rejection("/some/file"));
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
                assert!(!file_needs_binary_rejection("/some/file"));
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
                assert!(!file_needs_binary_rejection("/some/file"));
            },
        );
    }

    #[test]
    fn spawn_prepared_rejects_binary_file_in_child() {
        run_trace(
            vec![t_fork(
                TraceResult::Pid(1000),
                vec![
                    t(
                        "open",
                        vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                        TraceResult::Fd(20),
                    ),
                    t(
                        "read",
                        vec![ArgMatcher::Fd(20), ArgMatcher::Any],
                        TraceResult::Bytes(b"\x00binary".to_vec()),
                    ),
                    t("close", vec![ArgMatcher::Fd(20)], TraceResult::Int(0)),
                    t(
                        "write",
                        vec![
                            ArgMatcher::Fd(sys::STDERR_FILENO),
                            ArgMatcher::Bytes(b"test: cannot execute binary file\n".to_vec()),
                        ],
                        TraceResult::Auto,
                    ),
                ],
            )],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/test".into(),
                    argv: vec!["test".into()].into_boxed_slice(),
                    redirections: vec![],
                    child_env: vec![].into_boxed_slice(),
                    path_verified: true,
                    noclobber: false,
                };
                let _handle =
                    spawn_prepared(&mut shell, &prepared, ProcessGroupPlan::None).expect("spawn");
            },
        );
    }

    #[test]
    fn exec_no_cmd_error_path() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/--".into()), ArgMatcher::Int(0)],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/--".into()), ArgMatcher::Int(0)],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 1: exec: --: not found\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let err = shell.execute_string("exec -- --").expect_err("exec -- --");
                assert_eq!(err.exit_status(), 127);
            },
        );
    }

    #[test]
    fn wait_for_pipeline_pipefail_returns_rightmost_nonzero() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(100), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(3),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(101), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let spawned = SpawnedProcesses {
                    children: vec![
                        sys::ChildHandle {
                            pid: 100,
                            stdout_fd: None,
                        },
                        sys::ChildHandle {
                            pid: 101,
                            stdout_fd: None,
                        },
                    ],
                    pgid: None,
                };
                let status = wait_for_pipeline(&mut shell, spawned, Some("pipe"), true).unwrap();
                assert_eq!(status, 3);
            },
        );
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

    // ── Line-number regression tests ──────────────────────────────────

    #[test]
    fn lineno_parse_error_unterminated_single_quote() {
        run_trace(
            vec![t_stderr("meiksh: line 3: unterminated single quote")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\ntrue\necho '");
            },
        );
    }

    #[test]
    fn lineno_parse_error_unterminated_double_quote() {
        run_trace(
            vec![t_stderr("meiksh: line 2: unterminated double quote")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\necho \"hello");
            },
        );
    }

    #[test]
    fn lineno_parse_error_empty_if_condition() {
        run_trace(
            vec![t_stderr(
                "meiksh: line 3: expected command list after 'if'",
            )],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\nif\nthen true; fi");
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
                let _ = shell.execute_string("true\necho $MISSING");
            },
        );
    }

    #[test]
    fn lineno_expand_error_on_line_3() {
        run_trace(
            vec![t_stderr("meiksh: line 3: must be set")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\ntrue\n: ${NOVAR?must be set}");
            },
        );
    }

    #[test]
    fn lineno_runtime_break_outside_loop() {
        run_trace(
            vec![t_stderr("meiksh: line 2: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\nbreak");
            },
        );
    }

    #[test]
    fn lineno_runtime_readonly_assignment() {
        run_trace(
            vec![t_stderr("meiksh: line 2: X: readonly variable")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("readonly X=1\nX=2");
            },
        );
    }

    #[test]
    fn lineno_redirect_no_such_file() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Any, ArgMatcher::Int(10)],
                    TraceResult::Err(libc::EBADF),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/no/such/dir/file".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(libc::ENOENT),
                ),
                t("close", vec![ArgMatcher::Fd(1)], TraceResult::Int(0)),
                t_stderr("meiksh: line 2: No such file or directory"),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\ntrue > /no/such/dir/file");
            },
        );
    }

    #[test]
    fn lineno_error_inside_for_body() {
        run_trace(
            vec![t_stderr("meiksh: line 3: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string("for x in a; do\ntrue\necho $MISSING\ndone");
            },
        );
    }

    #[test]
    fn lineno_error_inside_if_body() {
        run_trace(
            vec![t_stderr("meiksh: line 3: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string("if true; then\ntrue\necho $MISSING\nfi");
            },
        );
    }

    #[test]
    fn lineno_error_inside_while_body() {
        run_trace(
            vec![t_stderr("meiksh: line 3: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string("while true; do\ntrue\necho $MISSING\nbreak\ndone");
            },
        );
    }

    #[test]
    fn lineno_error_inside_case_body() {
        run_trace(
            vec![t_stderr("meiksh: line 3: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string("case x in\nx)\necho $MISSING\n;;\nesac");
            },
        );
    }

    #[test]
    fn lineno_arithmetic_division_by_zero() {
        run_trace(
            vec![t_stderr("meiksh: line 2: division by zero")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\necho $((1/0))");
            },
        );
    }

    #[test]
    fn lineno_eval_restores_outer_line() {
        run_trace(
            vec![t_stderr(
                "meiksh: line 3: break: only meaningful in a loop",
            )],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\neval 'true'\nbreak");
            },
        );
    }

    #[test]
    fn lineno_interactive_suppresses_prefix() {
        run_trace(
            vec![t_stderr("meiksh: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let _ = shell.execute_string("true\nbreak");
            },
        );
    }

    #[test]
    fn lineno_single_line_reports_line_1() {
        run_trace(
            vec![t_stderr("meiksh: line 1: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("break");
            },
        );
    }

    #[test]
    fn lineno_readonly_in_export() {
        run_trace(
            vec![t_stderr("meiksh: line 3: RO: readonly variable")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("readonly RO=1\ntrue\nexport RO=2");
            },
        );
    }

    #[test]
    fn lineno_env_var_matches_shell_lineno() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string("true\ntrue\ntrue");
            assert_eq!(shell.get_var("LINENO"), Some("3"));
        });
    }

    #[test]
    fn lineno_env_var_updates_per_list_item() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string("A=$LINENO\ntrue\nB=$LINENO");
            assert_eq!(shell.get_var("A"), Some("1"));
            assert_eq!(shell.get_var("B"), Some("3"));
        });
    }

    #[test]
    fn lineno_continue_outside_loop() {
        run_trace(
            vec![t_stderr(
                "meiksh: line 4: continue: only meaningful in a loop",
            )],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\ntrue\ntrue\ncontinue");
            },
        );
    }

    #[test]
    fn lineno_sequential_scripts_track_independently() {
        run_trace(
            vec![
                t_stderr("meiksh: line 2: break: only meaningful in a loop"),
                t_stderr("meiksh: line 3: break: only meaningful in a loop"),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\nbreak");
                let _ = shell.execute_string("true\ntrue\nbreak");
            },
        );
    }

    #[test]
    fn lineno_error_after_multiline_compound() {
        run_trace(
            vec![t_stderr("meiksh: line 5: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(
                    "if true; then\ntrue\nfi\ntrue\nbreak",
                );
            },
        );
    }

    #[test]
    fn lineno_nested_compound_reports_inner_line() {
        run_trace(
            vec![t_stderr("meiksh: line 4: MISSING: parameter not set")],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string(
                    "if true; then\nif true; then\ntrue\necho $MISSING\nfi\nfi",
                );
            },
        );
    }

    #[test]
    fn lineno_parse_error_unterminated_command_substitution() {
        run_trace(
            vec![t_stderr(
                "meiksh: line 1: unterminated command substitution",
            )],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("echo $(");
            },
        );
    }

    #[test]
    fn lineno_parse_error_unterminated_parameter_expansion() {
        run_trace(
            vec![t_stderr(
                "meiksh: line 2: unterminated parameter expansion",
            )],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\necho ${x");
            },
        );
    }

    #[test]
    fn lineno_arithmetic_invalid_token() {
        run_trace(
            vec![t_stderr("meiksh: line 2: expected arithmetic operand")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\n: $((@))");
            },
        );
    }

    #[test]
    fn lineno_multiline_arithmetic_error_on_later_line() {
        run_trace(
            vec![t_stderr("meiksh: line 3: division by zero")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true\n: $(( 1 +\n1 / 0 ))");
            },
        );
    }

    #[test]
    fn lineno_error_after_semicolon_list() {
        run_trace(
            vec![t_stderr("meiksh: line 2: break: only meaningful in a loop")],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string("true; true\nbreak");
            },
        );
    }
}
