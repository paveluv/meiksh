use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builtin;
use crate::expand;
use crate::shell::{FlowSignal, PendingControl, Shell, ShellError};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand,
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
        if !shell.running || shell.has_pending_control() {
            break;
        }
    }
    Ok(status)
}

fn execute_list_item(shell: &mut Shell, item: &ListItem) -> Result<i32, ShellError> {
    if item.asynchronous {
        let devnull = sys::open_file("/dev/null", sys::O_RDONLY, 0)?;
        let spawned = spawn_and_or(shell, &item.and_or, Some(devnull))?;
        let last_pid = spawned.children.last().map(|c| c.pid).unwrap_or(0);
        let description = render_and_or(&item.and_or);
        let id = shell.register_background_job(description, spawned.pgid, spawned.children);
        println!("[{id}] {last_pid}");
        Ok(0)
    } else {
        execute_and_or(shell, &item.and_or)
    }
}

fn execute_and_or(shell: &mut Shell, node: &AndOr) -> Result<i32, ShellError> {
    let mut status = execute_pipeline(shell, &node.first, false)?;
    for (op, pipeline) in &node.rest {
        match op {
            LogicalOp::And if status == 0 => status = execute_pipeline(shell, pipeline, false)?,
            LogicalOp::Or if status != 0 => status = execute_pipeline(shell, pipeline, false)?,
            _ => {}
        }
    }
    Ok(status)
}

fn spawn_and_or(
    shell: &mut Shell,
    node: &AndOr,
    stdin_override: Option<i32>,
) -> Result<SpawnedProcesses, ShellError> {
    if node.rest.is_empty() {
        return spawn_pipeline(shell, &node.first, stdin_override);
    }
    let pid = sys::fork_process()?;
    if pid == 0 {
        if let Some(fd) = stdin_override {
            let _ = sys::duplicate_fd(fd, sys::STDIN_FILENO);
            let _ = sys::close_fd(fd);
        }
        let _ = sys::set_process_group(0, 0);
        let mut child_shell = shell.clone();
        let status = execute_and_or(&mut child_shell, node).unwrap_or(1);
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
    if pipeline.commands.len() == 1 {
        if !asynchronous {
            let status = execute_command(shell, &pipeline.commands[0])?;
            return Ok(if pipeline.negated {
                if status == 0 { 1 } else { 0 }
            } else {
                status
            });
        }
    }

    let spawned = spawn_pipeline(shell, pipeline, None)?;
    if asynchronous {
        return Ok(0);
    }

    let status = wait_for_children(shell, spawned)?;

    if pipeline.negated {
        Ok(if status == 0 { 1 } else { 0 })
    } else {
        Ok(status)
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
        let (r, w) = sys::create_pipe()?;
        Some((r, w))
    } else {
        Option::None
    };

    let pid = sys::fork_process()?;
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

        let handle = match command {
            Command::Simple(simple) => {
                let prepared = build_process(shell, simple)?;
                spawn_prepared(shell, &prepared, previous_stdout_fd.take(), !is_last, plan)?
            }
            _ => {
                fork_and_execute_command(shell, command, previous_stdout_fd.take(), !is_last, plan)?
            }
        };

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

fn wait_for_children(shell: &mut Shell, spawned: SpawnedProcesses) -> Result<i32, ShellError> {
    let saved_foreground = handoff_foreground(spawned.pgid);
    let mut status = 0;
    for handle in &spawned.children {
        status = shell.wait_for_child_pid(handle.pid, false)?;
    }
    restore_foreground(saved_foreground);
    Ok(status)
}

fn wait_for_external_child(
    shell: &mut Shell,
    handle: &sys::ChildHandle,
    pgid: Option<sys::Pid>,
) -> Result<i32, ShellError> {
    let saved_foreground = handoff_foreground(pgid);
    let status = shell.wait_for_child_pid(handle.pid, false)?;
    restore_foreground(saved_foreground);
    Ok(status)
}

fn execute_command(shell: &mut Shell, command: &Command) -> Result<i32, ShellError> {
    match command {
        Command::Simple(simple) => execute_simple(shell, simple),
        Command::Subshell(program) => {
            let pid = sys::fork_process()?;
            if pid == 0 {
                let mut child_shell = shell.clone();
                let status = execute_nested_program(&mut child_shell, program).unwrap_or(1);
                sys::exit_process(status as sys::RawFd);
            }
            let ws = sys::wait_pid(pid, false)?.expect("child status");
            Ok(sys::decode_wait_status(ws.status))
        }
        Command::Group(program) => execute_nested_program(shell, program),
        Command::FunctionDef(function) => {
            shell
                .functions
                .insert(function.name.clone(), (*function.body).clone());
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
    let expanded = expand_redirections(shell, redirections)?;
    with_shell_redirections(&expanded, shell.options.noclobber, || {
        execute_command(shell, command)
    })
}

fn execute_if(shell: &mut Shell, if_command: &IfCommand) -> Result<i32, ShellError> {
    if execute_nested_program(shell, &if_command.condition)? == 0 {
        return execute_nested_program(shell, &if_command.then_branch);
    }

    for branch in &if_command.elif_branches {
        if execute_nested_program(shell, &branch.condition)? == 0 {
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
            let condition_status = execute_nested_program(shell, &loop_command.condition)?;
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
    let values = if let Some(items) = &for_command.items {
        let mut values = Vec::new();
        for item in items {
            values.extend(expand::expand_word(shell, item)?);
        }
        values
    } else {
        shell.positional.clone()
    };

    shell.loop_depth += 1;
    let result = (|| {
        let mut last_status = 0;
        for value in values {
            shell.set_var(&for_command.name, value)?;
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
    let word = expand::expand_word_text(shell, &case_command.word)?;
    for arm in &case_command.arms {
        for pattern in &arm.patterns {
            let pattern = expand::expand_word_text(shell, pattern)?;
            if case_pattern_matches(&word, &pattern) {
                return execute_nested_program(shell, &arm.body);
            }
        }
    }
    Ok(0)
}

struct SavedVar {
    name: String,
    value: Option<String>,
    was_exported: bool,
}

fn save_vars(shell: &Shell, assignments: &[(String, String)]) -> Vec<SavedVar> {
    assignments
        .iter()
        .map(|(name, _)| SavedVar {
            name: name.clone(),
            value: shell.get_var(name),
            was_exported: shell.exported.contains(name),
        })
        .collect()
}

fn restore_vars(shell: &mut Shell, saved: Vec<SavedVar>) {
    for entry in saved {
        match entry.value {
            Some(v) => {
                shell.env.insert(entry.name.clone(), v);
            }
            None => {
                shell.env.remove(&entry.name);
            }
        }
        if entry.was_exported {
            shell.exported.insert(entry.name);
        } else {
            shell.exported.remove(&entry.name);
        }
    }
}

fn run_builtin_flow(
    shell: &mut Shell,
    argv: &[String],
    assignments: &[(String, String)],
) -> Result<i32, ShellError> {
    match shell.run_builtin(argv, assignments)? {
        FlowSignal::Continue(status) => Ok(status),
        FlowSignal::Exit(status) => {
            shell.running = false;
            Ok(status)
        }
    }
}

fn execute_simple(shell: &mut Shell, simple: &SimpleCommand) -> Result<i32, ShellError> {
    let expanded = expand_simple(shell, simple)?;

    if expanded.argv.is_empty() {
        return with_shell_redirections(&expanded.redirections, shell.options.noclobber, || {
            for (name, value) in expanded.assignments {
                shell.set_var(&name, value)?;
            }
            Ok(0)
        });
    }

    if builtin::is_builtin(&expanded.argv[0]) {
        let is_special_builtin = builtin::is_special_builtin(&expanded.argv[0]);
        let result = if is_special_builtin {
            with_shell_redirections(&expanded.redirections, shell.options.noclobber, || {
                run_builtin_flow(shell, &expanded.argv, &expanded.assignments)
            })
        } else {
            let saved_vars = save_vars(shell, &expanded.assignments);
            for (name, value) in &expanded.assignments {
                shell.set_var(name, value.clone())?;
            }
            let result =
                with_shell_redirections(&expanded.redirections, shell.options.noclobber, || {
                    run_builtin_flow(shell, &expanded.argv, &[])
                });
            restore_vars(shell, saved_vars);
            result
        };
        match result {
            Ok(status) => Ok(status),
            Err(error) if !is_special_builtin => {
                eprintln!("{}", error.display_message());
                Ok(error.exit_status())
            }
            Err(error) => Err(error),
        }
    } else if let Some(function) = shell.functions.get(&expanded.argv[0]).cloned() {
        with_shell_redirections(&expanded.redirections, shell.options.noclobber, || {
            let saved_vars = save_vars(shell, &expanded.assignments);
            for (name, value) in &expanded.assignments {
                shell.set_var(name, value.clone())?;
            }
            let saved = std::mem::replace(&mut shell.positional, expanded.argv[1..].to_vec());
            shell.function_depth += 1;
            let status = execute_command(shell, &function);
            shell.function_depth = shell.function_depth.saturating_sub(1);
            shell.positional = saved;
            restore_vars(shell, saved_vars);
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
        })
    } else {
        let prepared = build_process_from_expanded(shell, &expanded)?;
        let handle = spawn_prepared(shell, &prepared, None, false, ProcessGroupPlan::NewGroup)?;
        let pgid = handle.pid;
        let _ = sys::set_process_group(pgid, pgid);
        let status = wait_for_external_child(shell, &handle, Some(pgid))?;
        Ok(status)
    }
}

#[derive(Debug)]
struct ExpandedSimpleCommand {
    assignments: Vec<(String, String)>,
    argv: Vec<String>,
    redirections: Vec<ExpandedRedirection>,
}

#[derive(Clone, Debug)]
struct ExpandedRedirection {
    fd: i32,
    kind: RedirectionKind,
    target: String,
    here_doc_body: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedProcess {
    exec_path: String,
    argv: Vec<String>,
    child_env: Vec<(String, String)>,
    redirections: Vec<ExpandedRedirection>,
    noclobber: bool,
    path_verified: bool,
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
    stdin_redirected: bool,
    stdout_redirected: bool,
    actions: Vec<ChildFdAction>,
}

#[derive(Debug)]
struct ShellRedirectionGuard {
    saved: Vec<(i32, Option<i32>)>,
}

fn expand_simple(
    shell: &mut Shell,
    simple: &SimpleCommand,
) -> Result<ExpandedSimpleCommand, ShellError> {
    let mut assignments = Vec::new();
    for assignment in &simple.assignments {
        let value = expand::expand_word_text(shell, &assignment.value)?;
        assignments.push((assignment.name.clone(), value));
    }

    let argv = expand::expand_words(shell, &simple.words)?;
    let mut redirections = Vec::new();
    for redirection in &simple.redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection.here_doc.as_ref().ok_or_else(|| ShellError {
                message: "missing here-document body".to_string(),
            })?;
            let body = if here_doc.expand {
                expand::expand_here_document(shell, &here_doc.body)?
            } else {
                here_doc.body.clone()
            };
            (here_doc.delimiter.clone(), Some(body))
        } else {
            let fields = expand::expand_word(shell, &redirection.target)?;
            (fields.first().cloned().unwrap_or_default(), None)
        };
        redirections.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
        });
    }

    Ok(ExpandedSimpleCommand {
        assignments,
        argv,
        redirections,
    })
}

fn expand_redirections(
    shell: &mut Shell,
    redirections: &[crate::syntax::Redirection],
) -> Result<Vec<ExpandedRedirection>, ShellError> {
    let mut expanded = Vec::new();
    for redirection in redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection.here_doc.as_ref().ok_or_else(|| ShellError {
                message: "missing here-document body".to_string(),
            })?;
            let body = if here_doc.expand {
                expand::expand_here_document(shell, &here_doc.body)?
            } else {
                here_doc.body.clone()
            };
            (here_doc.delimiter.clone(), Some(body))
        } else {
            let fields = expand::expand_word(shell, &redirection.target)?;
            (fields.first().cloned().unwrap_or_default(), None)
        };
        expanded.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
        });
    }
    Ok(expanded)
}

fn build_process(shell: &mut Shell, simple: &SimpleCommand) -> Result<PreparedProcess, ShellError> {
    let expanded = expand_simple(shell, simple)?;
    build_process_from_expanded(shell, &expanded)
}

fn build_process_from_expanded(
    shell: &Shell,
    expanded: &ExpandedSimpleCommand,
) -> Result<PreparedProcess, ShellError> {
    let program = expanded.argv.first().ok_or_else(|| ShellError {
        message: "empty command".to_string(),
    })?;
    let resolved = resolve_command_path(shell, program);
    let path_verified = resolved.is_some();
    let exec_path = resolved
        .unwrap_or_else(|| PathBuf::from(program))
        .display()
        .to_string();
    let mut child_env: Vec<(String, String)> = shell.env_for_child().into_iter().collect();
    for (name, value) in &expanded.assignments {
        child_env.push((name.clone(), value.clone()));
    }
    Ok(PreparedProcess {
        exec_path,
        argv: expanded.argv.clone(),
        child_env,
        redirections: expanded.redirections.clone(),
        noclobber: shell.options.noclobber,
        path_verified,
    })
}

fn spawn_prepared(
    shell: &Shell,
    prepared: &PreparedProcess,
    stdin_fd: Option<i32>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    if !prepared.path_verified && !prepared.exec_path.is_empty() && prepared.exec_path.contains('/')
    {
        if sys::access_path(&prepared.exec_path, sys::F_OK).is_err() {
            return Err(sys::SysError::Errno(sys::ENOENT).into());
        }
        if sys::access_path(&prepared.exec_path, sys::X_OK).is_err() {
            return Err(sys::SysError::Errno(sys::EACCES).into());
        }
    }

    let prepared_redirections = prepare_redirections(&prepared.redirections, prepared.noclobber)?;

    let effective_stdin = if !prepared_redirections.stdin_redirected {
        stdin_fd
    } else {
        if let Some(fd) = stdin_fd {
            let _ = sys::close_fd(fd);
        }
        None
    };

    let effective_pipe_stdout = pipe_stdout && !prepared_redirections.stdout_redirected;

    let stdout_pipe = if effective_pipe_stdout {
        let (r, w) = sys::create_pipe()?;
        Some((r, w))
    } else {
        None
    };

    let pid = sys::fork_process()?;
    if pid == 0 {
        // Child: set up process group, stdin, stdout, redirections
        match process_group {
            ProcessGroupPlan::NewGroup => {
                let _ = sys::set_process_group(0, 0);
            }
            ProcessGroupPlan::Join(pgid) => {
                let _ = sys::set_process_group(0, pgid);
            }
            ProcessGroupPlan::None => {}
        }
        if let Some(fd) = effective_stdin {
            let _ = sys::duplicate_fd(fd, sys::STDIN_FILENO);
            let _ = sys::close_fd(fd);
        }
        if let Some((r, w)) = stdout_pipe {
            let _ = sys::close_fd(r);
            let _ = sys::duplicate_fd(w, sys::STDOUT_FILENO);
            let _ = sys::close_fd(w);
        }
        let _ = apply_child_fd_actions(&prepared_redirections.actions);

        for (key, value) in &prepared.child_env {
            let _ = sys::env_set_var(key, value);
        }

        match sys::exec_replace(&prepared.exec_path, &prepared.argv) {
            Err(err) if err.is_enoexec() => {
                let mut child_shell = shell.clone();
                child_shell.shell_name = prepared.argv[0].clone();
                child_shell.positional = prepared.argv[1..].to_vec();
                let status = child_shell
                    .source_path(std::path::Path::new(&prepared.exec_path))
                    .unwrap_or(126);
                sys::exit_process(status as sys::RawFd);
            }
            Err(err) if err.is_enoent() => sys::exit_process(127),
            Err(_) => sys::exit_process(126),
            Ok(()) => sys::exit_process(0),
        }
    }

    // Parent: clean up fds
    if let Some(fd) = effective_stdin {
        let _ = sys::close_fd(fd);
    }
    let stdout_read = stdout_pipe.map(|(r, w)| {
        let _ = sys::close_fd(w);
        r
    });
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
        stdout_fd: stdout_read,
    })
}

fn prepare_redirections(
    redirections: &[ExpandedRedirection],
    noclobber: bool,
) -> Result<PreparedRedirections, ShellError> {
    let mut prepared = PreparedRedirections::default();
    for redirection in redirections {
        if redirection.fd == 0 {
            prepared.stdin_redirected = true;
        }
        if redirection.fd == 1 {
            prepared.stdout_redirected = true;
        }
        match redirection.kind {
            RedirectionKind::Read => {
                let fd = sys::open_file(&redirection.target, sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::Write | RedirectionKind::ClobberWrite => {
                let flags = if noclobber && redirection.kind == RedirectionKind::Write {
                    sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC
                } else {
                    sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC
                };
                let fd = sys::open_file(&redirection.target, flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::Append => {
                let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
                let fd = sys::open_file(&redirection.target, flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::HereDoc => {
                let (read_fd, write_fd) = sys::create_pipe()?;
                sys::write_all_fd(
                    write_fd,
                    redirection
                        .here_doc_body
                        .as_deref()
                        .unwrap_or("")
                        .as_bytes(),
                )?;
                sys::close_fd(write_fd)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd: read_fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::ReadWrite => {
                let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
                let fd = sys::open_file(&redirection.target, flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::DupInput | RedirectionKind::DupOutput => {
                if redirection.target == "-" {
                    prepared.actions.push(ChildFdAction::CloseFd {
                        target_fd: redirection.fd,
                    });
                } else {
                    let source_fd = redirection.target.parse::<i32>().map_err(|_| ShellError {
                        message: "redirection target must be a file descriptor or '-'".to_string(),
                    })?;
                    prepared.actions.push(ChildFdAction::DupFd {
                        source_fd,
                        target_fd: redirection.fd,
                    });
                }
            }
        }
    }
    Ok(prepared)
}

fn resolve_command_path(shell: &Shell, program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }

    let path = shell
        .get_var("PATH")
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

fn with_shell_redirections<T>(
    redirections: &[ExpandedRedirection],
    noclobber: bool,
    action: impl FnOnce() -> Result<T, ShellError>,
) -> Result<T, ShellError> {
    let _guard = apply_shell_redirections(redirections, noclobber)?;
    action()
}

fn apply_shell_redirections(
    redirections: &[ExpandedRedirection],
    noclobber: bool,
) -> Result<ShellRedirectionGuard, ShellError> {
    let mut guard = ShellRedirectionGuard { saved: Vec::new() };
    let mut saved = HashMap::new();

    for redirection in redirections {
        if let std::collections::hash_map::Entry::Vacant(entry) = saved.entry(redirection.fd) {
            let original = match sys::duplicate_fd_to_new(redirection.fd) {
                Ok(fd) => Some(fd),
                Err(error) if error.is_ebadf() => None,
                Err(error) => return Err(error.into()),
            };
            entry.insert(original);
            guard.saved.push((redirection.fd, original));
        }
        apply_shell_redirection(redirection, noclobber)?;
    }

    Ok(guard)
}

fn apply_shell_redirection(
    redirection: &ExpandedRedirection,
    noclobber: bool,
) -> Result<(), ShellError> {
    match redirection.kind {
        RedirectionKind::Read => {
            let fd = sys::open_file(&redirection.target, sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
            replace_shell_fd(fd, redirection.fd)?;
        }
        RedirectionKind::Write | RedirectionKind::ClobberWrite => {
            let flags = if noclobber && redirection.kind == RedirectionKind::Write {
                sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC
            } else {
                sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC
            };
            let fd = sys::open_file(&redirection.target, flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd)?;
        }
        RedirectionKind::Append => {
            let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
            let fd = sys::open_file(&redirection.target, flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd)?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::create_pipe()?;
            sys::write_all_fd(
                write_fd,
                redirection
                    .here_doc_body
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes(),
            )?;
            sys::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd)?;
        }
        RedirectionKind::ReadWrite => {
            let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
            let fd = sys::open_file(&redirection.target, flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd)?;
        }
        RedirectionKind::DupInput | RedirectionKind::DupOutput => {
            if redirection.target == "-" {
                close_shell_fd(redirection.fd)?;
            } else {
                let source_fd = redirection.target.parse::<i32>().map_err(|_| ShellError {
                    message: "redirection target must be a file descriptor or '-'".to_string(),
                })?;
                sys::duplicate_fd(source_fd, redirection.fd)?;
            }
        }
    }
    Ok(())
}

fn replace_shell_fd(fd: i32, target_fd: i32) -> Result<(), ShellError> {
    if fd == target_fd {
        return Ok(());
    }
    sys::duplicate_fd(fd, target_fd)?;
    sys::close_fd(fd)?;
    Ok(())
}

fn close_shell_fd(target_fd: i32) -> Result<(), ShellError> {
    if let Err(error) = sys::close_fd(target_fd) {
        if !error.is_ebadf() {
            return Err(error.into());
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

fn case_pattern_matches(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    match_pattern(&text, 0, &pattern, 0)
}

fn match_pattern(text: &[char], ti: usize, pattern: &[char], pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }

    match pattern[pi] {
        '*' => (ti..=text.len()).any(|next_ti| match_pattern(text, next_ti, pattern, pi + 1)),
        '?' => ti < text.len() && match_pattern(text, ti + 1, pattern, pi + 1),
        '[' => match match_bracket(text.get(ti).copied(), pattern, pi) {
            Some((matched, next_pi)) => matched && match_pattern(text, ti + 1, pattern, next_pi),
            None => {
                ti < text.len() && text[ti] == '[' && match_pattern(text, ti + 1, pattern, pi + 1)
            }
        },
        '\\' if pi + 1 < pattern.len() => {
            ti < text.len()
                && text[ti] == pattern[pi + 1]
                && match_pattern(text, ti + 1, pattern, pi + 2)
        }
        ch => ti < text.len() && text[ti] == ch && match_pattern(text, ti + 1, pattern, pi + 1),
    }
}

fn match_bracket(current: Option<char>, pattern: &[char], start: usize) -> Option<(bool, usize)> {
    let current = current?;
    let mut index = start + 1;
    if index >= pattern.len() {
        return None;
    }

    let mut negate = false;
    if matches!(pattern.get(index), Some('!') | Some('^')) {
        negate = true;
        index += 1;
    }

    let mut matched = false;
    let mut saw_closer = false;
    while index < pattern.len() {
        if pattern[index] == ']' {
            saw_closer = true;
            index += 1;
            break;
        }

        let first = if pattern[index] == '\\' && index + 1 < pattern.len() {
            index += 1;
            pattern[index]
        } else {
            pattern[index]
        };

        if index + 2 < pattern.len() && pattern[index + 1] == '-' && pattern[index + 2] != ']' {
            let last = pattern[index + 2];
            matched |= first <= current && current <= last;
            index += 3;
        } else {
            matched |= current == first;
            index += 1;
        }
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn render_program(program: &Program) -> String {
    let mut text = String::new();
    for (index, item) in program.items.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str(&render_list_item(item));
    }
    text
}

fn render_list_item(item: &ListItem) -> String {
    let mut text = render_and_or(&item.and_or);
    if item.asynchronous {
        text.push_str(" &");
    }
    text
}

fn render_and_or(and_or: &AndOr) -> String {
    let mut text = render_pipeline(&and_or.first);
    for (op, pipeline) in &and_or.rest {
        match op {
            LogicalOp::And => text.push_str(" && "),
            LogicalOp::Or => text.push_str(" || "),
        }
        text.push_str(&render_pipeline(pipeline));
    }
    text
}

fn execute_nested_program(shell: &mut Shell, program: &Program) -> Result<i32, ShellError> {
    shell.execute_string(&render_program(program))
}

fn render_command(command: &Command) -> String {
    match command {
        Command::Simple(simple) => render_simple(simple),
        Command::Subshell(program) => format!("({})", render_program(program)),
        Command::Group(program) => format!("{{ {}; }}", render_program(program)),
        Command::FunctionDef(function) => render_function(function),
        Command::If(if_command) => render_if(if_command),
        Command::Loop(loop_command) => render_loop(loop_command),
        Command::For(for_command) => render_for(for_command),
        Command::Case(case_command) => render_case(case_command),
        Command::Redirected(command, redirections) => {
            render_redirected_command(command, redirections)
        }
    }
}

fn render_pipeline(pipeline: &Pipeline) -> String {
    let mut parts = Vec::new();
    for command in &pipeline.commands {
        parts.push(render_command(command));
    }
    let text = parts.join(" | ");
    if pipeline.negated {
        format!("! {text}")
    } else {
        text
    }
}

fn render_function(function: &FunctionDef) -> String {
    format!(
        "{}() {}",
        function.name,
        render_pipeline(&Pipeline {
            negated: false,
            commands: vec![(*function.body).clone()],
        })
    )
}

fn render_if(if_command: &IfCommand) -> String {
    let mut text = format!(
        "if {}\nthen\n{}",
        render_program(&if_command.condition),
        render_program(&if_command.then_branch)
    );
    for branch in &if_command.elif_branches {
        text.push_str(&format!(
            "\nelif {}\nthen\n{}",
            render_program(&branch.condition),
            render_program(&branch.body)
        ));
    }
    if let Some(else_branch) = &if_command.else_branch {
        text.push_str(&format!("\nelse\n{}", render_program(else_branch)));
    }
    text.push_str("\nfi");
    text
}

fn render_loop(loop_command: &LoopCommand) -> String {
    let keyword = match loop_command.kind {
        LoopKind::While => "while",
        LoopKind::Until => "until",
    };
    format!(
        "{keyword} {}\ndo\n{}\ndone",
        render_program(&loop_command.condition),
        render_program(&loop_command.body)
    )
}

fn render_for(for_command: &ForCommand) -> String {
    let mut text = format!("for {}", for_command.name);
    if let Some(items) = &for_command.items {
        text.push_str(" in");
        for item in items {
            text.push(' ');
            text.push_str(&item.raw);
        }
    }
    text.push_str(&format!(
        "\ndo\n{}\ndone",
        render_program(&for_command.body)
    ));
    text
}

fn render_case(case_command: &CaseCommand) -> String {
    let mut text = format!("case {} in", case_command.word.raw);
    for arm in &case_command.arms {
        text.push('\n');
        let patterns = arm
            .patterns
            .iter()
            .map(|pattern| pattern.raw.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        text.push_str(&patterns);
        text.push_str(")\n");
        text.push_str(&render_program(&arm.body));
        text.push_str("\n;;");
    }
    text.push_str("\nesac");
    text
}

fn render_simple(simple: &SimpleCommand) -> String {
    render_command_line_with_redirections(
        {
            let mut parts = Vec::new();
            for assignment in &simple.assignments {
                parts.push(format!("{}={}", assignment.name, assignment.value.raw));
            }
            for word in &simple.words {
                parts.push(word.raw.clone());
            }
            parts.join(" ")
        },
        &simple.redirections,
    )
}

fn render_redirections_with_bodies(
    redirections: &[crate::syntax::Redirection],
) -> (String, Vec<String>) {
    let mut parts = Vec::new();
    let mut heredocs = Vec::new();
    for redirection in redirections {
        parts.push(render_redirection_operator(redirection));
        if let Some(here_doc) = &redirection.here_doc {
            heredocs.push(render_here_doc_body(here_doc));
        }
    }
    (parts.join(" "), heredocs)
}

fn render_redirection_operator(redirection: &crate::syntax::Redirection) -> String {
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
    let fd = redirection.fd.map(|fd| fd.to_string()).unwrap_or_default();
    format!("{fd}{op}{}", redirection.target.raw)
}

fn render_here_doc_body(here_doc: &HereDoc) -> String {
    if here_doc.body.ends_with('\n') {
        format!("{}{}", here_doc.body, here_doc.delimiter)
    } else {
        format!("{}\n{}", here_doc.body, here_doc.delimiter)
    }
}

fn render_command_line_with_redirections(
    base: String,
    redirections: &[crate::syntax::Redirection],
) -> String {
    let (redir_text, heredocs) = render_redirections_with_bodies(redirections);
    let mut line = base;
    if !redir_text.is_empty() {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&redir_text);
    }
    if heredocs.is_empty() {
        line
    } else {
        format!("{line}\n{}", heredocs.join("\n"))
    }
}

fn render_redirected_command(
    command: &Command,
    redirections: &[crate::syntax::Redirection],
) -> String {
    render_command_line_with_redirections(render_command(command), redirections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{
        ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t, t_fork,
    };
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: "meiksh".to_string(),
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
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
            interactive: false,
        }
    }

    #[test]
    fn execute_and_or_skips_rhs_when_guard_fails() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = crate::syntax::parse("true || false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);

            let program = crate::syntax::parse("false && true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
        });
    }

    #[test]
    fn execute_pipeline_async_single_command() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str("/usr/bin/true".into()), ArgMatcher::Any],
                    TraceResult::StatFile(0o755),
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
                            "execvp",
                            vec![ArgMatcher::Str("/usr/bin/true".into()), ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                    ],
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
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word { raw: "true".into() }],
                        ..SimpleCommand::default()
                    })],
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
                t(
                    "stat",
                    vec![ArgMatcher::Str("/usr/bin/printf".into()), ArgMatcher::Any],
                    TraceResult::StatFile(0o755),
                ),
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                        t(
                            "execvp",
                            vec![ArgMatcher::Str("/usr/bin/printf".into()), ArgMatcher::Any],
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
                t(
                    "stat",
                    vec![ArgMatcher::Str("/usr/bin/wc".into()), ArgMatcher::Any],
                    TraceResult::StatFile(0o755),
                ),
                t_fork(
                    TraceResult::Pid(1001),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(1000)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(200), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
                        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
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
                t("isatty", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
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
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let pipeline = Pipeline {
                    negated: true,
                    commands: vec![
                        Command::Simple(SimpleCommand {
                            words: vec![
                                Word {
                                    raw: "printf".into(),
                                },
                                Word { raw: "ok".into() },
                            ],
                            ..SimpleCommand::default()
                        }),
                        Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "wc".into() }, Word { raw: "-c".into() }],
                            ..SimpleCommand::default()
                        }),
                    ],
                };
                let status =
                    execute_pipeline(&mut shell, &pipeline, false).expect("negated pipeline");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn build_process_from_expanded_covers_empty_and_assignment_env() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let error = build_process_from_expanded(
                &shell,
                &ExpandedSimpleCommand {
                    assignments: Vec::new(),
                    argv: Vec::new(),
                    redirections: Vec::new(),
                },
            )
            .expect_err("empty command");
            assert_eq!(error.message, "empty command");

            let mut shell = test_shell();
            shell.env.insert("PATH".into(), String::new());
            let prepared = build_process_from_expanded(
                &shell,
                &ExpandedSimpleCommand {
                    assignments: vec![("ASSIGN_VAR".to_string(), "works".to_string())],
                    argv: vec!["echo".to_string(), "hello".to_string()],
                    redirections: Vec::new(),
                },
            )
            .expect("process");
            assert_eq!(
                prepared.child_env,
                vec![("ASSIGN_VAR".into(), "works".into())]
            );
            assert_eq!(prepared.argv, vec!["echo", "hello"]);
        });
    }

    #[test]
    fn spawn_prepared_enoexec_falls_back_to_source() {
        run_trace(
            vec![
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
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
                    argv: vec!["/tmp/script.sh".into(), "arg1".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let child = spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::None)
                    .expect("enoexec fallback spawn");
                let output = child.wait_with_output().expect("output");
                assert!(output.status.success());
            },
        );
    }

    #[test]
    fn spawn_prepared_errors_for_missing_executable() {
        run_trace(
            vec![t(
                "access",
                vec![
                    ArgMatcher::Str("/nonexistent/missing".into()),
                    ArgMatcher::Int(0),
                ],
                TraceResult::Err(sys::ENOENT),
            )],
            || {
                let missing = PreparedProcess {
                    exec_path: "/nonexistent/missing".into(),
                    argv: vec!["missing".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: false,
                };
                let shell = test_shell();
                assert!(
                    spawn_prepared(&shell, &missing, None, false, ProcessGroupPlan::None).is_err()
                );
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
                words: vec![Word { raw: "echo".into() }],
                redirections: vec![
                    Redirection {
                        fd: Some(5),
                        kind: RedirectionKind::ReadWrite,
                        target: Word { raw: "rw".into() },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(0),
                        kind: RedirectionKind::DupInput,
                        target: Word { raw: "5".into() },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(1),
                        kind: RedirectionKind::DupOutput,
                        target: Word { raw: "-".into() },
                        here_doc: None,
                    },
                ],
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
                            target: "-".into(),
                            here_doc_body: None,
                        },
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::ReadWrite,
                            target: "/tmp/rw.txt".into(),
                            here_doc_body: None,
                        },
                    ],
                    false,
                )
                .expect("prepare");
                assert!(prepared.stdin_redirected);
                assert!(prepared.stdout_redirected);
                assert_eq!(prepared.actions.len(), 2);
            },
        );
    }

    #[test]
    fn heredoc_expansion_error_paths() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let error = expand_simple(
                &mut shell,
                &SimpleCommand {
                    words: vec![Word { raw: "cat".into() }],
                    redirections: vec![Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word { raw: "EOF".into() },
                        here_doc: None,
                    }],
                    ..SimpleCommand::default()
                },
            )
            .expect_err("expected missing here-document body");
            assert_eq!(error.message, "missing here-document body");

            let error = prepare_redirections(
                &[ExpandedRedirection {
                    fd: 1,
                    kind: RedirectionKind::DupOutput,
                    target: "bad".into(),
                    here_doc_body: None,
                }],
                false,
            )
            .expect_err("bad dup target");
            assert_eq!(
                error.message,
                "redirection target must be a file descriptor or '-'"
            );

            let mut shell = test_shell();
            let expanded = expand_redirections(
                &mut shell,
                &[Redirection {
                    fd: None,
                    kind: RedirectionKind::HereDoc,
                    target: Word { raw: "EOF".into() },
                    here_doc: Some(HereDoc {
                        delimiter: "EOF".into(),
                        body: "hello $USER".into(),
                        expand: true,
                        strip_tabs: false,
                    }),
                }],
            )
            .expect("expand heredoc redirection");
            assert_eq!(expanded[0].target, "EOF");
            assert_eq!(expanded[0].here_doc_body.as_deref(), Some("hello "));

            let mut shell = test_shell();
            let literal = expand_redirections(
                &mut shell,
                &[Redirection {
                    fd: None,
                    kind: RedirectionKind::HereDoc,
                    target: Word { raw: "EOF".into() },
                    here_doc: Some(HereDoc {
                        delimiter: "EOF".into(),
                        body: "hello $USER".into(),
                        expand: false,
                        strip_tabs: false,
                    }),
                }],
            )
            .expect("literal heredoc redirection");
            assert_eq!(literal[0].here_doc_body.as_deref(), Some("hello $USER"));

            let mut shell = test_shell();
            let error = expand_redirections(
                &mut shell,
                &[Redirection {
                    fd: None,
                    kind: RedirectionKind::HereDoc,
                    target: Word { raw: "EOF".into() },
                    here_doc: None,
                }],
            )
            .expect_err("missing expanded heredoc body");
            assert_eq!(error.message, "missing here-document body");
        });
    }

    #[test]
    fn prepare_redirections_creates_heredoc_pipe() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Int(5),
                ),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            ],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: "EOF".into(),
                        here_doc_body: Some("body\n".into()),
                    }],
                    false,
                )
                .expect("prepare heredoc");
                assert!(prepared.stdin_redirected);
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
                        target: "EOF".into(),
                        here_doc_body: Some("body\n".into()),
                    }],
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.message.is_empty());
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
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: "true".to_string(),
                                }],
                                ..SimpleCommand::default()
                            })],
                        },
                        rest: Vec::new(),
                    },
                    asynchronous: false,
                }],
            };

            let function = FunctionDef {
                name: "greet".to_string(),
                body: Box::new(Command::Group(program.clone())),
            };
            let if_command = IfCommand {
                condition: program.clone(),
                then_branch: program.clone(),
                elif_branches: Vec::new(),
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
                    name: "X".to_string(),
                    value: Word {
                        raw: "1".to_string(),
                    },
                }],
                words: vec![Word {
                    raw: "echo".to_string(),
                }],
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word {
                        raw: "out".to_string(),
                    },
                    here_doc: None,
                }],
            };
            assert_eq!(render_simple(&simple), "X=1 echo >out");

            let pipeline = Pipeline {
                negated: true,
                commands: vec![
                    Command::Subshell(program.clone()),
                    Command::Group(program.clone()),
                    Command::FunctionDef(function),
                    Command::If(if_command),
                    Command::Loop(loop_command),
                ],
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
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: "true".to_string(),
                            }],
                            ..SimpleCommand::default()
                        })],
                    },
                    rest: Vec::new(),
                },
                asynchronous: false,
            }],
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
                        commands: vec![
                            Command::Subshell(program.clone()),
                            Command::Group(program.clone()),
                        ],
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
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word { raw: "true".into() }],
                                    ..SimpleCommand::default()
                                })],
                            },
                            rest: Vec::new(),
                        },
                        asynchronous: true,
                    },
                    ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: "false".into(),
                                    }],
                                    ..SimpleCommand::default()
                                })],
                            },
                            rest: Vec::new(),
                        },
                        asynchronous: false,
                    },
                ],
            };
            assert_eq!(render_list_item(&async_program.items[0]), "true &");
            assert_eq!(render_program(&async_program), "true &\nfalse");

            let heredoc_program =
                crate::syntax::parse(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
            assert_eq!(render_program(&heredoc_program), ": <<EOF\nhello\nEOF");
        });
    }

    #[test]
    fn execute_nested_program_sets_up_heredoc_fd() {
        run_trace(
            vec![
                t("dup", vec![ArgMatcher::Fd(0)], TraceResult::Err(sys::EBADF)),
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"hello\n".to_vec())],
                    TraceResult::Int(6),
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
                let heredoc_program =
                    crate::syntax::parse(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
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
            let if_program = crate::syntax::parse(
                "if false; then VALUE=no; elif true; then VALUE=yes; else VALUE=bad; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("yes"));

            let mut shell = test_shell();
            let while_program = crate::syntax::parse(
                "COUNTER=1; while case $COUNTER in 0) false ;; *) true ;; esac; do COUNTER=0; FLAG=done; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &while_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FLAG").as_deref(), Some("done"));

            let mut shell = test_shell();
            let until_program = crate::syntax::parse(
                "READY=; until case $READY in yes) true ;; *) false ;; esac; do READY=yes; VALUE=ready; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &until_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("ready"));
        });
    }

    #[test]
    fn execute_for_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program =
                crate::syntax::parse("for item in a b c; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("LAST").as_deref(), Some("c"));

            let mut shell = test_shell();
            shell.positional = vec!["alpha".into(), "beta".into()];
            let program = crate::syntax::parse("for item; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("LAST").as_deref(), Some("beta"));
        });
    }

    #[test]
    fn execute_case_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "name=beta; case $name in alpha) VALUE=no ;; b*) VALUE=yes ;; esac",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("yes"));

            let mut shell = test_shell();
            let program =
                crate::syntax::parse("name=zeta; case $name in alpha|beta) VALUE=hit ;; esac")
                    .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);
        });
    }

    #[test]
    fn execute_if_covers_then_and_else_branches() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
            shell.exported.insert("PATH".into());

            let if_program =
                crate::syntax::parse("if true; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("yes"));

            let mut shell = test_shell();
            let if_program =
                crate::syntax::parse("if false; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("no"));

            let mut shell = test_shell();
            let if_program = crate::syntax::parse(
                "if false; then VALUE=yes; elif false; then VALUE=maybe; else VALUE=no; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("no"));
        });
    }

    #[test]
    fn render_and_or_produces_correct_output() {
        assert_no_syscalls(|| {
            let render = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word { raw: "true".into() }],
                        ..SimpleCommand::default()
                    })],
                },
                rest: vec![(
                    LogicalOp::And,
                    Pipeline {
                        negated: false,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: "false".into(),
                            }],
                            ..SimpleCommand::default()
                        })],
                    },
                )],
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
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word { raw: "true".into() }],
                            ..SimpleCommand::default()
                        })],
                    },
                    rest: Vec::new(),
                },
                asynchronous: false,
            }],
        };
        let pipeline = Pipeline {
            negated: false,
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
                    }],
                    else_branch: Some(program.clone()),
                }),
                Command::Loop(LoopCommand {
                    kind: LoopKind::Until,
                    condition: program.clone(),
                    body: program,
                }),
                Command::For(ForCommand {
                    name: "item".into(),
                    items: Some(vec![Word { raw: "a".into() }]),
                    body: Program::default(),
                }),
                Command::Case(CaseCommand {
                    word: Word { raw: "item".into() },
                    arms: vec![crate::syntax::CaseArm {
                        patterns: vec![Word { raw: "item".into() }],
                        body: Program::default(),
                    }],
                }),
            ],
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
                items: Some(vec![Word { raw: "a".into() }, Word { raw: "b".into() }]),
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
                words: vec![Word { raw: "echo".into() }],
                redirections: vec![
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::Read,
                        target: Word { raw: "in".into() },
                        here_doc: None,
                    },
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::Append,
                        target: Word { raw: "out".into() },
                        here_doc: None,
                    },
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word { raw: "EOF".into() },
                        here_doc: Some(crate::syntax::HereDoc {
                            delimiter: "EOF".into(),
                            body: "body\n".into(),
                            expand: false,
                            strip_tabs: false,
                        }),
                    },
                ],
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            assert!(rendered.contains("<in"));
            assert!(rendered.contains(">>out"));
            assert!(rendered.contains("<<EOF"));
            assert!(rendered.contains("body\nEOF"));

            let strip_tabs = SimpleCommand {
                words: vec![Word { raw: "cat".into() }],
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::HereDoc,
                    target: Word { raw: "EOF".into() },
                    here_doc: Some(crate::syntax::HereDoc {
                        delimiter: "EOF".into(),
                        body: "body".into(),
                        expand: false,
                        strip_tabs: true,
                    }),
                }],
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&strip_tabs);
            assert!(rendered.contains("<<-EOF"));
            assert!(rendered.contains("body\nEOF"));

            let case_command = CaseCommand {
                word: Word {
                    raw: "$item".into(),
                },
                arms: vec![crate::syntax::CaseArm {
                    patterns: vec![Word { raw: "a*".into() }, Word { raw: "b".into() }],
                    body: Program::default(),
                }],
            };
            assert!(render_case(&case_command).contains("a* | b)"));
            assert!(
                render_pipeline(&Pipeline {
                    negated: false,
                    commands: vec![Command::Case(case_command)],
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
        });
    }

    #[test]
    fn render_and_or_covers_or_and_for_variants() {
        assert_no_syscalls(|| {
            let render = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: "false".into(),
                        }],
                        ..SimpleCommand::default()
                    })],
                },
                rest: vec![(
                    LogicalOp::Or,
                    Pipeline {
                        negated: false,
                        commands: vec![Command::For(ForCommand {
                            name: "item".into(),
                            items: Some(vec![Word { raw: "a".into() }]),
                            body: Program::default(),
                        })],
                    },
                )],
            });
            assert!(render.contains("||"));
            assert!(render.contains("for item in a"));
        });
    }

    #[test]
    fn loop_and_function_exit_behavior() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = crate::syntax::parse("if false; then VALUE=yes; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);

            let mut shell = test_shell();
            let for_program =
                crate::syntax::parse("for item in a b; do exit 9; done").expect("parse");
            let status = execute_program(&mut shell, &for_program).expect("exec");
            assert_eq!(status, 9);
            assert!(!shell.running);
            assert_eq!(shell.get_var("item").as_deref(), Some("a"));

            let mut shell = test_shell();
            let loop_program = crate::syntax::parse("while true; do exit 7; done").expect("parse");
            let status = execute_program(&mut shell, &loop_program).expect("exec");
            assert_eq!(status, 7);
            assert!(!shell.running);

            let mut shell = test_shell();
            let program =
                crate::syntax::parse("greet() { RESULT=$X; }; X=ok greet").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("RESULT").as_deref(), Some("ok"));
        });
    }

    #[test]
    fn control_flow_propagates_across_functions_and_loops() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = crate::syntax::parse("f() { return 6; VALUE=bad; }; f").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 6);
            assert_eq!(shell.get_var("VALUE"), None);
            assert_eq!(shell.pending_control, None);

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "for outer in x y; do for inner in a b; do continue 2; VALUE=bad; done; printf no; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);
            assert_eq!(shell.get_var("outer").as_deref(), Some("y"));

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "for outer in x y; do for inner in a b; do break 2; done; VALUE=bad; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), None);
            assert_eq!(shell.get_var("outer").as_deref(), Some("x"));

            let mut shell = test_shell();
            let program =
                crate::syntax::parse("f() { while true; do return 4; done; }; f").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 4);
            assert_eq!(shell.pending_control, None);

            let mut shell = test_shell();
            let program = crate::syntax::parse("g() { break; }; g").expect("parse");
            let error = execute_program(&mut shell, &program).expect_err("function error");
            assert_eq!(error.message, "break: only meaningful in a loop");

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "for outer in x; do while break 2; do printf no; done; AFTER=bad; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("AFTER"), None);

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "for outer in x; do while continue 2; do printf no; done; AFTER=bad; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("AFTER"), None);

            let mut shell = test_shell();
            let program = crate::syntax::parse("f() { while return 3; do printf no; done; }; f")
                .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 3);

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "COUNT=1; while case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; do printf no; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("COUNT").as_deref(), Some("0"));

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "COUNT=1; while true; do case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; printf no; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("COUNT").as_deref(), Some("0"));

            let mut shell = test_shell();
            let program = crate::syntax::parse("f() { for item in a; do return 5; done; }; f")
                .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 5);

            let mut shell = test_shell();
            let program = crate::syntax::parse(
                "for outer in x; do for inner in y; do break 2; done; DONE=no; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("DONE"), None);

            let mut shell = test_shell();
            let program = crate::syntax::parse(
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
                condition: crate::syntax::parse("true").expect("parse"),
                body: crate::syntax::parse("break 2").expect("parse"),
            };
            let status = execute_loop(&mut shell, &loop_command).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.pending_control, Some(PendingControl::Break(1)));

            let mut shell = test_shell();
            shell.loop_depth = 1;
            let loop_command = LoopCommand {
                kind: LoopKind::While,
                condition: crate::syntax::parse("true").expect("parse"),
                body: crate::syntax::parse("continue 2").expect("parse"),
            };
            let status = execute_loop(&mut shell, &loop_command).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.pending_control, Some(PendingControl::Continue(1)));
        });
    }

    #[test]
    fn render_simple_handles_clobber_write() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word { raw: "echo".into() }],
                redirections: vec![Redirection {
                    fd: Some(1),
                    kind: RedirectionKind::ClobberWrite,
                    target: Word { raw: "out".into() },
                    here_doc: None,
                }],
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
                t("dup", vec![ArgMatcher::Fd(42)], TraceResult::Fd(92)),
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
                            target: "/redir/write.txt".into(),
                            here_doc_body: None,
                        },
                        ExpandedRedirection {
                            fd: target_fd,
                            kind: RedirectionKind::Append,
                            target: "/redir/append.txt".into(),
                            here_doc_body: None,
                        },
                    ],
                    false,
                )
                .expect("redir guard");
                drop(guard);

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::Read,
                        target: "/redir/input.txt".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect("read redirection");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::ReadWrite,
                        target: "/redir/rw.txt".into(),
                        here_doc_body: None,
                    },
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
                    TraceResult::Int(5),
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
                        target: "EOF".into(),
                        here_doc_body: Some("body\n".into()),
                    },
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
                        target: "EOF".into(),
                        here_doc_body: Some("body\n".into()),
                    },
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.message.is_empty());
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
                        target: "1".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect("dup output");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    },
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
                // DupOutput "bad" → error, no OS calls
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
                // apply_shell_redirections with dup returning EBADF for high fd → treated as absent
                t(
                    "dup",
                    vec![ArgMatcher::Fd(123_456)],
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
                // apply_shell_redirections with dup failure (errno 22)
                t(
                    "dup",
                    vec![ArgMatcher::Fd(42)],
                    TraceResult::Err(sys::EINVAL),
                ),
            ],
            || {
                let error = apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "bad".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect_err("bad dup output");
                assert_eq!(
                    error.message,
                    "redirection target must be a file descriptor or '-'"
                );

                let error = apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::Write,
                        target: "/redir/noclobber.txt".into(),
                        here_doc_body: None,
                    },
                    true,
                )
                .expect_err("noclobber");
                assert!(!error.message.is_empty());

                drop(ShellRedirectionGuard {
                    saved: vec![(99, None)],
                });

                let guard = apply_shell_redirections(
                    &[ExpandedRedirection {
                        fd: 123_456,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    }],
                    false,
                )
                .expect("invalid fd is treated as absent");
                drop(guard);

                let error = apply_shell_redirections(
                    &[ExpandedRedirection {
                        fd: 42,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    }],
                    false,
                )
                .expect_err("dup failure");
                assert!(!error.message.is_empty());
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
                assert!(!error.message.is_empty());
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
                    target: Word { raw: "out".into() },
                    here_doc: None,
                }],
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
                    argv: vec!["/tmp/script.sh".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: false,
                };
                let child =
                    spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::NewGroup)
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
                    argv: vec!["/bin/echo".into(), "hello".into()],
                    child_env: Vec::new(),
                    redirections: vec![ExpandedRedirection {
                        fd: 1,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    }],
                    noclobber: false,
                    path_verified: false,
                };
                let child = spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::None)
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
                    argv: vec!["/bin/echo".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: false,
                };
                let child =
                    spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::Join(42))
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
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word { raw: "true".into() }],
                                ..SimpleCommand::default()
                            })],
                        },
                        rest: Vec::new(),
                    },
                    asynchronous: false,
                }],
            };
            let if_command = IfCommand {
                condition: program.clone(),
                then_branch: program.clone(),
                elif_branches: vec![crate::syntax::ElifBranch {
                    condition: program.clone(),
                    body: program.clone(),
                }],
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
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word { raw: "true".into() }],
                                ..SimpleCommand::default()
                            })],
                        },
                        rest: Vec::new(),
                    },
                    asynchronous: false,
                }],
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
                t("dup", vec![ArgMatcher::Fd(1)], TraceResult::Fd(51)),
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
                t("dup", vec![ArgMatcher::Fd(0)], TraceResult::Fd(53)),
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
                    target: "/tmp/out".into(),
                    here_doc_body: None,
                }];
                with_shell_redirections(&redirections, false, || Ok(0)).expect("append");

                let redirections = vec![ExpandedRedirection {
                    fd: 0,
                    kind: RedirectionKind::ReadWrite,
                    target: "/tmp/rw".into(),
                    here_doc_body: None,
                }];
                with_shell_redirections(&redirections, false, || Ok(0)).expect("readwrite");
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
                        target: "/tmp/log".into(),
                        here_doc_body: None,
                    },
                    ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::ReadWrite,
                        target: "/tmp/rw".into(),
                        here_doc_body: None,
                    },
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
                    vec![t(
                        "execvp",
                        vec![ArgMatcher::Str("/bin/true".into()), ArgMatcher::Any],
                        TraceResult::Int(0),
                    )],
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
                    argv: vec!["true".into()],
                    redirections: vec![],
                    child_env: vec![],
                    path_verified: false,
                    noclobber: false,
                };
                let handle =
                    spawn_prepared(&mut shell, &prepared, None, false, ProcessGroupPlan::None)
                        .expect("spawn");
                let ws = sys::wait_pid(handle.pid, false)
                    .expect("wait")
                    .expect("status");
                assert_eq!(sys::decode_wait_status(ws.status), 0);
            },
        );
    }

    #[test]
    fn spawn_prepared_stdin_closed_when_redirected() {
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
                t("close", vec![ArgMatcher::Fd(100)], TraceResult::Int(0)),
                t_fork(
                    TraceResult::Pid(1000),
                    vec![
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(60), ArgMatcher::Fd(0)],
                            TraceResult::Int(0),
                        ),
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
                    argv: vec!["cat".into()],
                    redirections: vec![ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::Read,
                        target: "/tmp/in".into(),
                        here_doc_body: None,
                    }],
                    child_env: vec![],
                    path_verified: true,
                    noclobber: false,
                };
                let _handle = spawn_prepared(
                    &mut shell,
                    &prepared,
                    Some(100),
                    false,
                    ProcessGroupPlan::None,
                )
                .expect("spawn");
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
                    argv: vec!["true".into()],
                    redirections: vec![],
                    child_env: vec![("MEIKSH_TEST_COVERAGE".into(), "1".into())],
                    path_verified: true,
                    noclobber: false,
                };
                let _handle =
                    spawn_prepared(&mut shell, &prepared, None, false, ProcessGroupPlan::None)
                        .expect("spawn");
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
            assert_eq!(shell.get_var("FOO"), Some("temp".into()));
            assert_eq!(shell.get_var("BAR"), Some("new".into()));

            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var("FOO"), Some("original".into()));
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
            let program = crate::syntax::parse("FOO=temp true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("original".into()));
        });
    }

    #[test]
    fn special_builtin_prefix_assignments_are_permanent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = crate::syntax::parse("FOO=permanent :").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("permanent".into()));
        });
    }

    #[test]
    fn function_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("FOO".into(), "original".into());
            let program = crate::syntax::parse("myfn() { :; }; FOO=temp myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("FOO"), Some("original".into()));
        });
    }

    #[test]
    fn non_special_builtin_exit_with_temp_assignments() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = crate::syntax::parse("FOO=bar exit 0").expect("parse");
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
            let program = crate::syntax::parse("Y=$X").expect("parse");
            let _status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(shell.get_var("Y"), Some("a b c".into()));
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
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/tmp/noexec.sh".into(),
                    argv: vec!["noexec.sh".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: false,
                };
                let err = spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::None)
                    .unwrap_err();
                assert!(
                    err.message.contains("Permission denied") || err.message.contains("errno 13")
                );
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
                        "execvp",
                        vec![ArgMatcher::Str("/bin/noperm".into()), ArgMatcher::Any],
                        TraceResult::Err(sys::EACCES),
                    ),
                ],
            )],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/noperm".into(),
                    argv: vec!["noperm".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let _handle =
                    spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::NewGroup)
                        .expect("spawn");
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
                        "execvp",
                        vec![ArgMatcher::Str("/bin/missing".into()), ArgMatcher::Any],
                        TraceResult::Err(sys::ENOENT),
                    ),
                ],
            )],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: "/bin/missing".into(),
                    argv: vec!["missing".into()],
                    child_env: Vec::new(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let _handle =
                    spawn_prepared(&shell, &prepared, None, false, ProcessGroupPlan::NewGroup)
                        .expect("spawn");
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
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: "true".to_string(),
                            }],
                            ..SimpleCommand::default()
                        })],
                    },
                    rest: vec![(
                        LogicalOp::And,
                        Pipeline {
                            negated: false,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: ":".to_string(),
                                }],
                                ..SimpleCommand::default()
                            })],
                        },
                    )],
                };
                let spawned = spawn_and_or(&mut shell, &node, Some(50)).expect("spawn");
                assert_eq!(spawned.children.len(), 1);
                assert!(spawned.pgid.is_some());
            },
        );
    }
}
