use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builtin;
use crate::expand;
use crate::shell::{FlowSignal, PendingControl, Shell, ShellError};
use crate::sys;
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, IfCommand, ListItem, LogicalOp,
    HereDoc, LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
enum ProcessGroupPlan {
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
        let spawned = spawn_and_or(shell, &item.and_or)?;
        let description = render_and_or(&item.and_or);
        let id = shell.launch_background_job(description, spawned.pgid, spawned.children);
        println!("[{id}]");
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

fn spawn_and_or(shell: &mut Shell, node: &AndOr) -> Result<SpawnedProcesses, ShellError> {
    if !node.rest.is_empty() {
        return Err(ShellError {
            message: "background execution currently supports single pipelines".to_string(),
        });
    }
    spawn_pipeline(shell, &node.first)
}

fn execute_pipeline(shell: &mut Shell, pipeline: &Pipeline, asynchronous: bool) -> Result<i32, ShellError> {
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

    let spawned = spawn_pipeline(shell, pipeline)?;
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

fn spawn_pipeline(shell: &mut Shell, pipeline: &Pipeline) -> Result<SpawnedProcesses, ShellError> {
    let mut previous_stdout_fd: Option<i32> = None;
    let mut children = Vec::new();
    let mut pgid = None;

    for (index, command) in pipeline.commands.iter().enumerate() {
        let is_last = index + 1 == pipeline.commands.len();
        let prepared = match command {
            Command::Simple(simple) => build_process(shell, simple)?,
            Command::Subshell(program) | Command::Group(program) => {
                prepared_shell_process(shell, render_program(program))
            }
            Command::FunctionDef(function) => {
                prepared_shell_process(shell, render_function(function))
            }
            Command::If(if_command) => {
                prepared_shell_process(shell, render_if(if_command))
            }
            Command::Loop(loop_command) => {
                prepared_shell_process(shell, render_loop(loop_command))
            }
            Command::For(for_command) => {
                prepared_shell_process(shell, render_for(for_command))
            }
            Command::Case(case_command) => {
                prepared_shell_process(shell, render_case(case_command))
            }
            Command::Redirected(command, redirections) => {
                prepared_shell_process(shell, render_redirected_command(command, redirections))
            }
        };

        let plan = match pgid {
            Some(pgid) => ProcessGroupPlan::Join(pgid),
            None => ProcessGroupPlan::NewGroup,
        };
        let handle = spawn_prepared(&prepared, previous_stdout_fd.take(), !is_last, plan)?;
        if pgid.is_none() {
            let child_pgid = handle.pid;
            let _ = sys::set_process_group(child_pgid, child_pgid);
            pgid = Some(child_pgid);
        } else if let Some(job_pgid) = pgid {
            let _ = sys::set_process_group(handle.pid, job_pgid);
        }
        previous_stdout_fd = handle.stdout_fd;
        children.push(sys::ChildHandle { pid: handle.pid, stdout_fd: None });
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
            let child_env = shell.env_for_child();
            let env_pairs: Vec<(&str, &str)> = child_env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let script = render_program(program);
            let exe = shell.current_exe.display().to_string();
            let argv = [exe.as_str(), "-c", &script];
            let status = sys::run_to_status(&exe, &argv, Some(&env_pairs))?;
            Ok(status)
        }
        Command::Group(program) => execute_nested_program(shell, program),
        Command::FunctionDef(function) => {
            shell.functions
                .insert(function.name.clone(), (*function.body).clone());
            Ok(0)
        }
        Command::If(if_command) => execute_if(shell, if_command),
        Command::Loop(loop_command) => execute_loop(shell, loop_command),
        Command::For(for_command) => execute_for(shell, for_command),
        Command::Case(case_command) => execute_case(shell, case_command),
        Command::Redirected(command, redirections) => execute_redirected(shell, command, redirections),
    }
}

fn execute_redirected(
    shell: &mut Shell,
    command: &Command,
    redirections: &[crate::syntax::Redirection],
) -> Result<i32, ShellError> {
    let expanded = expand_redirections(shell, redirections)?;
    with_shell_redirections(&expanded, shell.options.noclobber, || execute_command(shell, command))
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
        let result = with_shell_redirections(&expanded.redirections, shell.options.noclobber, || {
            match shell.run_builtin(&expanded.argv, &expanded.assignments)? {
                FlowSignal::Continue(status) => Ok(status),
                FlowSignal::Exit(status) => {
                    shell.running = false;
                    Ok(status)
                }
            }
        });
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
            for (name, value) in expanded.assignments {
                shell.set_var(&name, value)?;
            }
            let saved = std::mem::replace(&mut shell.positional, expanded.argv[1..].to_vec());
            shell.function_depth += 1;
            let status = execute_command(shell, &function);
            shell.function_depth = shell.function_depth.saturating_sub(1);
            shell.positional = saved;
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
        let handle = spawn_prepared(&prepared, None, false, ProcessGroupPlan::NewGroup)?;
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
}

#[derive(Debug)]
#[allow(dead_code)]
enum ChildFdAction {
    DupRawFd { fd: i32, target_fd: i32, close_source: bool },
    DupFd { source_fd: i32, target_fd: i32 },
    CloseFd { target_fd: i32 },
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

fn expand_simple(shell: &mut Shell, simple: &SimpleCommand) -> Result<ExpandedSimpleCommand, ShellError> {
    let mut assignments = Vec::new();
    for assignment in &simple.assignments {
        let fields = expand::expand_word(shell, &assignment.value)?;
        assignments.push((assignment.name.clone(), fields.first().cloned().unwrap_or_default()));
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

fn prepared_shell_process(shell: &Shell, script: String) -> PreparedProcess {
    PreparedProcess {
        exec_path: shell.current_exe.display().to_string(),
        argv: vec![
            shell.current_exe.display().to_string(),
            "-c".to_string(),
            script,
        ],
        child_env: shell.env_for_child().into_iter().collect(),
        redirections: Vec::new(),
        noclobber: shell.options.noclobber,
    }
}

fn build_process_from_expanded(
    shell: &Shell,
    expanded: &ExpandedSimpleCommand,
) -> Result<PreparedProcess, ShellError> {
    let program = expanded.argv.first().ok_or_else(|| ShellError {
        message: "empty command".to_string(),
    })?;
    let exec_path = resolve_command_path(shell, program)
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
    })
}

fn spawn_prepared(
    prepared: &PreparedProcess,
    stdin_fd: Option<i32>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    spawn_prepared_inner(prepared, stdin_fd, pipe_stdout, process_group, false)
}

fn spawn_prepared_inner(
    prepared: &PreparedProcess,
    stdin_fd: Option<i32>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
    fallback_to_sh: bool,
) -> Result<sys::ChildHandle, ShellError> {
    if !fallback_to_sh && !prepared.exec_path.is_empty() && prepared.exec_path.contains('/') {
        if sys::access_path(&prepared.exec_path, sys::F_OK).is_err() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound,
                format!("{}: not found", prepared.exec_path)).into());
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

    let mut fd_redirections: Vec<(i32, i32)> = Vec::new();
    for action in &prepared_redirections.actions {
        match action {
            ChildFdAction::DupRawFd { fd, target_fd, .. } => {
                fd_redirections.push((*fd, *target_fd));
            }
            ChildFdAction::DupFd { source_fd, target_fd } => {
                fd_redirections.push((*source_fd, *target_fd));
            }
            ChildFdAction::CloseFd { .. } => {}
        }
    }

    let pgid = match process_group {
        ProcessGroupPlan::NewGroup => Some(0),
        ProcessGroupPlan::Join(pgid) => Some(pgid),
        ProcessGroupPlan::None => None,
    };

    let (program, argv) = if fallback_to_sh {
        let mut v = vec!["sh".to_string(), prepared.exec_path.clone()];
        v.extend(prepared.argv.iter().skip(1).cloned());
        ("sh".to_string(), v)
    } else {
        (prepared.exec_path.clone(), prepared.argv.clone())
    };

    let env_pairs: Vec<(&str, &str)> = prepared.child_env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let argv_strs: Vec<&str> = argv.iter().map(String::as_str).collect();

    let result = sys::spawn_child(
        &program,
        &argv_strs,
        Some(&env_pairs),
        &fd_redirections,
        effective_stdin,
        effective_pipe_stdout,
        pgid,
    );

    for action in &prepared_redirections.actions {
        if let ChildFdAction::DupRawFd { fd, close_source: true, .. } = action {
            let _ = sys::close_fd(*fd);
        }
    }

    match result {
        Ok(handle) => Ok(handle),
        Err(error) if error.raw_os_error() == Some(8) && !fallback_to_sh => {
            spawn_prepared_inner(prepared, None, pipe_stdout, process_group, true)
        }
        Err(error) => Err(error.into()),
    }
}

// These functions are used by tests directly
#[allow(dead_code)]
fn spawn_with_fallback(
    prepared: &PreparedProcess,
    stdin_fd: Option<i32>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    spawn_prepared_inner(prepared, stdin_fd, pipe_stdout, process_group, true)
}

#[allow(dead_code)]
fn maybe_spawn_with_fallback(
    error: std::io::Error,
    prepared: &PreparedProcess,
    stdin_fd: Option<i32>,
    pipe_stdout: bool,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    if error.raw_os_error() == Some(8) {
        spawn_with_fallback(prepared, stdin_fd, pipe_stdout, process_group)
    } else {
        Err(error.into())
    }
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
                let fd = sys::open_file(
                    &redirection.target,
                    sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC,
                    0o666,
                )?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::HereDoc => {
                let (read_fd, write_fd) = sys::create_pipe()?;
                sys::write_all_fd(write_fd, redirection.here_doc_body.as_deref().unwrap_or("").as_bytes())?;
                sys::close_fd(write_fd)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd: read_fd,
                    target_fd: redirection.fd,
                    close_source: true,
                });
            }
            RedirectionKind::ReadWrite => {
                let fd = sys::open_file(
                    &redirection.target,
                    sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC,
                    0o666,
                )?;
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
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();

    path.split(':')
        .filter(|segment| !segment.is_empty())
        .map(|segment| Path::new(segment).join(program))
        .find(|candidate| sys::is_regular_file(&candidate.display().to_string()))
}

// PreparedProcess.build_command() is replaced by spawn_prepared_inner()
// PipelineInput is replaced by raw fd from ChildHandle.stdout_fd

#[allow(dead_code)]
fn apply_child_fd_actions(actions: &[ChildFdAction]) -> std::io::Result<()> {
    for action in actions {
        match action {
            ChildFdAction::DupRawFd { fd, target_fd, .. } => {
                sys::duplicate_fd(*fd, *target_fd)?;
            }
            ChildFdAction::DupFd { source_fd, target_fd } => {
                sys::duplicate_fd(*source_fd, *target_fd)?;
            }
            ChildFdAction::CloseFd { target_fd } => {
                if let Err(error) = sys::close_fd(*target_fd) {
                    if error.raw_os_error() != Some(9) {
                        return Err(error);
                    }
                }
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn apply_child_setup(actions: &[ChildFdAction], process_group: ProcessGroupPlan) -> std::io::Result<()> {
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
                Err(error) if error.raw_os_error() == Some(9) => None,
                Err(error) => return Err(error.into()),
            };
            entry.insert(original);
            guard.saved.push((redirection.fd, original));
        }
        apply_shell_redirection(redirection, noclobber)?;
    }

    Ok(guard)
}

fn apply_shell_redirection(redirection: &ExpandedRedirection, noclobber: bool) -> Result<(), ShellError> {
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
            let fd = sys::open_file(
                &redirection.target,
                sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC,
                0o666,
            )?;
            replace_shell_fd(fd, redirection.fd)?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::create_pipe()?;
            sys::write_all_fd(write_fd, redirection.here_doc_body.as_deref().unwrap_or("").as_bytes())?;
            sys::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd)?;
        }
        RedirectionKind::ReadWrite => {
            let fd = sys::open_file(
                &redirection.target,
                sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC,
                0o666,
            )?;
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
        if error.raw_os_error() != Some(9) {
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
            None => ti < text.len() && text[ti] == '[' && match_pattern(text, ti + 1, pattern, pi + 1),
        },
        '\\' if pi + 1 < pattern.len() => {
            ti < text.len() && text[ti] == pattern[pi + 1] && match_pattern(text, ti + 1, pattern, pi + 2)
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
        Command::Redirected(command, redirections) => render_redirected_command(command, redirections),
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
    format!("{}() {}", function.name, render_pipeline(&Pipeline {
        negated: false,
        commands: vec![(*function.body).clone()],
    }))
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
    text.push_str(&format!("\ndo\n{}\ndone", render_program(&for_command.body)));
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

fn render_redirections_with_bodies(redirections: &[crate::syntax::Redirection]) -> (String, Vec<String>) {
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
            if redirection.here_doc.as_ref().is_some_and(|here_doc| here_doc.strip_tabs) {
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

fn render_command_line_with_redirections(base: String, redirections: &[crate::syntax::Redirection]) -> String {
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
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::fs;
    use std::fs::File;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::raw::c_int;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::test_utils::{cwd_lock, meiksh_bin_path};

    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }

    fn set_errno(value: c_int) {
        unsafe {
            *__error() = value;
        }
    }

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
            current_exe: meiksh_bin_path(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        }
    }

    #[test]
    fn execute_and_or_skips_rhs_when_guard_fails() {
        let mut shell = test_shell();
        let program = crate::syntax::parse("true || false").expect("parse");
        let status = execute_program(&mut shell, &program).expect("execute");
        assert_eq!(status, 0);

        let program = crate::syntax::parse("false && true").expect("parse");
        let status = execute_program(&mut shell, &program).expect("execute");
        assert_eq!(status, 1);
    }

    #[test]
    fn execute_pipeline_covers_async_and_negated_multi_command_paths() {
        let mut shell = test_shell();
        let pipeline = Pipeline {
            negated: false,
            commands: vec![Command::Simple(SimpleCommand {
                words: vec![Word {
                    raw: "true".to_string(),
                }],
                ..SimpleCommand::default()
            })],
        };
        let status = execute_pipeline(&mut shell, &pipeline, true).expect("async");
        assert_eq!(status, 0);

        let mut shell = test_shell();
        let pipeline = Pipeline {
            negated: true,
            commands: vec![
                Command::Simple(SimpleCommand {
                    words: vec![Word {
                        raw: "printf".to_string(),
                    }, Word {
                        raw: "ok".to_string(),
                    }],
                    ..SimpleCommand::default()
                }),
                Command::Simple(SimpleCommand {
                    words: vec![Word {
                        raw: "wc".to_string(),
                    }, Word {
                        raw: "-c".to_string(),
                    }],
                    ..SimpleCommand::default()
                }),
            ],
        };
        let status = execute_pipeline(&mut shell, &pipeline, false).expect("negated pipeline");
        assert_eq!(status, 1);
    }

    #[test]
    fn build_process_from_expanded_covers_empty_and_assignment_env() {
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
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
        let prepared = build_process_from_expanded(
            &shell,
            &ExpandedSimpleCommand {
                assignments: vec![("ASSIGN_VAR".to_string(), "works".to_string())],
                argv: vec![shell.current_exe.display().to_string(), "-c".to_string(), "printf \"$ASSIGN_VAR\"".to_string()],
                redirections: Vec::new(),
            },
        )
        .expect("process");
        let child = spawn_prepared(&prepared, None, true, ProcessGroupPlan::None).expect("spawn");
        let output = child.wait_with_output().expect("output");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "works");
    }

    #[test]
    fn enoexec_helpers_cover_fallback_and_error_paths() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("meiksh-exec-unit-{unique}"));
        fs::create_dir(&dir).expect("mkdir");

        let script = dir.join("fallback-script");
        fs::write(&script, "printf unit:$1").expect("write script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod script");

        let prepared = PreparedProcess {
            exec_path: script.display().to_string(),
            argv: vec!["fallback-script".into(), "ok".into()],
            child_env: Vec::new(),
            redirections: Vec::new(),
            noclobber: false,
        };
        let child = spawn_with_fallback(&prepared, None, true, ProcessGroupPlan::None).expect("fallback spawn");
        let output = child.wait_with_output().expect("output");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unit:ok");

        fs::write(&script, "cat").expect("rewrite script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod script");

        let producer = sys::spawn_child(
            &meiksh_bin_path().display().to_string(),
            &[&meiksh_bin_path().display().to_string(), "-c", "printf piped"],
            None, &[], None, true, None,
        ).expect("spawn producer");
        let producer_stdout_fd = producer.stdout_fd.expect("stdout fd");
        let prepared = PreparedProcess {
            exec_path: script.display().to_string(),
            argv: vec!["fallback-script".into()],
            child_env: Vec::new(),
            redirections: Vec::new(),
            noclobber: false,
        };
        let child = spawn_with_fallback(
            &prepared,
            Some(producer_stdout_fd),
            true,
            ProcessGroupPlan::None,
        )
        .expect("fallback pipeline");
        let _ = sys::wait_pid(producer.pid, false);
        let output = child.wait_with_output().expect("output");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "piped");

        let missing = PreparedProcess {
            exec_path: dir.join("missing").display().to_string(),
            argv: vec!["missing".into()],
            child_env: Vec::new(),
            redirections: Vec::new(),
            noclobber: false,
        };
        assert!(spawn_prepared(&missing, None, false, ProcessGroupPlan::None).is_err());

        let _ = fs::remove_file(script);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn redirection_helpers_cover_fd_actions_and_rendering() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let temp = std::env::temp_dir().join(format!("meiksh-fd-action-{unique}.txt"));
        fs::write(&temp, "fd").expect("write temp");

        let owned: OwnedFd = File::open(&temp).expect("open temp").into();
        let raw_fd = owned.as_raw_fd();
        std::mem::forget(owned);
        apply_child_fd_actions(&[
            ChildFdAction::DupRawFd {
                fd: raw_fd,
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
        let _ = sys::close_fd(90);
        let _ = fs::remove_file(temp);

        let simple = SimpleCommand {
            words: vec![Word { raw: "echo".into() }],
            redirections: vec![
                Redirection { fd: Some(5), kind: RedirectionKind::ReadWrite, target: Word { raw: "rw".into() }, here_doc: None },
                Redirection { fd: Some(0), kind: RedirectionKind::DupInput, target: Word { raw: "5".into() }, here_doc: None },
                Redirection { fd: Some(1), kind: RedirectionKind::DupOutput, target: Word { raw: "-".into() }, here_doc: None },
            ],
            ..SimpleCommand::default()
        };
        let rendered = render_simple(&simple);
        assert!(rendered.contains("5<>rw"));
        assert!(rendered.contains("0<&5"));
        assert!(rendered.contains("1>&-"));

        sys::test_support::VfsBuilder::new()
            .dir("/tmp")
            .run(|| {
                let prepared = prepare_redirections(&[
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
                ], false)
                .expect("prepare");
                assert!(prepared.stdin_redirected);
                assert!(prepared.stdout_redirected);
                assert_eq!(prepared.actions.len(), 2);
            });
    }

    #[test]
    fn heredoc_process_helpers_cover_error_paths() {
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

        let error = prepare_redirections(&[ExpandedRedirection {
            fd: 1,
            kind: RedirectionKind::DupOutput,
            target: "bad".into(),
            here_doc_body: None,
        }], false)
        .expect_err("bad dup target");
        assert_eq!(error.message, "redirection target must be a file descriptor or '-'");

        let prepared = prepare_redirections(&[ExpandedRedirection {
            fd: 0,
            kind: RedirectionKind::HereDoc,
            target: "EOF".into(),
            here_doc_body: Some("body\n".into()),
        }], false)
        .expect("prepare heredoc");
        assert!(prepared.stdin_redirected);
        assert_eq!(prepared.actions.len(), 1);

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
    }

    #[test]
    fn spawn_pipeline_and_render_helpers_cover_all_command_forms() {
        let mut shell = test_shell();
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

        let spawned = spawn_pipeline(&mut shell, &Pipeline {
            negated: false,
            commands: vec![
                Command::Subshell(program.clone()),
                Command::Group(program.clone()),
            ],
        })
        .expect("spawn");
        for child in spawned.children {
            let _ = child.wait().expect("wait");
        }
    }

    #[test]
    fn nested_program_helpers_cover_rendering_and_heredoc_detection() {
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
                                words: vec![Word { raw: "false".into() }],
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

        let heredoc_program = crate::syntax::parse(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
        assert_eq!(render_program(&heredoc_program), ": <<EOF\nhello\nEOF");
        let mut shell = test_shell();
        let status = execute_nested_program(&mut shell, &heredoc_program).expect("execute heredoc nested");
        assert_eq!(status, 0);
    }

    #[test]
    fn execute_if_and_loop_commands() {
        let _guard = cwd_lock().lock().expect("cwd lock");
        std::env::set_current_dir(std::env::temp_dir()).expect("set cwd");
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
        let if_program = crate::syntax::parse(
            "if false; then printf no; elif true; then printf yes; else printf bad; fi",
        )
        .expect("parse");
        let status = execute_program(&mut shell, &if_program).expect("execute");
        assert_eq!(status, 0);

        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let marker = std::env::temp_dir().join(format!("meiksh-loop-{unique}.flag"));
        std::fs::write(&marker, "present").expect("seed marker");
        let while_program = crate::syntax::parse(&format!(
            "while test -f {}; do rm {}; FLAG=done; done",
            marker.display(),
            marker.display()
        ))
        .expect("parse");
        let status = execute_program(&mut shell, &while_program).expect("execute");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("FLAG").as_deref(), Some("done"));

        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
        let _ = std::fs::remove_file(&marker);
        let until_program = crate::syntax::parse(&format!(
            "until test -f {}; do touch {}; VALUE=ready; done",
            marker.display(),
            marker.display()
        ))
        .expect("parse");
        let status = execute_program(&mut shell, &until_program).expect("execute");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("ready"));
        let _ = std::fs::remove_file(marker);
    }

    #[test]
    fn execute_for_commands() {
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
    }

    #[test]
    fn execute_case_commands() {
        let mut shell = test_shell();
        let program = crate::syntax::parse(
            "name=beta; case $name in alpha) VALUE=no ;; b*) VALUE=yes ;; esac",
        )
        .expect("parse");
        let status = execute_program(&mut shell, &program).expect("execute");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("yes"));

        let mut shell = test_shell();
        let program = crate::syntax::parse(
            "name=zeta; case $name in alpha|beta) VALUE=hit ;; esac",
        )
        .expect("parse");
        let status = execute_program(&mut shell, &program).expect("execute");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE"), None);
    }

    #[test]
    fn exec_helpers_cover_then_else_and_render_paths() {
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());

        let if_program = crate::syntax::parse("if true; then VALUE=yes; else VALUE=no; fi").expect("parse");
        let status = execute_program(&mut shell, &if_program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("yes"));

        let mut shell = test_shell();
        let if_program = crate::syntax::parse("if false; then VALUE=yes; else VALUE=no; fi").expect("parse");
        let status = execute_program(&mut shell, &if_program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("no"));

        let mut shell = test_shell();
        let if_program =
            crate::syntax::parse("if false; then VALUE=yes; elif false; then VALUE=maybe; else VALUE=no; fi")
                .expect("parse");
        let status = execute_program(&mut shell, &if_program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("no"));

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
                        words: vec![Word { raw: "false".into() }],
                        ..SimpleCommand::default()
                    })],
                },
            )],
        });
        assert!(render.contains("&&"));
    }

    #[test]
    fn spawn_pipeline_covers_compound_command_variants() {
        let mut shell = test_shell();
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
        let children = spawn_pipeline(&mut shell, &pipeline).expect("spawn");
        for child in children.children {
            let _ = child.wait().expect("wait");
        }
    }

    #[test]
    fn exec_render_helpers_cover_remaining_variants() {
        let for_command = ForCommand {
            name: "item".into(),
            items: Some(vec![Word { raw: "a".into() }, Word { raw: "b".into() }]),
            body: Program::default(),
        };
        assert!(render_for(&for_command).contains("in a b"));
        assert!(render_for(&ForCommand { name: "item".into(), items: None, body: Program::default() }).starts_with("for item\n"));

        let simple = SimpleCommand {
            words: vec![Word { raw: "echo".into() }],
            redirections: vec![
                Redirection { fd: None, kind: RedirectionKind::Read, target: Word { raw: "in".into() }, here_doc: None },
                Redirection { fd: None, kind: RedirectionKind::Append, target: Word { raw: "out".into() }, here_doc: None },
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
            word: Word { raw: "$item".into() },
            arms: vec![crate::syntax::CaseArm {
                patterns: vec![Word { raw: "a*".into() }, Word { raw: "b".into() }],
                body: Program::default(),
            }],
        };
        assert!(render_case(&case_command).contains("a* | b)"));
        assert!(render_pipeline(&Pipeline {
            negated: false,
            commands: vec![Command::Case(case_command)],
        })
        .contains("case "));
    }

    #[test]
    fn case_pattern_matching_covers_wildcards_and_classes() {
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
    }

    #[test]
    fn exec_additional_edges_cover_for_render_or_and_exit_breaks() {
        let render = render_and_or(&AndOr {
            first: Pipeline {
                negated: false,
                commands: vec![Command::Simple(SimpleCommand {
                    words: vec![Word { raw: "false".into() }],
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

        let mut shell = test_shell();
        let if_program = crate::syntax::parse("if false; then VALUE=yes; fi").expect("parse");
        let status = execute_program(&mut shell, &if_program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE"), None);

        let mut shell = test_shell();
        let for_program = crate::syntax::parse("for item in a b; do exit 9; done").expect("parse");
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
        let program = crate::syntax::parse("greet() { RESULT=$X; }; X=ok greet").expect("parse");
        let status = execute_program(&mut shell, &program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("RESULT").as_deref(), Some("ok"));
    }

    #[test]
    fn control_flow_builtins_propagate_across_functions_and_nested_loops() {
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
        let program =
            crate::syntax::parse("for outer in x y; do for inner in a b; do break 2; done; VALUE=bad; done")
                .expect("parse");
        let status = execute_program(&mut shell, &program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE"), None);
        assert_eq!(shell.get_var("outer").as_deref(), Some("x"));

        let mut shell = test_shell();
        let program = crate::syntax::parse("f() { while true; do return 4; done; }; f").expect("parse");
        let status = execute_program(&mut shell, &program).expect("exec");
        assert_eq!(status, 4);
        assert_eq!(shell.pending_control, None);

        let mut shell = test_shell();
        let program = crate::syntax::parse("g() { break; }; g").expect("parse");
        let error = execute_program(&mut shell, &program).expect_err("function error");
        assert_eq!(error.message, "break: only meaningful in a loop");

        let mut shell = test_shell();
        let program = crate::syntax::parse("for outer in x; do while break 2; do printf no; done; AFTER=bad; done")
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

        let mut shell = test_shell();
        let program = crate::syntax::parse(
            "f() { while return 3; do printf no; done; }; f",
        )
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
        let program = crate::syntax::parse("f() { for item in a; do return 5; done; }; f").expect("parse");
        let status = execute_program(&mut shell, &program).expect("exec");
        assert_eq!(status, 5);

        let mut shell = test_shell();
        let program = crate::syntax::parse("for outer in x; do for inner in y; do break 2; done; DONE=no; done")
            .expect("parse");
        let status = execute_program(&mut shell, &program).expect("exec");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("DONE"), None);

        let mut shell = test_shell();
        let program =
            crate::syntax::parse("for outer in x; do for inner in y; do continue 2; done; DONE=no; done")
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
    }

    #[test]
    fn shell_redirection_helpers_cover_current_shell_paths() {
        fn fake_dup(fd: i32) -> i32 {
            fd + 50
        }
        fn fake_dup2(_oldfd: i32, newfd: i32) -> i32 {
            newfd
        }
        fn fake_close(_fd: i32) -> i32 {
            0
        }
        fn fake_dup_error(_fd: i32) -> i32 {
            set_errno(22);
            -1
        }
        fn fake_close_error(_fd: i32) -> i32 {
            set_errno(22);
            -1
        }

        sys::test_support::VfsBuilder::new()
            .dir("/redir")
            .file("/redir/input.txt", b"input")
            .file("/redir/noclobber.txt", b"old")
            .run_with_fd_ops(fake_dup, fake_dup2, fake_close, || {
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

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::HereDoc,
                        target: "EOF".into(),
                        here_doc_body: Some("body\n".into()),
                    },
                    false,
                )
                .expect("heredoc redirection");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::DupOutput,
                        target: "1".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect("dup output");

                apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::DupOutput,
                        target: "-".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect("close dup output");

                let error = apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::DupOutput,
                        target: "bad".into(),
                        here_doc_body: None,
                    },
                    false,
                )
                .expect_err("bad dup output");
                assert_eq!(error.message, "redirection target must be a file descriptor or '-'");

                let error = apply_shell_redirection(
                    &ExpandedRedirection {
                        fd: target_fd,
                        kind: RedirectionKind::Write,
                        target: "/redir/noclobber.txt".into(),
                        here_doc_body: None,
                    },
                    true,
                )
                .expect_err("noclobber");
                assert!(!error.message.is_empty());
            });

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

        sys::test_support::with_fd_ops_for_test(fake_dup, fake_dup2, fake_close, || {
            drop(ShellRedirectionGuard {
                saved: vec![(99, None)],
            });
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

        sys::test_support::with_fd_ops_for_test(fake_dup_error, fake_dup2, fake_close, || {
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
        });

        let error = apply_child_fd_actions(&[ChildFdAction::DupFd {
            source_fd: -1,
            target_fd: 56,
        }])
        .expect_err("child dup failure");
        assert!(!error.to_string().is_empty());

        sys::test_support::with_fd_ops_for_test(fake_dup, fake_dup2, fake_close_error, || {
            let error = apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 56 }])
                .expect_err("child close failure");
            assert!(!error.to_string().is_empty());

            let error = close_shell_fd(57).expect_err("close failure");
            assert!(!error.message.is_empty());
        });

    }

    #[test]
    fn prepared_process_helpers_cover_fallback_and_pre_exec_paths() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let script = std::env::temp_dir().join(format!("meiksh-fallback-{unique}.sh"));
        fs::write(&script, "exit 0\n").expect("write fallback script");
        let mut perms = fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).expect("chmod");

        let prepared = PreparedProcess {
            exec_path: script.display().to_string(),
            argv: vec![script.display().to_string()],
            child_env: Vec::new(),
            redirections: Vec::new(),
            noclobber: false,
        };
        let child = spawn_prepared(&prepared, None, false, ProcessGroupPlan::NewGroup).expect("spawn fallback");
        assert!(child.wait_with_output().expect("wait output").status.success());
        let devnull_fd = sys::open_file("/dev/null", sys::O_RDONLY, 0).expect("open /dev/null");
        let child = spawn_with_fallback(&prepared, Some(devnull_fd), false, ProcessGroupPlan::None)
            .expect("fallback spawn");
        assert!(child.wait_with_output().expect("wait output").status.success());
        let devnull_fd2 = sys::open_file("/dev/null", sys::O_RDONLY, 0).expect("open /dev/null");
        let child = maybe_spawn_with_fallback(
            std::io::Error::from_raw_os_error(8),
            &prepared,
            Some(devnull_fd2),
            false,
            ProcessGroupPlan::None,
        )
        .expect("enoexec fallback helper");
        assert!(child.wait_with_output().expect("wait output").status.success());

        let prepared = PreparedProcess {
            exec_path: meiksh_bin_path().display().to_string(),
            argv: vec![meiksh_bin_path().display().to_string(), "-c".into(), "exit 0".into()],
            child_env: Vec::new(),
            redirections: vec![ExpandedRedirection {
                fd: 1,
                kind: RedirectionKind::DupOutput,
                target: "-".into(),
                here_doc_body: None,
            }],
            noclobber: false,
        };
        let child = spawn_prepared(&prepared, None, false, ProcessGroupPlan::None)
            .expect("spawn with stdout redirect");
        assert!(child.wait().expect("wait").success());

        let prepared = PreparedProcess {
            exec_path: meiksh_bin_path().display().to_string(),
            argv: vec![meiksh_bin_path().display().to_string(), "-c".into(), "exit 0".into()],
            child_env: Vec::new(),
            redirections: Vec::new(),
            noclobber: false,
        };
        let child = spawn_prepared(&prepared, None, false, ProcessGroupPlan::NewGroup)
            .expect("spawn newgroup");
        assert!(child.wait().expect("wait").success());
        let child = spawn_prepared(&prepared, None, false, ProcessGroupPlan::Join(0))
            .expect("spawn join");
        assert!(child.wait().expect("wait").success());

        fn fake_isatty(_fd: i32) -> i32 {
            1
        }
        fn fake_tcgetpgrp(_fd: i32) -> sys::Pid {
            55
        }
        fn fake_tcsetpgrp(_fd: i32, _pgid: sys::Pid) -> i32 {
            0
        }
        fn fake_setpgid(_pid: sys::Pid, _pgid: sys::Pid) -> i32 {
            0
        }
        fn fake_kill(_pid: sys::Pid, _sig: i32) -> i32 {
            0
        }
        assert_eq!(fake_kill(1, 0), 0);
        sys::test_support::with_job_control_syscalls_for_test(
            fake_isatty,
            fake_tcgetpgrp,
            fake_tcsetpgrp,
            fake_setpgid,
            fake_kill,
            || {
                assert_eq!(handoff_foreground(Some(77)), Some(55));
                assert_eq!(handoff_foreground(Some(77)), Some(55));
                restore_foreground(Some(55));
                assert_eq!(handoff_foreground(None), None);
            },
        );
        sys::test_support::with_job_control_syscalls_for_test(
            fake_isatty,
            |_fd| -1,
            fake_tcsetpgrp,
            fake_setpgid,
            fake_kill,
            || assert_eq!(handoff_foreground(Some(77)), None),
        );
        sys::test_support::with_job_control_syscalls_for_test(
            fake_isatty,
            fake_tcgetpgrp,
            fake_tcsetpgrp,
            fake_setpgid,
            fake_kill,
            || {
                apply_child_setup(&[], ProcessGroupPlan::None).expect("setup none");
                apply_child_setup(&[], ProcessGroupPlan::NewGroup).expect("setup newgroup");
                apply_child_setup(&[], ProcessGroupPlan::Join(0)).expect("setup join");
            },
        );

        let _ = fs::remove_file(script);
    }

}
