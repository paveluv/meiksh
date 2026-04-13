use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::shell::{Shell, ShellError, TrapAction, TrapCondition};
use crate::sys;

fn write_stderr(msg: &str) {
    let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
}

fn diag_status(shell: &Shell, status: i32, msg: impl std::fmt::Display) -> BuiltinOutcome {
    shell.diagnostic(status, msg);
    BuiltinOutcome::Status(status)
}

#[derive(Debug)]
pub enum BuiltinOutcome {
    Status(i32),
    UtilityError(i32),
    Exit(i32),
    Return(i32),
    Break(usize),
    Continue(usize),
}

pub fn run(
    shell: &mut Shell,
    argv: &[String],
    cmd_assignments: &[(String, String)],
) -> Result<BuiltinOutcome, ShellError> {
    if argv.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }

    let outcome = match argv[0].as_str() {
        ":" | "true" => BuiltinOutcome::Status(0),
        "false" => BuiltinOutcome::Status(1),
        "cd" => cd(shell, argv)?,
        "pwd" => pwd(shell, argv)?,
        "exit" => exit(shell, argv)?,
        "export" => export(shell, argv)?,
        "readonly" => readonly(shell, argv)?,
        "unset" => unset(shell, argv)?,
        "set" => set(shell, argv),
        "shift" => shift(shell, argv)?,
        "eval" => eval(shell, argv)?,
        "." => dot(shell, argv)?,
        "exec" => exec_builtin(shell, argv, cmd_assignments)?,
        "jobs" => jobs(shell, argv),
        "fg" => fg(shell, argv)?,
        "bg" => bg(shell, argv)?,
        "wait" => wait(shell, argv)?,
        "kill" => kill(shell, argv)?,
        "read" => read(shell, argv)?,
        "getopts" => getopts(shell, argv)?,
        "alias" => alias(shell, argv)?,
        "unalias" => unalias(shell, argv)?,
        "return" => return_builtin(shell, argv)?,
        "break" => break_builtin(shell, argv)?,
        "continue" => continue_builtin(shell, argv)?,
        "times" => times(shell),
        "trap" => trap(shell, argv),
        "umask" => umask(shell, argv)?,
        "command" => command(shell, argv)?,
        _ => BuiltinOutcome::Status(127),
    };

    Ok(outcome)
}

const BUILTIN_NAMES: &[&str] = &[
    ".", ":", "alias", "bg", "break", "cd", "command", "continue", "eval", "exec", "exit",
    "export", "false", "fg", "getopts", "jobs", "kill", "pwd", "read", "readonly", "return", "set",
    "shift", "times", "trap", "true", "umask", "unalias", "unset", "wait",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.binary_search(&name).is_ok()
}

const DEFAULT_COMMAND_PATH: &str = "/usr/bin:/bin";

fn cd(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (target, print_new_pwd, physical, check_pwd) = parse_cd_target(shell, argv)?;
    let (resolved_target, _, print_new_pwd) = resolve_cd_target(shell, &target, print_new_pwd);

    let curpath = if physical {
        resolved_target.display().to_string()
    } else {
        cd_logical_curpath(shell, &resolved_target.display().to_string())?
    };

    let old_pwd = current_logical_pwd(shell)?;
    sys::change_dir(&curpath).map_err(|e| shell.diagnostic(1, &e))?;

    let new_pwd = if physical {
        match sys::get_cwd() {
            Ok(cwd) => cwd,
            Err(_) if check_pwd => {
                shell
                    .set_var("OLDPWD", old_pwd)
                    .map_err(|e| shell.diagnostic(1, format_args!("cd: {e}")))?;
                return Ok(BuiltinOutcome::Status(1));
            }
            Err(_) => curpath.clone(),
        }
    } else {
        curpath.clone()
    };

    shell
        .set_var("OLDPWD", old_pwd)
        .map_err(|e| shell.diagnostic(1, format_args!("cd: {e}")))?;
    shell
        .set_var("PWD", new_pwd.clone())
        .map_err(|e| shell.diagnostic(1, format_args!("cd: {e}")))?;
    if print_new_pwd {
        sys_println!("{new_pwd}");
    }
    Ok(BuiltinOutcome::Status(0))
}

fn cd_logical_curpath(shell: &Shell, target: &str) -> Result<String, ShellError> {
    let curpath = if target.starts_with('/') {
        target.to_string()
    } else {
        let pwd = current_logical_pwd(shell)?;
        if pwd.ends_with('/') {
            format!("{pwd}{target}")
        } else {
            format!("{pwd}/{target}")
        }
    };
    Ok(canonicalize_logical_path(&curpath))
}

fn canonicalize_logical_path(path: &str) -> String {
    let mut components: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return "/".to_string();
    }
    let mut result = String::new();
    for component in &components {
        result.push('/');
        result.push_str(component);
    }
    result
}

fn parse_cd_target(
    shell: &Shell,
    argv: &[String],
) -> Result<(String, bool, bool, bool), ShellError> {
    let mut index = 1usize;
    let mut physical = false;
    let mut check_pwd = false;
    while let Some(arg) = argv.get(index) {
        if arg == "--" {
            index += 1;
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            break;
        }
        for ch in arg[1..].chars() {
            match ch {
                'L' => {
                    physical = false;
                    check_pwd = false;
                }
                'P' => physical = true,
                'e' => check_pwd = true,
                _ => {
                    return Err(shell.diagnostic(1, format_args!("cd: invalid option: -{ch}")));
                }
            }
        }
        index += 1;
    }
    if !physical {
        check_pwd = false;
    }
    if argv.len() > index + 1 {
        return Err(shell.diagnostic(1, "cd: too many arguments"));
    }
    let Some(target) = argv.get(index) else {
        return Ok((
            shell.get_var("HOME").unwrap_or(".").to_string(),
            false,
            physical,
            check_pwd,
        ));
    };
    if target.is_empty() {
        return Err(shell.diagnostic(1, "cd: empty directory"));
    }
    if target == "-" {
        return Ok((
            shell
                .get_var("OLDPWD")
                .ok_or_else(|| shell.diagnostic(1, "cd: OLDPWD not set"))?
                .to_string(),
            true,
            physical,
            check_pwd,
        ));
    }
    Ok((target.clone(), false, physical, check_pwd))
}

fn resolve_cd_target(shell: &Shell, target: &str, print_new_pwd: bool) -> (PathBuf, String, bool) {
    if print_new_pwd || target.starts_with('/') {
        return (PathBuf::from(target), target.to_string(), print_new_pwd);
    }
    let first_component = target.split('/').next().unwrap_or("");
    if first_component == "." || first_component == ".." {
        return (PathBuf::from(target), target.to_string(), print_new_pwd);
    }

    let Some(cdpath) = shell.get_var("CDPATH") else {
        return (PathBuf::from(target), target.to_string(), print_new_pwd);
    };

    for prefix in cdpath.split(':') {
        let base = if prefix.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(prefix)
        };
        let candidate = base.join(target);
        if sys::is_directory(&candidate.display().to_string()) {
            let should_print = print_new_pwd || !prefix.is_empty();
            let pwd_target = if prefix.is_empty() {
                target.to_string()
            } else {
                candidate.display().to_string()
            };
            return (candidate, pwd_target, should_print);
        }
    }

    (PathBuf::from(target), target.to_string(), print_new_pwd)
}

fn pwd(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let mut logical = true;
    for arg in &argv[1..] {
        match arg.as_str() {
            "-L" => logical = true,
            "-P" => logical = false,
            _ if arg.starts_with('-') => {
                return Ok(diag_status(
                    shell,
                    1,
                    format_args!("pwd: invalid option: {arg}"),
                ));
            }
            _ => {
                return Ok(diag_status(shell, 1, "pwd: too many arguments"));
            }
        }
    }

    sys_println!("{}", pwd_output(shell, logical)?);
    Ok(BuiltinOutcome::Status(0))
}

fn exit(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let status = argv
        .get(1)
        .map(|value| value.parse::<i32>())
        .transpose()
        .map_err(|_| shell.diagnostic(2, "exit: numeric argument required"))?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Exit(status))
}

fn expand_assignment_tilde(shell: &Shell, value: &str) -> String {
    if !value.starts_with('~') {
        return value.to_string();
    }
    let slash_pos = value.find('/');
    let prefix_end = slash_pos.unwrap_or(value.len());
    let user = &value[1..prefix_end];
    let replacement = if user.is_empty() {
        shell.get_var("HOME").unwrap_or("~").to_string()
    } else {
        match sys::home_dir_for_user(user) {
            Some(dir) => dir,
            None => return value.to_string(),
        }
    };
    let suffix = &value[prefix_end..];
    format!("{replacement}{suffix}")
}

fn export(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, "export", argv)?;
    if print || index == argv.len() {
        for line in exported_lines(shell) {
            sys_println!("{line}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once('=') {
            let value = expand_assignment_tilde(shell, value);
            shell.export_var(name, Some(value))?;
        } else {
            shell.export_var(item, None)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn readonly(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, "readonly", argv)?;
    if print || index == argv.len() {
        for line in readonly_lines(shell) {
            sys_println!("{line}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once('=') {
            let value = expand_assignment_tilde(shell, value);
            shell
                .set_var(name, value)
                .map_err(|e| shell.diagnostic(1, format_args!("readonly: {e}")))?;
            shell.mark_readonly(name);
        } else {
            shell.mark_readonly(item);
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn unset(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (target, index) = parse_unset_target(shell, argv)?;
    let mut status = 0;
    for item in &argv[index..] {
        match target {
            UnsetTarget::Variable => {
                if let Err(error) = shell.unset_var(item) {
                    shell.diagnostic(1, format_args!("unset: {error}"));
                    status = 1;
                }
            }
            UnsetTarget::Function => {
                shell.functions.remove(item);
            }
        }
    }
    if status != 0 {
        Ok(BuiltinOutcome::UtilityError(status))
    } else {
        Ok(BuiltinOutcome::Status(status))
    }
}

fn set(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.env.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            sys_println!("{name}={}", shell_quote_if_needed(value));
        }
    } else {
        let mut index = 1usize;
        while let Some(arg) = argv.get(index) {
            match arg.as_str() {
                "-o" | "+o" => {
                    let reinput = arg == "+o";
                    if let Some(name) = argv.get(index + 1) {
                        if let Err(e) = shell.options.set_named_option(name, !reinput) {
                            return BuiltinOutcome::UtilityError(
                                shell.diagnostic(2, format_args!("set: {e}")).exit_status(),
                            );
                        }
                        index += 2;
                    } else {
                        for (name, enabled) in shell.options.reportable_options() {
                            if reinput {
                                let prefix = if enabled { '-' } else { '+' };
                                sys_println!("set {prefix}o {name}");
                            } else {
                                sys_println!("{name} {}", if enabled { "on" } else { "off" });
                            }
                        }
                        return BuiltinOutcome::Status(0);
                    }
                }
                "--" => {
                    shell.set_positional(argv[index + 1..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
                _ if (arg.starts_with('-') || arg.starts_with('+')) && arg != "-" && arg != "+" => {
                    let enabled = arg.starts_with('-');
                    for ch in arg[1..].chars() {
                        if let Err(e) = shell.options.set_short_option(ch, enabled) {
                            return BuiltinOutcome::UtilityError(
                                shell.diagnostic(2, format_args!("set: {e}")).exit_status(),
                            );
                        }
                    }
                    index += 1;
                }
                _ => {
                    shell.set_positional(argv[index..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
            }
        }
    }
    BuiltinOutcome::Status(0)
}

fn shift(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let count = argv
        .get(1)
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| shell.diagnostic(1, "shift: numeric argument required"))?
        .unwrap_or(1);
    if count > shell.positional.len() {
        return Ok(diag_status(
            shell,
            1,
            format_args!("shift: {count}: shift count out of range"),
        ));
    }
    shell.positional.drain(0..count);
    Ok(BuiltinOutcome::Status(0))
}

fn eval(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let source = argv[1..].join(" ");
    let status = shell.execute_string(&source)?;
    Ok(BuiltinOutcome::Status(status))
}

fn dot(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let path = argv
        .get(1)
        .ok_or_else(|| shell.diagnostic(2, ".: filename argument required"))?;
    if argv.len() > 2 {
        return Err(shell.diagnostic(2, ".: too many arguments"));
    }
    let resolved = match resolve_dot_path(shell, path) {
        Ok(p) => p,
        Err(_) => {
            return Err(shell.diagnostic(1, format_args!(".: {path}: not found")));
        }
    };
    let status = shell.source_path(&resolved)?;
    Ok(BuiltinOutcome::Status(status))
}

fn resolve_dot_path(shell: &Shell, path: &str) -> Result<PathBuf, ()> {
    if path.contains('/') {
        let candidate = PathBuf::from(path);
        if readable_regular_file(&candidate) {
            return Ok(candidate);
        }
        return Err(());
    }
    search_path(path, shell, false, readable_regular_file).ok_or(())
}

fn exec_builtin(
    shell: &Shell,
    argv: &[String],
    cmd_assignments: &[(String, String)],
) -> Result<BuiltinOutcome, ShellError> {
    let args = if argv.get(1).map(|s| s.as_str()) == Some("--") {
        &argv[2..]
    } else {
        &argv[1..]
    };
    if args.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }
    if args.iter().any(|s| s.contains('\0')) {
        return Err(shell.diagnostic(1, "exec: invalid argument"));
    }
    let Some(program_path) = which_in_path(&args[0], shell, true) else {
        return Err(shell.diagnostic(127, format_args!("exec: {}: not found", args[0])));
    };
    let program = program_path.display().to_string();
    let argv_owned: Vec<String> = args.iter().cloned().collect();
    let env = shell.env_for_exec_utility(cmd_assignments);
    sys::exec_replace_with_env(&program, &argv_owned, &env).map_err(|e| shell.diagnostic(1, &e))?;
    Ok(BuiltinOutcome::Status(0))
}

fn return_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.function_depth == 0 && shell.source_depth == 0 {
        return Err(shell.diagnostic(1, "return: not in a function"));
    }
    if argv.len() > 2 {
        return Err(shell.diagnostic(1, "return: too many arguments"));
    }
    let status = argv
        .get(1)
        .map(|value| value.parse::<i32>())
        .transpose()
        .map_err(|_| shell.diagnostic(1, "return: numeric argument required"))?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Return(status))
}

fn break_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, "break: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, "break", argv)?;
    Ok(BuiltinOutcome::Break(levels.min(shell.loop_depth)))
}

fn continue_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, "continue: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, "continue", argv)?;
    Ok(BuiltinOutcome::Continue(levels.min(shell.loop_depth)))
}

fn parse_loop_count(shell: &Shell, name: &str, argv: &[String]) -> Result<usize, ShellError> {
    if argv.len() > 2 {
        return Err(shell.diagnostic(1, format_args!("{name}: too many arguments")));
    }
    let levels = argv
        .get(1)
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| shell.diagnostic(1, format_args!("{name}: numeric argument required")))?
        .unwrap_or(1);
    if levels == 0 {
        return Err(shell.diagnostic(1, format_args!("{name}: numeric argument required")));
    }
    Ok(levels)
}

fn jobs(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    let (mode, index) = match parse_jobs_options(argv) {
        Ok(value) => value,
        Err(message) => return diag_status(shell, 1, message),
    };
    let selected = match parse_jobs_operands(&argv[index..], shell) {
        Ok(value) => value,
        Err(message) => return diag_status(shell, 1, message),
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
                    let state_str = if *status == 0 {
                        "Done".to_string()
                    } else {
                        format!("Done({status})")
                    };
                    sys_println!("[{id}] {marker} {state_str}\t{cmd}");
                }
                crate::shell::ReapedJobState::Signaled(sig, cmd) => {
                    sys_println!(
                        "[{id}] {marker} Terminated ({})\t{cmd}",
                        sys::signal_name(*sig)
                    );
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
                    sys_println!("{pid}");
                }
            }
            _ => {
                let marker = job_current_marker(job.id, current_id, previous_id);
                let (state_str, pid_field) = format_job_state(job);
                if mode == JobsMode::Long {
                    sys_println!(
                        "[{}] {marker} {} {state_str} {}",
                        job.id,
                        pid_field,
                        job.command
                    );
                } else {
                    sys_println!("[{}] {marker} {state_str} {}", job.id, job.command);
                }
            }
        }
    }
    BuiltinOutcome::Status(0)
}

fn job_current_marker(id: usize, current: Option<usize>, previous: Option<usize>) -> char {
    if Some(id) == current {
        '+'
    } else if Some(id) == previous {
        '-'
    } else {
        ' '
    }
}

fn format_job_state(job: &crate::shell::Job) -> (String, String) {
    let pid_str = job_display_pid(job)
        .map(|p| p.to_string())
        .unwrap_or_default();
    let state = match job.state {
        crate::shell::JobState::Running => "Running".to_string(),
        crate::shell::JobState::Stopped(sig) => {
            format!("Stopped ({})", sys::signal_name(sig))
        }
        crate::shell::JobState::Done(status) => {
            if status == 0 {
                "Done".to_string()
            } else {
                format!("Done({status})")
            }
        }
    };
    (state, pid_str)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobsMode {
    Normal,
    Long,
    PidOnly,
}

fn parse_jobs_options(argv: &[String]) -> Result<(JobsMode, usize), String> {
    let mut mode = JobsMode::Normal;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        if !arg.starts_with('-') || arg == "-" {
            break;
        }
        if arg == "--" {
            index += 1;
            break;
        }
        match arg.as_str() {
            "-p" => mode = JobsMode::PidOnly,
            "-l" => mode = JobsMode::Long,
            _ => return Err(format!("jobs: invalid option: {arg}")),
        }
        index += 1;
    }
    Ok((mode, index))
}

fn parse_jobs_operands(operands: &[String], shell: &Shell) -> Result<Option<Vec<usize>>, String> {
    if operands.is_empty() {
        return Ok(None);
    }
    let mut ids = Vec::new();
    for operand in operands {
        let Some(id) = resolve_job_id(shell, Some(operand.as_str())) else {
            return Err(format!("jobs: invalid job id: {operand}"));
        };
        ids.push(id);
    }
    Ok(Some(ids))
}

fn job_display_pid(job: &crate::shell::Job) -> Option<sys::Pid> {
    job.pgid
        .or_else(|| job.children.first().map(|child| child.pid))
        .or_else(|| job.last_pid)
}

fn fg(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if !shell.options.monitor {
        return Ok(diag_status(shell, 1, "fg: no job control"));
    }
    let id = resolve_job_id(shell, argv.get(1).map(String::as_str))
        .or_else(|| shell.current_job_id())
        .ok_or_else(|| shell.diagnostic(1, "fg: no current job"))?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        sys_println!("{}", job.command);
    }
    shell.continue_job(id, true)?;
    let status = shell.wait_for_job(id)?;
    Ok(BuiltinOutcome::Status(status))
}

fn bg(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if !shell.options.monitor {
        return Ok(diag_status(shell, 1, "bg: no job control"));
    }
    let id = resolve_job_id(shell, argv.get(1).map(String::as_str))
        .or_else(|| {
            shell
                .jobs
                .iter()
                .rev()
                .find(|j| matches!(j.state, crate::shell::JobState::Stopped(_)))
                .map(|j| j.id)
        })
        .ok_or_else(|| shell.diagnostic(1, "bg: no current job"))?;
    shell.continue_job(id, false)?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        sys_println!("[{id}] {}", job.command);
    }
    Ok(BuiltinOutcome::Status(0))
}

fn wait(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        return Ok(BuiltinOutcome::Status(shell.wait_for_all_jobs()?));
    }
    let mut status = 0;
    for operand in &argv[1..] {
        status = match parse_wait_operand(operand, shell) {
            Ok(WaitOperand::Job(id)) => shell.wait_for_job_operand(id)?,
            Ok(WaitOperand::Pid(pid)) => shell.wait_for_pid_operand(pid)?,
            Err(message) => {
                shell.diagnostic(1, message);
                1
            }
        };
    }
    Ok(BuiltinOutcome::Status(status))
}

fn kill(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Ok(diag_status(
            shell,
            2,
            "kill: usage: kill [-s sigspec | -signum] pid... | -l [exit_status]",
        ));
    }

    let mut args = &argv[1..];
    if args[0] == "-l" || args[0] == "-L" {
        if args.len() == 1 {
            let names: Vec<&str> = sys::all_signal_names()
                .iter()
                .map(|(name, _)| *name)
                .collect();
            sys_println!("{}", names.join(" "));
            return Ok(BuiltinOutcome::Status(0));
        }
        for arg in &args[1..] {
            if let Ok(code) = arg.parse::<i32>() {
                let sig = if code > 128 { code - 128 } else { code };
                let name = sys::signal_name(sig);
                if name != "UNKNOWN" {
                    sys_println!("{}", &name[3..]);
                } else {
                    return Ok(diag_status(
                        shell,
                        1,
                        format_args!("kill: unknown signal: {arg}"),
                    ));
                }
            } else {
                return Ok(diag_status(
                    shell,
                    1,
                    format_args!("kill: invalid exit status: {arg}"),
                ));
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let mut signal = sys::SIGTERM;
    if args[0] == "-s" {
        if args.len() < 3 {
            return Ok(diag_status(shell, 2, "kill: -s requires a signal name"));
        }
        signal = parse_kill_signal(shell, &args[1])?;
        args = &args[2..];
    } else if args[0].starts_with('-') && args[0] != "--" {
        let spec = &args[0][1..];
        signal = parse_kill_signal(shell, spec)?;
        args = &args[1..];
    }

    if args.is_empty() || (args.len() == 1 && args[0] == "--") {
        return Ok(diag_status(shell, 2, "kill: no process id specified"));
    }
    if args[0] == "--" {
        args = &args[1..];
    }

    let mut status = 0;
    for operand in args {
        if operand.starts_with('%') {
            let resolved_id = resolve_job_id(shell, Some(operand));
            let resolved = resolved_id.and_then(|id| shell.jobs.iter().find(|j| j.id == id));
            if let Some(job) = resolved {
                let pid = job
                    .pgid
                    .unwrap_or_else(|| job.children.first().map(|c| c.pid).unwrap_or(0));
                if pid != 0 {
                    if sys::send_signal(-pid, signal).is_err() {
                        shell.diagnostic(1, format_args!("kill: ({pid}): No such process"));
                        status = 1;
                    } else if signal == sys::SIGCONT && resolved_id.is_some() {
                        let id = resolved_id.unwrap();
                        if let Some(j) = shell.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = crate::shell::JobState::Running;
                        }
                    }
                }
            } else {
                shell.diagnostic(1, format_args!("kill: {operand}: no such job"));
                status = 1;
            }
        } else if let Ok(pid) = operand.parse::<sys::Pid>() {
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
                shell.diagnostic(1, format_args!("kill: ({pid}): No such process"));
                status = 1;
            }
        } else {
            shell.diagnostic(1, format_args!("kill: invalid pid: {operand}"));
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn parse_kill_signal(shell: &Shell, spec: &str) -> Result<i32, ShellError> {
    if let Ok(num) = spec.parse::<i32>() {
        return Ok(num);
    }
    let upper = spec.to_uppercase();
    let name = upper.strip_prefix("SIG").unwrap_or(&upper);
    for (n, sig) in sys::all_signal_names() {
        if *n == name {
            return Ok(*sig);
        }
    }
    if name == "0" {
        return Ok(0);
    }
    Err(shell.diagnostic(1, format_args!("kill: unknown signal: {spec}")))
}

#[derive(Clone, Copy)]
struct ReadOptions {
    raw: bool,
    delimiter: u8,
}

fn read(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| shell.diagnostic(1, &e))?;
    read_with_input(shell, argv, sys::STDIN_FILENO)
}

fn read_with_input(
    shell: &mut Shell,
    argv: &[String],
    input_fd: i32,
) -> Result<BuiltinOutcome, ShellError> {
    let Some((options, vars)) = parse_read_options(argv) else {
        return Ok(diag_status(shell, 2, "read: invalid usage"));
    };
    let vars = if vars.is_empty() {
        vec!["REPLY".to_string()]
    } else {
        vars
    };

    let (pieces, hit_delimiter) = match read_logical_line(shell, options, input_fd) {
        Ok(result) => result,
        Err(error) => {
            return Ok(diag_status(shell, 2, format_args!("read: {error}")));
        }
    };
    let values =
        split_read_assignments(&pieces, &vars, shell.get_var("IFS").map(|s| s.to_string()));
    for (name, value) in vars.iter().zip(values) {
        if let Err(error) = shell.set_var(name, value) {
            return Ok(diag_status(shell, 2, format_args!("read: {error}")));
        }
    }
    Ok(BuiltinOutcome::Status(if hit_delimiter { 0 } else { 1 }))
}

fn parse_read_options(argv: &[String]) -> Option<(ReadOptions, Vec<String>)> {
    let mut options = ReadOptions {
        raw: false,
        delimiter: b'\n',
    };
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_str() {
            "--" => {
                index += 1;
                break;
            }
            "-r" => {
                options.raw = true;
                index += 1;
            }
            "-d" => {
                let delim = argv.get(index + 1)?;
                options.delimiter = if delim.is_empty() {
                    0
                } else if delim.len() == 1 {
                    delim.as_bytes()[0]
                } else {
                    return None;
                };
                index += 2;
            }
            _ if arg.starts_with('-') && arg != "-" => return None,
            _ => break,
        }
    }
    Some((options, argv[index..].to_vec()))
}

fn read_logical_line(
    shell: &Shell,
    options: ReadOptions,
    input_fd: i32,
) -> sys::SysResult<(Vec<(String, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_quoted = false;

    loop {
        let mut byte = [0u8; 1];
        let count = sys::read_fd(input_fd, &mut byte)?;
        if count == 0 {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, false));
        }
        let ch = byte[0];
        if !options.raw && ch == b'\\' {
            let count = sys::read_fd(input_fd, &mut byte)?;
            if count == 0 {
                current.push('\\');
                push_read_piece(&mut pieces, &mut current, current_quoted);
                return Ok((pieces, false));
            }
            let escaped = byte[0];
            if escaped == b'\n' || escaped == options.delimiter {
                push_read_piece(&mut pieces, &mut current, current_quoted);
                current_quoted = false;
                if shell.is_interactive() {
                    let prompt = shell.get_var("PS2").unwrap_or("> ");
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, prompt.as_bytes());
                }
                continue;
            }
            push_read_piece(&mut pieces, &mut current, current_quoted);
            current_quoted = true;
            current.push(escaped as char);
            continue;
        }
        if ch == options.delimiter {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, true));
        }
        if current_quoted {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            current_quoted = false;
        }
        current.push(ch as char);
    }
}

fn push_read_piece(pieces: &mut Vec<(String, bool)>, current: &mut String, quoted: bool) {
    if current.is_empty() {
        return;
    }
    if let Some((last, last_quoted)) = pieces.last_mut() {
        if *last_quoted == quoted {
            last.push_str(current);
            current.clear();
            return;
        }
    }
    pieces.push((std::mem::take(current), quoted));
}

fn split_read_assignments(
    pieces: &[(String, bool)],
    vars: &[String],
    ifs_value: Option<String>,
) -> Vec<String> {
    if vars.is_empty() {
        return Vec::new();
    }
    let ifs = ifs_value.unwrap_or_else(|| " \t\n".to_string());
    if ifs.is_empty() {
        let mut values = vec![flatten_read_pieces(pieces)];
        values.resize(vars.len(), String::new());
        return values;
    }

    let ifs_ws: Vec<char> = ifs
        .chars()
        .filter(|ch| matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let ifs_other: Vec<char> = ifs
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let chars = flatten_read_chars(pieces);
    if vars.len() == 1 {
        return vec![trim_read_ifs_whitespace(&chars, &ifs_ws)];
    }

    let mut values = Vec::new();
    let mut index = 0usize;
    skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
    while index < chars.len() && values.len() + 1 < vars.len() {
        let mut current = String::new();
        loop {
            if index >= chars.len() {
                values.push(current);
                break;
            }
            let (ch, quoted) = chars[index];
            if !quoted && ifs_other.contains(&ch) {
                values.push(current);
                index += 1;
                skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
                break;
            }
            if !quoted && ifs_ws.contains(&ch) {
                debug_assert!(
                    !current.is_empty(),
                    "leading IFS whitespace should already be skipped"
                );
                values.push(current);
                skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
                break;
            }
            current.push(ch);
            index += 1;
        }
    }

    values.push(trim_read_ifs_whitespace(&chars[index..], &ifs_ws));
    values.resize(vars.len(), String::new());
    values
}

fn flatten_read_pieces(pieces: &[(String, bool)]) -> String {
    pieces.iter().map(|(part, _)| part).cloned().collect()
}

fn flatten_read_chars(pieces: &[(String, bool)]) -> Vec<(char, bool)> {
    let mut chars = Vec::new();
    for (text, quoted) in pieces {
        for ch in text.chars() {
            chars.push((ch, *quoted));
        }
    }
    chars
}

fn skip_read_ifs_whitespace(chars: &[(char, bool)], ifs_ws: &[char], index: &mut usize) {
    while *index < chars.len() && !chars[*index].1 && ifs_ws.contains(&chars[*index].0) {
        *index += 1;
    }
}

fn trim_read_ifs_whitespace(chars: &[(char, bool)], ifs_ws: &[char]) -> String {
    let mut start = 0usize;
    let mut end = chars.len();
    while start < end && !chars[start].1 && ifs_ws.contains(&chars[start].0) {
        start += 1;
    }
    while end > start && !chars[end - 1].1 && ifs_ws.contains(&chars[end - 1].0) {
        end -= 1;
    }
    chars[start..end].iter().map(|(ch, _)| *ch).collect()
}

fn getopts_set(shell: &mut Shell, name: &str, value: String) -> Result<(), BuiltinOutcome> {
    shell
        .set_var(name, value)
        .map_err(|e| diag_status(shell, 2, format_args!("getopts: {e}")))
}

fn getopts(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 3 {
        return Ok(diag_status(
            shell,
            2,
            "getopts: usage: getopts optstring name [arg ...]",
        ));
    }
    let optstring = &argv[1];
    let name = &argv[2];
    let silent = optstring.starts_with(':');
    let opts = if silent {
        &optstring[1..]
    } else {
        optstring.as_str()
    };

    let params: Vec<String> = if argv.len() > 3 {
        argv[3..].to_vec()
    } else {
        shell.positional.clone()
    };

    let optind: usize = shell
        .get_var("OPTIND")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    let charind: usize = shell
        .get_var("_GETOPTS_CIND")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    match getopts_inner(shell, name, opts, silent, &params, optind, charind) {
        Ok(status) => Ok(status),
        Err(outcome) => Ok(outcome),
    }
}

fn getopts_inner(
    shell: &mut Shell,
    name: &str,
    opts: &str,
    silent: bool,
    params: &[String],
    optind: usize,
    charind: usize,
) -> Result<BuiltinOutcome, BuiltinOutcome> {
    if optind < 1 || optind > params.len() {
        getopts_set(shell, name, "?".into())?;
        let _ = shell.unset_var("OPTARG");
        getopts_set(shell, "OPTIND", (params.len() + 1).to_string())?;
        return Ok(BuiltinOutcome::Status(1));
    }

    let arg = &params[optind - 1];
    let arg_chars: Vec<char> = arg.chars().collect();

    if charind == 0 {
        if arg == "--" {
            getopts_set(shell, name, "?".into())?;
            let _ = shell.unset_var("OPTARG");
            getopts_set(shell, "OPTIND", (optind + 1).to_string())?;
            shell.env.remove("_GETOPTS_CIND");
            return Ok(BuiltinOutcome::Status(1));
        }
        if arg_chars.len() < 2 || arg_chars[0] != '-' {
            getopts_set(shell, name, "?".into())?;
            let _ = shell.unset_var("OPTARG");
            getopts_set(shell, "OPTIND", optind.to_string())?;
            shell.env.remove("_GETOPTS_CIND");
            return Ok(BuiltinOutcome::Status(1));
        }
    }

    let ci = if charind == 0 { 1 } else { charind };
    let opt_char = arg_chars[ci];
    let next_ci = ci + 1;

    if let Some(pos) = opts.find(opt_char) {
        let takes_arg = opts.as_bytes().get(pos + 1) == Some(&b':');

        if takes_arg {
            if next_ci < arg_chars.len() {
                let optarg: String = arg_chars[next_ci..].iter().collect();
                getopts_set(shell, "OPTARG", optarg)?;
                getopts_set(shell, "OPTIND", (optind + 1).to_string())?;
                shell.env.remove("_GETOPTS_CIND");
            } else if optind < params.len() {
                let optarg = params[optind].clone();
                getopts_set(shell, "OPTARG", optarg)?;
                getopts_set(shell, "OPTIND", (optind + 2).to_string())?;
                shell.env.remove("_GETOPTS_CIND");
            } else {
                if silent {
                    getopts_set(shell, name, ":".into())?;
                    getopts_set(shell, "OPTARG", opt_char.to_string())?;
                } else {
                    write_stderr(&format!(
                        "{}: option requires an argument -- {}\n",
                        shell.shell_name, opt_char
                    ));
                    getopts_set(shell, name, "?".into())?;
                    let _ = shell.unset_var("OPTARG");
                }
                getopts_set(shell, "OPTIND", (optind + 1).to_string())?;
                shell.env.remove("_GETOPTS_CIND");
                return Ok(BuiltinOutcome::Status(0));
            }
        } else {
            let _ = shell.unset_var("OPTARG");
            if next_ci < arg_chars.len() {
                shell
                    .env
                    .insert("_GETOPTS_CIND".into(), next_ci.to_string());
            } else {
                getopts_set(shell, "OPTIND", (optind + 1).to_string())?;
                shell.env.remove("_GETOPTS_CIND");
            }
        }
        getopts_set(shell, name, opt_char.to_string())?;
        Ok(BuiltinOutcome::Status(0))
    } else {
        if silent {
            getopts_set(shell, "OPTARG", opt_char.to_string())?;
        } else {
            write_stderr(&format!(
                "{}: illegal option -- {}\n",
                shell.shell_name, opt_char
            ));
            let _ = shell.unset_var("OPTARG");
        }
        getopts_set(shell, name, "?".into())?;
        if next_ci < arg_chars.len() {
            shell
                .env
                .insert("_GETOPTS_CIND".into(), next_ci.to_string());
        } else {
            getopts_set(shell, "OPTIND", (optind + 1).to_string())?;
            shell.env.remove("_GETOPTS_CIND");
        }
        Ok(BuiltinOutcome::Status(0))
    }
}

fn alias(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.aliases.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            sys_println!("{}", format_alias_definition(name, value));
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.aliases.insert(name.into(), value.into());
        } else if let Some(value) = shell.aliases.get(item.as_str()) {
            sys_println!("{}", format_alias_definition(item, value));
        } else {
            shell.diagnostic(1, format_args!("alias: {item}: not found"));
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn format_alias_definition(name: &str, value: &str) -> String {
    format!("{name}={}", shell_quote(value))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', r#"'\''"#);
    format!("'{escaped}'")
}

fn needs_quoting(value: &str) -> bool {
    value.is_empty()
        || value.bytes().any(|b| {
            !b.is_ascii_alphanumeric()
                && b != b'_'
                && b != b'/'
                && b != b'.'
                && b != b'-'
                && b != b'+'
                && b != b':'
                && b != b','
        })
}

fn shell_quote_if_needed(value: &str) -> std::borrow::Cow<'_, str> {
    if needs_quoting(value) {
        std::borrow::Cow::Owned(shell_quote(value))
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

fn unalias(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Err(shell.diagnostic(1, "unalias: name required"));
    }
    if argv.len() == 2 && argv[1] == "-a" {
        shell.aliases.clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv[1].starts_with('-') && argv[1] != "-" && argv[1] != "--" {
        return Err(shell.diagnostic(1, format_args!("unalias: invalid option: {}", argv[1])));
    }
    let start = usize::from(argv[1] == "--") + 1;
    if start >= argv.len() {
        return Err(shell.diagnostic(1, "unalias: name required"));
    }
    let mut status = 0;
    for item in &argv[start..] {
        if shell.aliases.remove(item.as_str()).is_none() {
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn times(shell: &Shell) -> BuiltinOutcome {
    match (sys::process_times(), sys::clock_ticks_per_second()) {
        (Ok(times), Ok(ticks_per_second)) => {
            sys_println!(
                "{} {}",
                format_times_value(times.user_ticks, ticks_per_second),
                format_times_value(times.system_ticks, ticks_per_second)
            );
            sys_println!(
                "{} {}",
                format_times_value(times.child_user_ticks, ticks_per_second),
                format_times_value(times.child_system_ticks, ticks_per_second)
            );
            BuiltinOutcome::Status(0)
        }
        (Err(error), _) | (_, Err(error)) => diag_status(shell, 1, format_args!("times: {error}")),
    }
}

fn trap(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    match trap_impl(shell, argv) {
        Ok(status) => BuiltinOutcome::Status(status),
        Err(error) => BuiltinOutcome::Status(error.exit_status()),
    }
}

#[derive(Clone, Copy)]
enum WaitOperand {
    Job(usize),
    Pid(sys::Pid),
}

fn resolve_job_id(shell: &Shell, operand: Option<&str>) -> Option<usize> {
    let operand = operand?;
    let spec = operand.strip_prefix('%').unwrap_or(operand);
    match spec {
        "%" | "+" | "" => shell.current_job_id(),
        "-" => shell.previous_job_id(),
        _ => {
            if let Some(rest) = spec.strip_prefix('?') {
                return shell.find_job_by_substring(rest);
            }
            if let Ok(n) = spec.parse::<usize>() {
                if shell.jobs.iter().any(|j| j.id == n) {
                    return Some(n);
                }
                return None;
            }
            shell.find_job_by_prefix(spec)
        }
    }
}

fn parse_wait_operand(operand: &str, shell: &Shell) -> Result<WaitOperand, String> {
    if operand.starts_with('%') {
        return resolve_job_id(shell, Some(operand))
            .map(WaitOperand::Job)
            .ok_or_else(|| format!("wait: invalid job id: {operand}"));
    }
    operand
        .parse::<sys::Pid>()
        .map(WaitOperand::Pid)
        .map_err(|_| format!("wait: invalid process id: {operand}"))
}

fn trap_impl(shell: &mut Shell, argv: &[String]) -> Result<i32, ShellError> {
    if argv.len() == 1 {
        print_traps(shell, false, &[])?;
        return Ok(0);
    }
    if argv[1] == "-p" {
        print_traps(shell, true, &argv[2..])?;
        return Ok(0);
    }
    let (action_index, conditions_start) = if argv[1] == "--" {
        if argv.len() == 2 {
            print_traps(shell, false, &[])?;
            return Ok(0);
        }
        (2, 3)
    } else {
        (1, 2)
    };
    if is_unsigned_decimal(&argv[action_index]) {
        for condition in &argv[action_index..] {
            if let Some(condition) = parse_trap_condition(condition) {
                shell.set_trap(condition, None)?;
            } else {
                shell.diagnostic(1, format_args!("trap: invalid condition: {condition}"));
                return Ok(1);
            }
        }
        return Ok(0);
    }
    let action = &argv[action_index];
    if argv.len() <= conditions_start {
        return Err(shell.diagnostic(1, "trap: condition argument required"));
    }
    let trap_action = parse_trap_action(action);
    let mut status = 0;
    for condition in &argv[conditions_start..] {
        let Some(condition) = parse_trap_condition(condition) else {
            shell.diagnostic(1, format_args!("trap: invalid condition: {condition}"));
            status = 1;
            continue;
        };
        shell.set_trap(condition, trap_action.clone())?;
    }
    Ok(status)
}

fn print_traps(
    shell: &Shell,
    include_defaults: bool,
    operands: &[String],
) -> Result<(), ShellError> {
    let conditions = if operands.is_empty() {
        if include_defaults {
            supported_trap_conditions()
        } else if let Some(saved) = &shell.subshell_saved_traps {
            let mut keys: BTreeSet<TrapCondition> = shell.trap_actions.keys().copied().collect();
            keys.extend(saved.keys().copied());
            keys.into_iter().collect()
        } else {
            shell.trap_actions.keys().copied().collect()
        }
    } else {
        let mut parsed = Vec::new();
        for operand in operands {
            let Some(condition) = parse_trap_condition(operand) else {
                return Err(shell.diagnostic(1, format_args!("trap: invalid condition: {operand}")));
            };
            parsed.push(condition);
        }
        parsed
    };
    for condition in conditions {
        if let Some(action) =
            trap_output_action(shell, condition, include_defaults, !operands.is_empty())
        {
            sys_println!("trap -- {action} {}", format_trap_condition(condition));
        }
    }
    Ok(())
}

fn supported_trap_conditions() -> Vec<TrapCondition> {
    let mut conditions = vec![TrapCondition::Exit];
    conditions.extend(
        sys::supported_trap_signals()
            .into_iter()
            .map(TrapCondition::Signal),
    );
    conditions
}

fn parse_trap_action(action: &str) -> Option<TrapAction> {
    match action {
        "-" => None,
        _ if action.is_empty() => Some(TrapAction::Ignore),
        _ => Some(TrapAction::Command(action.to_string().into())),
    }
}

fn parse_trap_condition(text: &str) -> Option<TrapCondition> {
    let name = text.strip_prefix("SIG").unwrap_or(text);
    match name {
        "0" | "EXIT" => Some(TrapCondition::Exit),
        "HUP" | "1" => Some(TrapCondition::Signal(sys::SIGHUP)),
        "INT" | "2" => Some(TrapCondition::Signal(sys::SIGINT)),
        "QUIT" | "3" => Some(TrapCondition::Signal(sys::SIGQUIT)),
        "ILL" | "4" => Some(TrapCondition::Signal(sys::SIGILL)),
        "ABRT" | "6" => Some(TrapCondition::Signal(sys::SIGABRT)),
        "FPE" | "8" => Some(TrapCondition::Signal(sys::SIGFPE)),
        "KILL" | "9" => Some(TrapCondition::Signal(sys::SIGKILL)),
        "USR1" | "10" => Some(TrapCondition::Signal(sys::SIGUSR1)),
        "SEGV" | "11" => Some(TrapCondition::Signal(sys::SIGSEGV)),
        "USR2" | "12" => Some(TrapCondition::Signal(sys::SIGUSR2)),
        "PIPE" | "13" => Some(TrapCondition::Signal(sys::SIGPIPE)),
        "ALRM" | "14" => Some(TrapCondition::Signal(sys::SIGALRM)),
        "TERM" | "15" => Some(TrapCondition::Signal(sys::SIGTERM)),
        "CHLD" | "17" => Some(TrapCondition::Signal(sys::SIGCHLD)),
        "STOP" | "19" => Some(TrapCondition::Signal(sys::SIGSTOP)),
        "CONT" | "18" => Some(TrapCondition::Signal(sys::SIGCONT)),
        "TRAP" | "5" => Some(TrapCondition::Signal(sys::SIGTRAP)),
        "TSTP" | "20" => Some(TrapCondition::Signal(sys::SIGTSTP)),
        "TTIN" | "21" => Some(TrapCondition::Signal(sys::SIGTTIN)),
        "TTOU" | "22" => Some(TrapCondition::Signal(sys::SIGTTOU)),
        "BUS" => Some(TrapCondition::Signal(sys::SIGBUS)),
        "SYS" => Some(TrapCondition::Signal(sys::SIGSYS)),
        _ => None,
    }
}

fn format_trap_condition(condition: TrapCondition) -> String {
    match condition {
        TrapCondition::Exit => "EXIT".to_string(),
        TrapCondition::Signal(sys::SIGHUP) => "HUP".to_string(),
        TrapCondition::Signal(sys::SIGINT) => "INT".to_string(),
        TrapCondition::Signal(sys::SIGQUIT) => "QUIT".to_string(),
        TrapCondition::Signal(sys::SIGILL) => "ILL".to_string(),
        TrapCondition::Signal(sys::SIGABRT) => "ABRT".to_string(),
        TrapCondition::Signal(sys::SIGFPE) => "FPE".to_string(),
        TrapCondition::Signal(sys::SIGKILL) => "KILL".to_string(),
        TrapCondition::Signal(sys::SIGUSR1) => "USR1".to_string(),
        TrapCondition::Signal(sys::SIGSEGV) => "SEGV".to_string(),
        TrapCondition::Signal(sys::SIGUSR2) => "USR2".to_string(),
        TrapCondition::Signal(sys::SIGPIPE) => "PIPE".to_string(),
        TrapCondition::Signal(sys::SIGALRM) => "ALRM".to_string(),
        TrapCondition::Signal(sys::SIGTERM) => "TERM".to_string(),
        TrapCondition::Signal(sys::SIGCHLD) => "CHLD".to_string(),
        TrapCondition::Signal(sys::SIGCONT) => "CONT".to_string(),
        TrapCondition::Signal(sys::SIGTRAP) => "TRAP".to_string(),
        TrapCondition::Signal(sys::SIGTSTP) => "TSTP".to_string(),
        TrapCondition::Signal(sys::SIGTTIN) => "TTIN".to_string(),
        TrapCondition::Signal(sys::SIGTTOU) => "TTOU".to_string(),
        TrapCondition::Signal(sys::SIGBUS) => "BUS".to_string(),
        TrapCondition::Signal(sys::SIGSYS) => "SYS".to_string(),
        TrapCondition::Signal(signal) => signal.to_string(),
    }
}

fn trap_output_action(
    shell: &Shell,
    condition: TrapCondition,
    include_defaults: bool,
    explicit_operand: bool,
) -> Option<String> {
    let action = shell
        .subshell_saved_traps
        .as_ref()
        .and_then(|saved| saved.get(&condition))
        .or_else(|| shell.trap_action(condition));
    match action {
        Some(TrapAction::Ignore) => Some("''".to_string()),
        Some(TrapAction::Command(command)) => Some(shell_quote(command)),
        None if include_defaults || explicit_operand => Some("-".to_string()),
        None => None,
    }
}

fn is_unsigned_decimal(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch.is_ascii_digit())
}

fn format_times_value(ticks: u64, ticks_per_second: u64) -> String {
    let total_seconds = ticks as f64 / ticks_per_second as f64;
    let minutes = (total_seconds / 60.0).floor() as u64;
    let seconds = total_seconds - (minutes * 60) as f64;
    format!("{minutes}m{seconds:.2}s")
}

fn umask(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let mut symbolic_output = false;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_str() {
            "-S" => {
                symbolic_output = true;
                index += 1;
            }
            "--" => {
                index += 1;
                break;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Ok(diag_status(
                    shell,
                    1,
                    format_args!("umask: invalid option: {arg}"),
                ));
            }
            _ => break,
        }
    }

    let current = sys::current_umask() as u16;
    if index == argv.len() {
        if symbolic_output {
            sys_println!("{}", format_umask_symbolic(current));
        } else {
            sys_println!("{current:04o}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    if index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, "umask: too many arguments"));
    }

    let Some(mask) = parse_umask_mask(&argv[index], current) else {
        return Ok(diag_status(
            shell,
            1,
            format_args!("umask: invalid mask: {}", argv[index]),
        ));
    };
    sys::set_umask(mask as sys::FileModeMask);
    Ok(BuiltinOutcome::Status(0))
}

fn parse_umask_mask(mask: &str, current_mask: u16) -> Option<u16> {
    if !mask.is_empty() && mask.chars().all(|ch| matches!(ch, '0'..='7')) {
        return u16::from_str_radix(mask, 8).ok().map(|value| value & 0o777);
    }
    parse_symbolic_umask(mask, current_mask)
}

fn parse_symbolic_umask(mask: &str, current_mask: u16) -> Option<u16> {
    let mut allowed = (!current_mask) & 0o777;
    for clause in mask.split(',') {
        if clause.is_empty() {
            return None;
        }
        let (targets, op, perms) = parse_symbolic_clause(clause)?;
        let perm_bits = symbolic_permission_bits(perms, targets, allowed)?;
        if op == '+' {
            allowed |= perm_bits;
        } else if op == '-' {
            allowed &= !perm_bits;
        } else {
            allowed = (allowed & !targets) | (perm_bits & targets);
        }
    }
    Some((!allowed) & 0o777)
}

fn parse_symbolic_clause(clause: &str) -> Option<(u16, char, &str)> {
    let mut split_at = 0usize;
    for ch in clause.chars() {
        if matches!(ch, 'u' | 'g' | 'o' | 'a') {
            split_at += ch.len_utf8();
        } else {
            break;
        }
    }
    let (who_text, rest) = clause.split_at(split_at);
    let mut rest_chars = rest.chars();
    let op = rest_chars.next()?;
    if !matches!(op, '+' | '-' | '=') {
        return None;
    }
    let perms = rest_chars.as_str();
    Some((parse_symbolic_targets(who_text), op, perms))
}

fn parse_symbolic_targets(who_text: &str) -> u16 {
    if who_text.is_empty() {
        return 0o777;
    }
    let mut targets = 0u16;
    for ch in who_text.chars() {
        match ch {
            'u' => targets |= 0o700,
            'g' => targets |= 0o070,
            'o' => targets |= 0o007,
            'a' => targets |= 0o777,
            _ => {}
        }
    }
    targets
}

fn symbolic_permission_bits(perms: &str, targets: u16, allowed: u16) -> Option<u16> {
    let mut bits = 0u16;
    for ch in perms.chars() {
        bits |= match ch {
            'r' => permission_bits_for_targets(targets, 0o444),
            'w' => permission_bits_for_targets(targets, 0o222),
            'x' => permission_bits_for_targets(targets, 0o111),
            'X' => permission_bits_for_targets(targets, 0o111),
            's' => 0,
            'u' => copy_permission_bits(allowed, targets, 0o700),
            'g' => copy_permission_bits(allowed, targets, 0o070),
            'o' => copy_permission_bits(allowed, targets, 0o007),
            _ => return None,
        };
    }
    Some(bits)
}

fn permission_bits_for_targets(targets: u16, mask: u16) -> u16 {
    let mut bits = 0u16;
    if targets & 0o700 != 0 {
        bits |= mask & 0o700;
    }
    if targets & 0o070 != 0 {
        bits |= mask & 0o070;
    }
    if targets & 0o007 != 0 {
        bits |= mask & 0o007;
    }
    bits
}

fn copy_permission_bits(allowed: u16, targets: u16, source_class: u16) -> u16 {
    let source = match source_class {
        0o700 => (allowed & 0o700) >> 6,
        0o070 => (allowed & 0o070) >> 3,
        0o007 => allowed & 0o007,
        _ => 0,
    };
    let mut bits = 0u16;
    if targets & 0o700 != 0 {
        bits |= source << 6;
    }
    if targets & 0o070 != 0 {
        bits |= source << 3;
    }
    if targets & 0o007 != 0 {
        bits |= source;
    }
    bits
}

fn format_umask_symbolic(mask: u16) -> String {
    format!(
        "u={},g={},o={}",
        symbolic_permissions_for_class(mask, 0o700, 6),
        symbolic_permissions_for_class(mask, 0o070, 3),
        symbolic_permissions_for_class(mask, 0o007, 0)
    )
}

fn symbolic_permissions_for_class(mask: u16, class_mask: u16, shift: u16) -> String {
    let allowed = ((!mask) & class_mask) >> shift;
    let mut result = String::new();
    if allowed & 0b100 != 0 {
        result.push('r');
    }
    if allowed & 0b010 != 0 {
        result.push('w');
    }
    if allowed & 0b001 != 0 {
        result.push('x');
    }
    result
}

fn command(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (use_default_path, mode, index) = parse_command_options(argv);
    let Some(name) = argv.get(index) else {
        return Ok(diag_status(
            shell,
            command_usage_status(mode),
            "command: utility name required",
        ));
    };

    if mode != CommandMode::Execute && index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, "command: too many arguments"));
    }

    match mode {
        CommandMode::QueryShort => {
            let Some(line) = command_short_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            sys_println!("{line}");
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::QueryVerbose => {
            let Some(line) = command_verbose_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            sys_println!("{line}");
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::Execute => execute_command_utility(shell, &argv[index..], use_default_path),
    }
}

#[cfg(test)]
fn which(name: &str, shell: &Shell) -> Option<PathBuf> {
    which_in_path(name, shell, false)
}

fn parse_declaration_listing_flag(
    shell: &Shell,
    name: &str,
    argv: &[String],
) -> Result<(bool, usize), ShellError> {
    if argv.len() >= 2 && argv[1] == "-p" {
        if argv.len() > 2 {
            return Err(shell.diagnostic(1, format_args!("{name}: -p does not accept operands")));
        }
        return Ok((true, 2));
    }
    if let Some(arg) = argv.get(1)
        && arg.starts_with('-')
        && arg != "-"
        && arg != "--"
    {
        return Err(shell.diagnostic(1, format_args!("{name}: invalid option: {arg}")));
    }
    Ok((false, 1))
}

fn exported_lines(shell: &Shell) -> Vec<String> {
    shell
        .exported
        .iter()
        .map(|name| declaration_line("export", name, shell.get_var(name)))
        .collect()
}

fn readonly_lines(shell: &Shell) -> Vec<String> {
    shell
        .readonly
        .iter()
        .map(|name| declaration_line("readonly", name, shell.get_var(name)))
        .collect()
}

fn declaration_line(prefix: &str, name: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => format!("{prefix} {name}={}", shell_quote(value)),
        None => format!("{prefix} {name}"),
    }
}

fn pwd_output(shell: &Shell, logical: bool) -> Result<String, ShellError> {
    if logical {
        return current_logical_pwd(shell);
    }
    sys::get_cwd().map_err(|e| shell.diagnostic(1, &e))
}

fn current_logical_pwd(shell: &Shell) -> Result<String, ShellError> {
    let cwd = sys::get_cwd().map_err(|e| shell.diagnostic(1, &e))?;
    if let Some(pwd) = shell.get_var("PWD")
        && logical_pwd_is_valid(pwd)
        && paths_match_logically(pwd, &cwd)
    {
        return Ok(pwd.to_string());
    }
    Ok(cwd)
}

fn logical_pwd_is_valid(path: &str) -> bool {
    Path::new(path).is_absolute()
        && !Path::new(path)
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

fn paths_match_logically(lhs: &str, rhs: &str) -> bool {
    sys::canonicalize(lhs).ok() == sys::canonicalize(rhs).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsetTarget {
    Variable,
    Function,
}

fn parse_unset_target(shell: &Shell, argv: &[String]) -> Result<(UnsetTarget, usize), ShellError> {
    let mut target = UnsetTarget::Variable;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        if !arg.starts_with('-') || arg == "-" {
            break;
        }
        if arg == "--" {
            index += 1;
            break;
        }
        for ch in arg[1..].chars() {
            match ch {
                'v' => target = UnsetTarget::Variable,
                'f' => target = UnsetTarget::Function,
                _ => {
                    return Err(shell.diagnostic(1, format_args!("unset: invalid option: -{ch}")));
                }
            }
        }
        index += 1;
    }
    Ok((target, index))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandMode {
    Execute,
    QueryShort,
    QueryVerbose,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CommandDescription {
    Alias(String),
    Function,
    SpecialBuiltin,
    RegularBuiltin,
    ReservedWord,
    External(PathBuf),
}

fn parse_command_options(argv: &[String]) -> (bool, CommandMode, usize) {
    let mut use_default_path = false;
    let mut mode = CommandMode::Execute;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_str() {
            "-p" => {
                use_default_path = true;
                index += 1;
            }
            "-v" => {
                mode = CommandMode::QueryShort;
                index += 1;
            }
            "-V" => {
                mode = CommandMode::QueryVerbose;
                index += 1;
            }
            "--" => {
                index += 1;
                break;
            }
            _ if arg.starts_with('-') && arg != "-" => break,
            _ => break,
        }
    }
    (use_default_path, mode, index)
}

fn command_usage_status(mode: CommandMode) -> i32 {
    match mode {
        CommandMode::Execute => 127,
        CommandMode::QueryShort | CommandMode::QueryVerbose => 1,
    }
}

fn command_short_description(shell: &Shell, name: &str, use_default_path: bool) -> Option<String> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => Some(format_alias_definition(name, &value)),
        CommandDescription::Function
        | CommandDescription::SpecialBuiltin
        | CommandDescription::RegularBuiltin
        | CommandDescription::ReservedWord => Some(name.to_string()),
        CommandDescription::External(path) => Some(path.display().to_string()),
    }
}

fn command_verbose_description(
    shell: &Shell,
    name: &str,
    use_default_path: bool,
) -> Option<String> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => {
            Some(format!("{name} is an alias for {}", shell_quote(&value)))
        }
        CommandDescription::Function => Some(format!("{name} is a function")),
        CommandDescription::SpecialBuiltin => Some(format!("{name} is a special built-in utility")),
        CommandDescription::RegularBuiltin => Some(format!("{name} is a regular built-in utility")),
        CommandDescription::ReservedWord => Some(format!("{name} is a reserved word")),
        CommandDescription::External(path) => Some(format!("{name} is {}", path.display())),
    }
}

fn describe_command(
    shell: &Shell,
    name: &str,
    use_default_path: bool,
) -> Option<CommandDescription> {
    if let Some(value) = shell.aliases.get(name) {
        return Some(CommandDescription::Alias(value.to_string()));
    }
    if shell.functions.contains_key(name) {
        return Some(CommandDescription::Function);
    }
    if is_special_builtin(name) {
        return Some(CommandDescription::SpecialBuiltin);
    }
    if is_builtin(name) {
        return Some(CommandDescription::RegularBuiltin);
    }
    if is_reserved_word_name(name) {
        return Some(CommandDescription::ReservedWord);
    }
    which_in_path(name, shell, use_default_path).map(CommandDescription::External)
}

fn execute_command_utility(
    shell: &mut Shell,
    argv: &[String],
    use_default_path: bool,
) -> Result<BuiltinOutcome, ShellError> {
    let name = &argv[0];
    if is_builtin(name) {
        return match run(shell, argv, &[]) {
            Ok(outcome) => Ok(outcome),
            Err(error) => Ok(BuiltinOutcome::Status(error.exit_status())),
        };
    }

    let Some(path) = which_in_path(name, shell, use_default_path) else {
        return Ok(diag_status(
            shell,
            127,
            format_args!("command: {name}: not found"),
        ));
    };

    let path_str = path.display().to_string();
    if sys::access_path(&path_str, sys::X_OK).is_err() {
        return Ok(diag_status(
            shell,
            126,
            format_args!("command: {name}: Permission denied"),
        ));
    }

    let mut child_env = shell.env_for_child();
    if use_default_path {
        child_env.retain(|(k, _)| k != "PATH");
        child_env.push(("PATH".to_string(), DEFAULT_COMMAND_PATH.to_string()));
    }
    let env_pairs: Vec<(&str, &str)> = child_env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let argv_strs: Vec<&str> = argv.iter().map(String::as_str).collect();

    match sys::spawn_child(
        &path_str,
        &argv_strs,
        Some(&env_pairs),
        &[],
        None,
        false,
        None,
    ) {
        Ok(handle) => {
            let ws = sys::wait_pid(handle.pid, false)
                .map_err(|e| shell.diagnostic(1, &e))?
                .expect("child status");
            Ok(BuiltinOutcome::Status(sys::decode_wait_status(ws.status)))
        }
        Err(error) if error.is_enoent() => Ok(diag_status(
            shell,
            127,
            format_args!("command: {name}: not found"),
        )),
        Err(error) => Ok(diag_status(
            shell,
            126,
            format_args!("command: {name}: {error}"),
        )),
    }
}

fn which_in_path(name: &str, shell: &Shell, use_default_path: bool) -> Option<PathBuf> {
    search_path(name, shell, use_default_path, path_exists)
}

fn search_path(
    name: &str,
    shell: &Shell,
    use_default_path: bool,
    predicate: fn(&Path) -> bool,
) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        if predicate(&path) {
            return absolute_path(&path);
        }
        return None;
    }

    let path_env = if use_default_path {
        DEFAULT_COMMAND_PATH.to_string()
    } else {
        shell
            .get_var("PATH")
            .map(|s| s.to_string())
            .or_else(|| sys::env_var("PATH"))
            .unwrap_or_default()
    };

    for dir in path_env.split(':') {
        let base = if dir.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(dir)
        };
        let path = base.join(name);
        if predicate(&path) {
            return absolute_path(&path);
        }
    }
    None
}

fn path_exists(path: &Path) -> bool {
    sys::file_exists(&path.display().to_string())
}

fn readable_regular_file(path: &Path) -> bool {
    let p = path.display().to_string();
    sys::is_regular_file(&p) && sys::access_path(&p, sys::R_OK).is_ok()
}

fn absolute_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    sys::get_cwd().ok().map(|cwd| PathBuf::from(cwd).join(path))
}

const SPECIAL_BUILTIN_NAMES: &[&str] = &[
    ".", ":", "break", "continue", "eval", "exec", "exit", "export", "readonly", "return", "set",
    "shift", "times", "trap", "unset",
];

pub fn is_special_builtin(name: &str) -> bool {
    SPECIAL_BUILTIN_NAMES.binary_search(&name).is_ok()
}

fn is_reserved_word_name(word: &str) -> bool {
    matches!(
        word,
        "!" | "{"
            | "}"
            | "case"
            | "do"
            | "done"
            | "elif"
            | "else"
            | "esac"
            | "fi"
            | "for"
            | "if"
            | "in"
            | "then"
            | "until"
            | "while"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use crate::syntax::Word;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::sys::test_support::{
        ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t, t_fork,
    };

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
            last_status: 3,
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
        }
    }

    fn invoke(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
        super::run(shell, argv, &[])
    }

    fn literal(raw: &str) -> Word {
        Word {
            raw: raw.into(),
            line: 0,
        }
    }

    #[test]
    fn builtin_registry_knows_core_commands() {
        assert_no_syscalls(|| {
            assert!(is_builtin("cd"));
            assert!(is_builtin("export"));
            assert!(is_builtin("read"));
            assert!(is_builtin("umask"));
            assert!(!is_builtin("printf"));
        });
    }

    #[test]
    fn export_updates_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &["export".into(), "NAME=value".into()]).expect("export");
            assert_eq!(shell.get_var("NAME"), Some("value"));
            assert!(shell.exported.contains("NAME"));
        });
    }

    #[test]
    fn unset_removes_variable_and_export_flag() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &["export".into(), "NAME=value".into()]).expect("export");

            invoke(&mut shell, &["unset".into(), "NAME".into()]).expect("unset");
            assert_eq!(shell.get_var("NAME"), None);
            assert!(!shell.exported.contains("NAME"));
        });
    }

    #[test]
    fn readonly_marks_variable_readonly() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &["readonly".into(), "LOCKED=value".into()]).expect("readonly");
            assert!(shell.readonly.contains("LOCKED"));
        });
    }

    #[test]
    fn shift_rejects_invalid_arguments() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: shift: 5: shift count out of range\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: shift: numeric argument required\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();

                shell.positional = vec!["a".into()];
                let outcome = invoke(&mut shell, &["shift".into(), "5".into()]).expect("shift");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                let error =
                    invoke(&mut shell, &["shift".into(), "bad".into()]).expect_err("bad shift");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn alias_and_unalias_manage_alias_table() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"ll='ls -l'\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: alias: missing: not found\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: unalias: name required\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &["alias".into(), "ll=ls -l".into()]).expect("alias");
                invoke(&mut shell, &["alias".into(), "la=ls -a".into()]).expect("alias");
                assert_eq!(shell.aliases.get("ll").map(|s| &**s), Some("ls -l"));

                let outcome =
                    invoke(&mut shell, &["alias".into(), "ll".into()]).expect("alias query");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));

                let outcome =
                    invoke(&mut shell, &["alias".into(), "missing".into()]).expect("missing alias");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                invoke(&mut shell, &["unalias".into(), "ll".into()]).expect("unalias");
                assert!(!shell.aliases.contains_key("ll"));
                let outcome = invoke(&mut shell, &["unalias".into(), "missing".into()])
                    .expect("unalias missing");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                let outcome =
                    invoke(&mut shell, &["unalias".into(), "-a".into()]).expect("unalias all");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(shell.aliases.is_empty());

                let error = invoke(&mut shell, &["unalias".into()]).expect_err("missing alias");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn alias_output_is_shell_quoted_for_reinput() {
        assert_no_syscalls(|| {
            assert_eq!(format_alias_definition("ll", "ls -l"), "ll='ls -l'");
            assert_eq!(format_alias_definition("sq", "a'b"), "sq='a'\\''b'");
            assert_eq!(format_alias_definition("empty", ""), "empty=''");
        });
    }

    #[test]
    fn read_options_and_assignments_parsing() {
        assert_no_syscalls(|| {
            let (options, vars) = parse_read_options(&[
                "read".into(),
                "-r".into(),
                "-d".into(),
                ",".into(),
                "A".into(),
                "B".into(),
            ])
            .expect("read options");
            assert!(options.raw);
            assert_eq!(options.delimiter, b',');
            assert_eq!(vars, vec!["A".to_string(), "B".to_string()]);
            assert_eq!(
                parse_read_options(&["read".into(), "-d".into(), "".into(), "NUL".into()])
                    .expect("read nul delim")
                    .0
                    .delimiter,
                0
            );
            assert_eq!(
                parse_read_options(&["read".into(), "--".into(), "NAME".into()])
                    .expect("read dash dash")
                    .1,
                vec!["NAME".to_string()]
            );

            assert_eq!(
                split_read_assignments(
                    &[("alpha beta gamma".to_string(), false)],
                    &["FIRST".into(), "SECOND".into()],
                    None,
                ),
                vec!["alpha".to_string(), "beta gamma".to_string()]
            );
            assert_eq!(
                split_read_assignments(
                    &[("  alpha beta  ".to_string(), false)],
                    &["ONLY".into()],
                    None,
                ),
                vec!["alpha beta".to_string()]
            );
            assert_eq!(split_read_assignments(&[], &[], None), Vec::<String>::new());
            assert_eq!(
                split_read_assignments(
                    &[("alpha beta".to_string(), false)],
                    &["ONE".into(), "TWO".into()],
                    Some(String::new()),
                ),
                vec!["alpha beta".to_string(), String::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(" \t ".to_string(), false)],
                    &["ONE".into(), "TWO".into()],
                    None,
                ),
                vec![String::new(), String::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[("left,right".to_string(), false)],
                    &["ONE".into(), "TWO".into()],
                    Some(",".into()),
                ),
                vec!["left".to_string(), "right".to_string()]
            );
            assert_eq!(
                split_read_assignments(
                    &[("alpha".to_string(), false)],
                    &["ONE".into(), "TWO".into(), "THREE".into()],
                    None,
                ),
                vec!["alpha".to_string(), String::new(), String::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[("alpha,   ".to_string(), false)],
                    &["ONE".into(), "TWO".into(), "THREE".into()],
                    Some(", ".into()),
                ),
                vec!["alpha".to_string(), String::new(), String::new()]
            );

            let mut pieces = Vec::new();
            let mut empty = String::new();
            push_read_piece(&mut pieces, &mut empty, false);
            assert!(pieces.is_empty());
        });
    }

    #[test]
    fn umask_parsing_helpers() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask("077", 0o022), Some(0o077));
            assert_eq!(parse_umask_mask("g-w", 0o002), Some(0o022));
            assert_eq!(parse_umask_mask("u=rw,go=r", 0o022), Some(0o133));
            assert_eq!(parse_umask_mask("a+x", 0o777), Some(0o666));
            assert_eq!(parse_umask_mask("u=g", 0o022), Some(0o222));
            assert_eq!(parse_umask_mask("u=o", 0o022), Some(0o222));
            assert_eq!(format_umask_symbolic(0o022), "u=rwx,g=rx,o=rx");
            assert_eq!(parse_symbolic_targets("z"), 0);
            assert_eq!(permission_bits_for_targets(0o070, 0o111), 0o010);
            assert_eq!(permission_bits_for_targets(0o007, 0o444), 0o004);
            assert_eq!(copy_permission_bits(0o754, 0o070, 0o070), 0o050);
            assert_eq!(copy_permission_bits(0o754, 0o007, 0o007), 0o004);
            assert_eq!(copy_permission_bits(0o754, 0o700, 0), 0);
        });
    }

    #[test]
    fn format_times_value_helper() {
        assert_no_syscalls(|| {
            assert_eq!(format_times_value(125, 100), "0m1.25s");
        });
    }

    #[test]
    fn read_builtin_error_paths() {
        fn byte_reads(fd: i32, data: &[u8]) -> Vec<crate::sys::test_support::TraceEntry> {
            data.iter()
                .map(|&b| {
                    t(
                        "read",
                        vec![ArgMatcher::Fd(fd), ArgMatcher::Any],
                        TraceResult::Bytes(vec![b]),
                    )
                })
                .collect()
        }
        fn wlen(msg: &str) -> crate::sys::test_support::TraceEntry {
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(msg.as_bytes().to_vec()),
                ],
                TraceResult::Auto,
            )
        }

        let read_err_msg = format!("meiksh: read: {}\n", sys::SysError::Errno(sys::EBADF));
        let readonly_err_msg = "meiksh: read: LOCKED: readonly variable\n";

        let mut trace: Vec<crate::sys::test_support::TraceEntry> = Vec::new();

        // Block 1: open empty, read_with_input(["read"], fd) → reads into REPLY, EOF → status 1, close
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/empty".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(100),
        ));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(100), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        trace.push(t("close", vec![ArgMatcher::Fd(100)], TraceResult::Int(0)));

        // Block 2: open empty, read_with_input(["read","-d","xx","NAME"], fd) → bad delim → write_stderr, close
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/empty".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(101),
        ));
        trace.push(wlen("meiksh: read: invalid usage\n"));
        trace.push(t("close", vec![ArgMatcher::Fd(101)], TraceResult::Int(0)));

        // Block 3: read_with_input(["read","NAME"], -1) → read error → write_stderr
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(-1), ArgMatcher::Any],
            TraceResult::Err(sys::EBADF),
        ));
        trace.push(t(
            "write",
            vec![
                ArgMatcher::Fd(2),
                ArgMatcher::Bytes(read_err_msg.as_bytes().to_vec()),
            ],
            TraceResult::Auto,
        ));

        // Block 4: open value_nl, read "value\n" byte-by-byte, readonly error → write_stderr, close
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/value_nl".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(102),
        ));
        trace.extend(byte_reads(102, b"value\n"));
        trace.push(wlen(readonly_err_msg));
        trace.push(t("close", vec![ArgMatcher::Fd(102)], TraceResult::Int(0)));

        // Block 5: open continued "line\\\ncontinued\n", force_interactive → PS2 prompt write, close
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/continued".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(103),
        ));
        trace.extend(byte_reads(103, b"line"));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(103), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\\']),
        ));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(103), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\n']),
        ));
        // line continuation detected, PS2 prompt written to stderr
        trace.push(t(
            "write",
            vec![ArgMatcher::Fd(2), ArgMatcher::Bytes(b"cont> ".to_vec())],
            TraceResult::Auto,
        ));
        trace.extend(byte_reads(103, b"continued\n"));
        trace.push(t("close", vec![ArgMatcher::Fd(103)], TraceResult::Int(0)));

        // Block 6: open softwrap "soft\\\nwrap\n", interactive=false → no PS2
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/softwrap".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(104),
        ));
        trace.extend(byte_reads(104, b"soft"));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(104), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\\']),
        ));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(104), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\n']),
        ));
        trace.extend(byte_reads(104, b"wrap\n"));
        trace.push(t("close", vec![ArgMatcher::Fd(104)], TraceResult::Int(0)));

        // Block 7: open escaped "left\\ right\n"
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/escaped".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(105),
        ));
        trace.extend(byte_reads(105, b"left"));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(105), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\\']),
        ));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(105), ArgMatcher::Any],
            TraceResult::Bytes(vec![b' ']),
        ));
        // escaped space: not '\n' and not delimiter, so push piece as quoted
        trace.extend(byte_reads(105, b"right\n"));
        trace.push(t("close", vec![ArgMatcher::Fd(105)], TraceResult::Int(0)));

        // Block 8: open tail_bs "tail\\"
        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/tail_bs".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(106),
        ));
        trace.extend(byte_reads(106, b"tail"));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(106), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\\']),
        ));
        // read next byte after backslash → EOF
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(106), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        trace.push(t("close", vec![ArgMatcher::Fd(106)], TraceResult::Int(0)));

        run_trace(trace, || {
            let mut shell = test_shell();

            let empty_fd = sys::open_file("/tmp/empty", sys::O_RDONLY, 0).expect("open empty");
            assert!(matches!(
                read_with_input(&mut shell, &["read".into()], empty_fd).expect("read into REPLY"),
                BuiltinOutcome::Status(1)
            ));
            assert_eq!(shell.get_var("REPLY"), Some(""));
            sys::close_fd(empty_fd).ok();

            let empty_fd2 = sys::open_file("/tmp/empty", sys::O_RDONLY, 0).expect("open empty2");
            assert!(matches!(
                read_with_input(
                    &mut shell,
                    &["read".into(), "-d".into(), "xx".into(), "NAME".into()],
                    empty_fd2,
                )
                .expect("read bad delim"),
                BuiltinOutcome::Status(2)
            ));
            sys::close_fd(empty_fd2).ok();

            assert!(matches!(
                read_with_input(&mut shell, &["read".into(), "NAME".into()], -1,)
                    .expect("read io error"),
                BuiltinOutcome::Status(2)
            ));

            shell.mark_readonly("LOCKED");
            let value_fd = sys::open_file("/tmp/value_nl", sys::O_RDONLY, 0).expect("open value");
            assert!(matches!(
                read_with_input(&mut shell, &["read".into(), "LOCKED".into()], value_fd,)
                    .expect("readonly read"),
                BuiltinOutcome::Status(2)
            ));
            sys::close_fd(value_fd).ok();

            shell.options.force_interactive = true;
            shell.interactive = true;
            shell.env.insert("PS2".into(), "cont> ".into());
            let cont_fd =
                sys::open_file("/tmp/continued", sys::O_RDONLY, 0).expect("open continued");
            assert!(matches!(
                read_with_input(&mut shell, &["read".into(), "JOINED".into()], cont_fd,)
                    .expect("continued read"),
                BuiltinOutcome::Status(0)
            ));
            assert_eq!(shell.get_var("JOINED"), Some("linecontinued"));
            sys::close_fd(cont_fd).ok();

            shell.options.force_interactive = false;
            shell.interactive = false;
            let wrap_fd = sys::open_file("/tmp/softwrap", sys::O_RDONLY, 0).expect("open softwrap");
            let (pieces, hit_delimiter) = read_logical_line(
                &shell,
                ReadOptions {
                    raw: false,
                    delimiter: b'\n',
                },
                wrap_fd,
            )
            .expect("direct read");
            assert!(hit_delimiter);
            assert_eq!(flatten_read_pieces(&pieces), "softwrap");
            sys::close_fd(wrap_fd).ok();

            let esc_fd = sys::open_file("/tmp/escaped", sys::O_RDONLY, 0).expect("open escaped");
            assert!(matches!(
                read_with_input(&mut shell, &["read".into(), "ESCAPED".into()], esc_fd,)
                    .expect("escaped read"),
                BuiltinOutcome::Status(0)
            ));
            assert_eq!(shell.get_var("ESCAPED"), Some("left right"));
            sys::close_fd(esc_fd).ok();

            let tail_fd = sys::open_file("/tmp/tail_bs", sys::O_RDONLY, 0).expect("open tail");
            assert!(matches!(
                read_with_input(&mut shell, &["read".into(), "TAIL".into()], tail_fd,)
                    .expect("tail read"),
                BuiltinOutcome::Status(1)
            ));
            assert_eq!(shell.get_var("TAIL"), Some("tail\\"));
            sys::close_fd(tail_fd).ok();
        });
    }

    #[test]
    fn times_builtin_error_path() {
        let times_err_msg = format!("meiksh: times: {}\n", sys::SysError::Errno(0));
        run_trace(
            vec![
                t("times", vec![ArgMatcher::Any], TraceResult::Err(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(60)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(times_err_msg.as_bytes().to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let shell = test_shell();
                assert!(matches!(times(&shell), BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn umask_builtin_error_paths() {
        run_trace(
            vec![
                // umask -Z → write_stderr
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: umask: invalid option: -Z\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // umask -- 077 → current_umask() then set_umask(077)
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t(
                    "umask",
                    vec![ArgMatcher::Int(0o077)],
                    TraceResult::Int(0o077),
                ),
                // umask 077 022 → current_umask() then write_stderr
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: umask: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // umask u+s → current_umask() then set_umask (s has no effect on permission bits)
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                // umask u+Q → current_umask() then write_stderr
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t("umask", vec![ArgMatcher::Int(0)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: umask: invalid mask: u+Q\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["umask".into(), "-Z".into()]).expect("bad option"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["umask".into(), "--".into(), "077".into()])
                        .expect("double dash"),
                    BuiltinOutcome::Status(0)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["umask".into(), "077".into(), "022".into()])
                        .expect("too many"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["umask".into(), "u+s".into()]).expect("s perm accepted"),
                    BuiltinOutcome::Status(0)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["umask".into(), "u+Q".into()]).expect("bad symbolic"),
                    BuiltinOutcome::Status(1)
                ));
            },
        );
    }

    #[test]
    fn umask_symbolic_mode_parsing() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask("-w", 0o022), Some(0o222));
            assert_eq!(parse_umask_mask("a+r", 0o777), Some(0o333));
            assert_eq!(parse_umask_mask("g=u", 0o022), Some(0o002));
            assert_eq!(parse_umask_mask("u!r", 0o022), None);
            assert_eq!(parse_umask_mask(",,", 0o022), None);
        });
    }

    #[test]
    fn exit_builtin_returns_status() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: exit: numeric argument required\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["exit".into()]).expect("exit");
                assert!(matches!(outcome, BuiltinOutcome::Exit(3)));

                let error =
                    invoke(&mut shell, &["exit".into(), "bad".into()]).expect_err("bad exit");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn command_builtin_runs_subcommands() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: command: utility name required\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();

                let outcome =
                    invoke(&mut shell, &["command".into(), "export".into()]).expect("command");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));

                let outcome = invoke(&mut shell, &["command".into()]).expect("missing utility");
                assert!(matches!(outcome, BuiltinOutcome::Status(127)));
            },
        );
    }

    #[test]
    fn control_flow_builtins_validate_context_and_arguments() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: return: not in a function\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: return: numeric argument required\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: return: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: break: only meaningful in a loop\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: continue: only meaningful in a loop\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: continue: numeric argument required\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: break: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: continue: numeric argument required\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &["return".into()]).expect_err("return outside function");

                shell.function_depth = 1;
                let outcome = invoke(&mut shell, &["return".into(), "7".into()]).expect("return");
                assert!(matches!(outcome, BuiltinOutcome::Return(7)));
                invoke(&mut shell, &["return".into(), "bad".into()]).expect_err("bad return");
                invoke(&mut shell, &["return".into(), "1".into(), "2".into()])
                    .expect_err("return args");

                shell.function_depth = 0;
                invoke(&mut shell, &["break".into()]).expect_err("break outside loop");
                invoke(&mut shell, &["continue".into()]).expect_err("continue outside loop");

                shell.loop_depth = 2;
                let outcome = invoke(&mut shell, &["break".into(), "9".into()]).expect("break");
                assert!(matches!(outcome, BuiltinOutcome::Break(2)));
                let outcome =
                    invoke(&mut shell, &["continue".into(), "2".into()]).expect("continue");
                assert!(matches!(outcome, BuiltinOutcome::Continue(2)));
                invoke(&mut shell, &["continue".into(), "0".into()]).expect_err("bad continue");
                invoke(&mut shell, &["break".into(), "1".into(), "2".into()])
                    .expect_err("break args");
                invoke(&mut shell, &["continue".into(), "bad".into()])
                    .expect_err("continue numeric");
            },
        );
    }

    #[test]
    fn wait_rejects_invalid_job_id() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: wait: invalid job id: %bad\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let wait_outcome =
                    invoke(&mut shell, &["wait".into(), "%bad".into()]).expect("bad wait");
                assert!(matches!(wait_outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn fg_errors_without_current_job() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: fg: no job control\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["fg".into()]).expect("fg"),
                    BuiltinOutcome::Status(1)
                ));
            },
        );
    }

    #[test]
    fn bg_errors_without_current_job() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: bg: no job control\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["bg".into()]).expect("bg"),
                    BuiltinOutcome::Status(1)
                ));
            },
        );
    }

    #[test]
    fn cd_changes_directory() {
        run_trace(
            vec![
                // cd /tmp/cd-target: current_logical_pwd → getcwd, then chdir
                t("getcwd", vec![], TraceResult::CwdStr("/home".into())),
                t(
                    "chdir",
                    vec![ArgMatcher::Str("/tmp/cd-target".into())],
                    TraceResult::Int(0),
                ),
                // assert_eq!(sys::get_cwd(), "/tmp/cd-target")
                t(
                    "getcwd",
                    vec![],
                    TraceResult::CwdStr("/tmp/cd-target".into()),
                ),
            ],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &["cd".into(), "/tmp/cd-target".into()]).expect("cd");
                assert_eq!(sys::get_cwd().expect("cwd"), "/tmp/cd-target");
            },
        );
    }

    #[test]
    fn set_builtin_handles_options() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();

            let outcome =
                invoke(&mut shell, &["set".into(), "alpha".into(), "beta".into()]).expect("set");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(
                shell.positional,
                vec!["alpha".to_string(), "beta".to_string()]
            );

            let outcome = invoke(
                &mut shell,
                &["set".into(), "--".into(), "gamma".into(), "delta".into()],
            )
            .expect("set --");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(
                shell.positional,
                vec!["gamma".to_string(), "delta".to_string()]
            );

            let outcome = invoke(&mut shell, &["set".into(), "-C".into()]).expect("set -C");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.noclobber);

            let outcome = invoke(&mut shell, &["set".into(), "+C".into()]).expect("set +C");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.noclobber);

            let outcome = invoke(&mut shell, &["set".into(), "-f".into()]).expect("set -f");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.noglob);

            let outcome = invoke(&mut shell, &["set".into(), "+f".into()]).expect("set +f");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.noglob);

            let outcome = invoke(&mut shell, &["set".into(), "-Cf".into()]).expect("set -Cf");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.noclobber);
            assert!(shell.options.noglob);

            let outcome = invoke(
                &mut shell,
                &["set".into(), "-C".into(), "--".into(), "epsilon".into()],
            )
            .expect("set -C --");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.noclobber);
            assert_eq!(shell.positional, vec!["epsilon".to_string()]);

            let outcome = invoke(&mut shell, &["set".into(), "-a".into()]).expect("set -a");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));

            let outcome = invoke(&mut shell, &["set".into(), "-o".into(), "noexec".into()])
                .expect("set -o noexec");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.syntax_check_only);

            let outcome = invoke(&mut shell, &["set".into(), "+o".into(), "noexec".into()])
                .expect("set +o noexec");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.syntax_check_only);

            let outcome = invoke(&mut shell, &["set".into(), "-n".into()]).expect("set -n");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.syntax_check_only);

            let outcome = invoke(&mut shell, &["set".into(), "+n".into()]).expect("set +n");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.syntax_check_only);

            let outcome = invoke(&mut shell, &["set".into(), "-u".into()]).expect("set -u");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.nounset);

            let outcome = invoke(&mut shell, &["set".into(), "+u".into()]).expect("set +u");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.nounset);

            let outcome = invoke(&mut shell, &["set".into(), "-v".into()]).expect("set -v");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.options.verbose);

            let outcome = invoke(&mut shell, &["set".into(), "+v".into()]).expect("set +v");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(!shell.options.verbose);
        });
    }

    #[test]
    fn eval_builtin_executes_code() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.last_status = 0;
            let outcome = invoke(&mut shell, &["eval".into(), "VALUE=42".into()]).expect("eval");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(shell.get_var("VALUE"), Some("42"));

            let outcome = invoke(&mut shell, &["set".into(), "-a".into()]).expect("set -a");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            invoke(&mut shell, &["eval".into(), "AUTO=42".into()]).expect("allexport eval");
            assert!(shell.exported.contains("AUTO"));
        });
    }

    #[test]
    fn dot_builtin_sources_file() {
        run_trace(
            vec![
                // dot /tmp/dot-script.sh: stat → access → open → read(data) → read(EOF) → close
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/tmp/dot-script.sh".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/tmp/dot-script.sh".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/dot-script.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(100),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(100), ArgMatcher::Any],
                    TraceResult::Bytes(b"FROM_DOT=1\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(100), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(100)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[".".into(), "/tmp/dot-script.sh".into()]).expect("dot");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("FROM_DOT"), Some("1"));
            },
        );
    }

    #[test]
    fn exec_noop_with_no_arguments() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &["exec".into()]).expect("exec no-op");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn set_rejects_invalid_options() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: set: invalid option: z\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: set: invalid option name: bogus\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["set".into(), "-z".into()]).expect("invalid set");
                assert!(matches!(outcome, BuiltinOutcome::UtilityError(2)));
                let outcome = invoke(&mut shell, &["set".into(), "-o".into(), "bogus".into()])
                    .expect("invalid set -o");
                assert!(matches!(outcome, BuiltinOutcome::UtilityError(2)));
            },
        );
    }

    #[test]
    fn dot_requires_filename_argument() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: .: filename argument required\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &[".".into()]).expect_err("dot missing arg");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn unknown_builtin_returns_127() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &["not-a-builtin".into()]).expect("unknown");
            assert!(matches!(outcome, BuiltinOutcome::Status(127)));
        });
    }

    #[test]
    fn lookup_helpers_cover_reporting_paths() {
        run_trace(
            vec![
                // which("/bin/sh") → access("/bin/sh", F_OK)
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/sh".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // command_short_description("meiksh-not-real") → search_path → access("/definitely/missing/meiksh-not-real", F_OK)
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/definitely/missing/meiksh-not-real".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("PATH".into(), "/definitely/missing".into());
                shell.aliases.insert("ll".into(), "ls -l".into());
                shell.functions.insert(
                    "greet".into(),
                    crate::syntax::Command::Simple(crate::syntax::SimpleCommand {
                        assignments: Vec::new().into_boxed_slice(),

                        words: vec![literal("printf"), literal("hello")].into_boxed_slice(),
                        redirections: Vec::new().into_boxed_slice(),
                    }),
                );
                shell.exported.insert("NAME".into());
                shell.env.insert("NAME".into(), "value with spaces".into());
                shell.readonly.insert("LOCK".into());
                shell.env.insert("LOCK".into(), "x y".into());

                let path = which("/bin/sh", &shell).expect("path lookup");
                assert_eq!(path, PathBuf::from("/bin/sh"));

                assert_eq!(
                    exported_lines(&shell),
                    vec!["export NAME='value with spaces'".to_string()]
                );
                assert_eq!(
                    readonly_lines(&shell),
                    vec!["readonly LOCK='x y'".to_string()]
                );
                assert_eq!(
                    command_short_description(&shell, "ll", false),
                    Some("ll='ls -l'".to_string())
                );
                assert_eq!(
                    command_verbose_description(&shell, "greet", false),
                    Some("greet is a function".to_string())
                );
                assert_eq!(
                    command_verbose_description(&shell, "if", false),
                    Some("if is a reserved word".to_string())
                );
                assert!(command_short_description(&shell, "meiksh-not-real", false).is_none());
            },
        );
    }

    #[test]
    fn pwd_errors_on_invalid_option_and_extra_args() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: pwd: invalid option: -Z\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: pwd: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["pwd".into(), "-Z".into()]).expect("pwd invalid"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["pwd".into(), "extra".into()]).expect("pwd extra"),
                    BuiltinOutcome::Status(1)
                ));
            },
        );
    }

    #[test]
    fn unset_readonly_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: unset: RO: readonly variable\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.env.insert("RO".into(), "1".into());
                shell.readonly.insert("RO".into());
                assert!(matches!(
                    invoke(&mut shell, &["unset".into(), "RO".into()]).expect("unset readonly"),
                    BuiltinOutcome::UtilityError(1)
                ));
            },
        );
    }

    #[test]
    fn declaration_listing_flag_errors() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: export: -p does not accept operands\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: readonly: invalid option: -x\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let shell = test_shell();
                parse_declaration_listing_flag(
                    &shell,
                    "export",
                    &["export".into(), "-p".into(), "NAME".into()],
                )
                .expect_err("export -p operands");

                parse_declaration_listing_flag(
                    &shell,
                    "readonly",
                    &["readonly".into(), "-x".into()],
                )
                .expect_err("readonly invalid");
            },
        );
    }

    #[test]
    fn parse_helpers_for_unset_and_command() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: unset: invalid option: -z\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let shell = test_shell();
                assert_eq!(
                    parse_unset_target(&shell, &["unset".into(), "--".into(), "NAME".into()])
                        .expect("unset --"),
                    (UnsetTarget::Variable, 2)
                );
                parse_unset_target(&shell, &["unset".into(), "-z".into()])
                    .expect_err("unset invalid");

                assert_eq!(
                    parse_command_options(&["command".into(), "--".into(), "echo".into()]),
                    (false, CommandMode::Execute, 2)
                );
                assert_eq!(
                    parse_command_options(&["command".into(), "-p".into(), "sh".into()]),
                    (true, CommandMode::Execute, 2)
                );
                assert_eq!(command_usage_status(CommandMode::QueryShort), 1);
            },
        );
    }

    #[test]
    fn command_v_and_capital_v_paths() {
        run_trace(
            vec![
                // command -v one two → write_stderr
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: command: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // command -v (missing) → write_stderr
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: command: utility name required\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // command -v meiksh-not-real → access in PATH
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/bin/meiksh-not-real".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                // command_short_description("sh") → access /bin/sh
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/sh".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // command_verbose_description("sh") → access /bin/sh
                t(
                    "access",
                    vec![ArgMatcher::Str("/bin/sh".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // which_in_path("./definitely-missing") → access
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("./definitely-missing".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/bin".into());

                assert!(matches!(
                    invoke(
                        &mut shell,
                        &["command".into(), "-v".into(), "one".into(), "two".into()]
                    )
                    .expect("command too many args"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["command".into(), "-v".into()])
                        .expect("command query missing"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(
                        &mut shell,
                        &["command".into(), "-v".into(), "meiksh-not-real".into()]
                    )
                    .expect("command query missing name"),
                    BuiltinOutcome::Status(1)
                ));

                shell.aliases.insert("ll".into(), "echo hi".into());
                assert_eq!(
                    command_verbose_description(&shell, "ll", false),
                    Some("ll is an alias for 'echo hi'".to_string())
                );
                assert_eq!(
                    command_verbose_description(&shell, "command", false),
                    Some("command is a regular built-in utility".to_string())
                );

                let sh_path =
                    command_short_description(&shell, "sh", false).expect("command -v sh");
                assert!(Path::new(&sh_path).is_absolute());
                let sh_verbose =
                    command_verbose_description(&shell, "sh", false).expect("command -V sh");
                assert!(sh_verbose.starts_with("sh is /"));
                assert_eq!(
                    describe_command(&shell, "command", false),
                    Some(CommandDescription::RegularBuiltin)
                );
                assert!(which_in_path("./definitely-missing", &shell, false).is_none());
            },
        );
    }

    #[test]
    fn command_execution_error_paths() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/bin/meiksh-not-real".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: command: meiksh-not-real: not found\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: return: not in a function\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/plain-file".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/plain-file".into()), ArgMatcher::Any],
                    TraceResult::Err(sys::EACCES),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(
                            b"meiksh: command: /tmp/plain-file: Permission denied\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/bin".into());

                assert!(matches!(
                    invoke(&mut shell, &["command".into(), "meiksh-not-real".into()])
                        .expect("command missing"),
                    BuiltinOutcome::Status(127)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["command".into(), "return".into()])
                        .expect("command builtin error"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["command".into(), "/tmp/plain-file".into()])
                        .expect("command plain file"),
                    BuiltinOutcome::Status(126)
                ));
            },
        );
    }

    #[test]
    fn command_spawns_external_utility() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/tmp/missing-interp".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/tmp/missing-interp".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(0),
                ),
                t_fork(
                    TraceResult::Pid(5000),
                    vec![t(
                        "execvp",
                        vec![
                            ArgMatcher::Str("/tmp/missing-interp".into()),
                            ArgMatcher::Any,
                        ],
                        TraceResult::Err(sys::ENOENT),
                    )],
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5000), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(127),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/bin".into());

                assert!(matches!(
                    invoke(
                        &mut shell,
                        &["command".into(), "/tmp/missing-interp".into()]
                    )
                    .expect("command missing interpreter"),
                    BuiltinOutcome::Status(127)
                ));
            },
        );
    }

    #[test]
    fn command_and_which_cover_vfs_lookup_path() {
        run_trace(
            vec![
                // which("sh") → search PATH="/usr/bin" → access("/usr/bin/sh", F_OK)
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/sh".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/usr/bin".into());

                let path = which("sh", &shell).expect("lookup sh");
                assert!(path.is_absolute());
                assert!(path.ends_with("sh"));
            },
        );
    }

    #[test]
    fn pwd_builtin_succeeds() {
        run_trace(
            vec![
                t("getcwd", vec![], TraceResult::CwdStr("/home".into())),
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"/home\n".to_vec())],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["pwd".into()]).expect("pwd"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn export_listing_succeeds() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"export ONLY_NAME\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"export PATH='/usr/bin:/bin'\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
                shell.exported.insert("PATH".into());
                shell.exported.insert("ONLY_NAME".into());
                assert!(matches!(
                    invoke(&mut shell, &["export".into()]).expect("export list"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn readonly_listing_succeeds() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert!(matches!(
                invoke(&mut shell, &["readonly".into(), "FLAG".into()]).expect("readonly"),
                BuiltinOutcome::Status(0)
            ));
            assert!(shell.readonly.contains("FLAG"));
        });
    }

    #[test]
    fn set_listing_succeeds() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert!(matches!(
                invoke(&mut shell, &["set".into()]).expect("set list"),
                BuiltinOutcome::Status(0)
            ));
        });
    }

    #[test]
    fn alias_listing_succeeds() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"ll='ls -l'\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.aliases.insert("ll".into(), "ls -l".into());
                assert!(matches!(
                    invoke(&mut shell, &["alias".into()]).expect("alias list"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn times_builtin_succeeds() {
        run_trace(
            vec![
                t("times", vec![ArgMatcher::Any], TraceResult::Int(0)),
                t("sysconf", vec![ArgMatcher::Any], TraceResult::Int(100)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"0m0.00s 0m0.00s\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"0m0.00s 0m0.00s\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["times".into()]).expect("times"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn trap_listing_succeeds() {
        run_trace(
            vec![t(
                "signal",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["trap".into()]).expect("trap"),
                    BuiltinOutcome::Status(0)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["trap".into(), "echo".into(), "INT".into()])
                        .expect("trap set"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn jobs_listing_succeeds() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert!(matches!(
                invoke(&mut shell, &["jobs".into()]).expect("jobs"),
                BuiltinOutcome::Status(0)
            ));
        });
    }

    #[test]
    fn trap_helpers_cover_listing_reset_and_invalid_paths() {
        let trap_p_defaults: Vec<_> = vec![
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - EXIT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - HUP\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - INT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - QUIT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - ILL\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - ABRT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - FPE\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - BUS\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - USR1\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - SEGV\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - USR2\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - PIPE\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - ALRM\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - TERM\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - CHLD\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - CONT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - TRAP\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - TSTP\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - TTIN\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - TTOU\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - SYS\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
        let mut trace_entries = trap_p_defaults;
        trace_entries.extend(vec![
            // trap "printf hi" QUIT ABRT ALRM TERM → 4 signal(install_handler)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGQUIT as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGABRT as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGALRM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // trap "" TERM → signal(SIGTERM, SIG_IGN)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // trap - TERM → signal(SIGTERM, SIG_DFL)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // trap 1 1 → default_signal_action(SIGHUP) x2 (two "1" args)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGHUP as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGHUP as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // trap -p EXIT INT → 2 stdout lines
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - EXIT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - INT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // shell.set_trap(SIGTERM, Ignore) → signal(SIGTERM, SIG_IGN)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // shell.set_trap(SIGINT, Command) → signal(SIGINT, handler)
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            // print_traps non-default → 5 stdout lines (INT, QUIT, ABRT, ALRM, TERM)
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'printf hi' INT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'printf hi' QUIT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'printf hi' ABRT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'printf hi' ALRM\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- '' TERM\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // print_traps EXIT → 1 stdout line
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- - EXIT\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // print_traps BAD → diagnostic
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: invalid condition: BAD\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // trap "printf hi" BAD → write_stderr
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: invalid condition: BAD\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // trap 999 → write_stderr
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: invalid condition: 999\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // trap_impl("printf hi") → diagnostic
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: condition argument required\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // trap("printf hi") → diagnostic
            t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: condition argument required\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ]);
        run_trace(trace_entries, || {
            let mut shell = test_shell();

            assert_eq!(
                trap_impl(&mut shell, &["trap".into(), "-p".into()]).expect("trap -p"),
                0
            );
            assert_eq!(
                trap_impl(
                    &mut shell,
                    &[
                        "trap".into(),
                        "printf hi".into(),
                        "QUIT".into(),
                        "ABRT".into(),
                        "ALRM".into(),
                        "TERM".into(),
                    ],
                )
                .expect("trap set many"),
                0
            );
            assert_eq!(
                trap_impl(&mut shell, &["trap".into(), "".into(), "TERM".into()])
                    .expect("trap ignore"),
                0
            );
            assert_eq!(
                trap_impl(&mut shell, &["trap".into(), "-".into(), "TERM".into()])
                    .expect("trap default"),
                0
            );
            assert_eq!(
                trap_impl(&mut shell, &["trap".into(), "1".into(), "1".into()])
                    .expect("numeric reset"),
                0
            );
            assert_eq!(
                trap_impl(
                    &mut shell,
                    &["trap".into(), "-p".into(), "EXIT".into(), "INT".into()]
                )
                .expect("trap -p operands"),
                0
            );
            shell
                .set_trap(
                    TrapCondition::Signal(sys::SIGTERM),
                    Some(TrapAction::Ignore),
                )
                .expect("set ignore");
            shell
                .set_trap(
                    TrapCondition::Signal(sys::SIGINT),
                    Some(TrapAction::Command("printf hi".into())),
                )
                .expect("set command");
            print_traps(&shell, false, &[]).expect("print non-default traps");
            print_traps(&shell, false, &["EXIT".into()]).expect("skip default trap");
            assert!(print_traps(&shell, false, &["BAD".into()]).is_err());
            assert_eq!(
                trap_impl(
                    &mut shell,
                    &["trap".into(), "printf hi".into(), "BAD".into()]
                )
                .expect("invalid set"),
                1
            );
            assert_eq!(
                trap_impl(&mut shell, &["trap".into(), "999".into()]).expect("invalid reset"),
                1
            );
            assert!(trap_impl(&mut shell, &["trap".into(), "printf hi".into()]).is_err());
            assert!(matches!(
                trap(&mut shell, &["trap".into(), "printf hi".into()]),
                BuiltinOutcome::Status(1)
            ));
            assert_eq!(
                trap_output_action(&shell, TrapCondition::Exit, false, false),
                None
            );
            assert!(
                matches!(parse_wait_operand("bad", &shell), Err(message) if message.contains("invalid process id"))
            );
            assert_eq!(format_trap_condition(TrapCondition::Signal(99)), "99");
            assert_eq!(supported_trap_conditions().len(), 21);
            assert_eq!(parse_trap_condition("BAD"), None);
        });
    }

    #[test]
    fn cd_dash_updates_pwd_and_oldpwd() {
        run_trace(
            vec![
                // cd - → current_logical_pwd: getcwd, paths_match_logically: realpath x2, then chdir
                t("getcwd", vec![], TraceResult::CwdStr("/home".into())),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/home".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/home".into()),
                ),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/home".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/home".into()),
                ),
                t(
                    "chdir",
                    vec![ArgMatcher::Str("/previous".into())],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"/previous\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("OLDPWD".into(), "/previous".into());
                shell.env.insert("PWD".into(), "/home".into());

                let outcome = invoke(&mut shell, &["cd".into(), "-".into()]).expect("cd dash");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("PWD"), Some("/previous"));
                assert_eq!(shell.get_var("OLDPWD"), Some("/home"));
            },
        );
    }

    #[test]
    fn cd_physical_with_e_returns_status_1_when_getcwd_fails() {
        run_trace(
            vec![
                // current_logical_pwd: getcwd + realpath x2
                t("getcwd", vec![], TraceResult::CwdStr("/old".into())),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/old".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/old".into()),
                ),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/old".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/old".into()),
                ),
                // chdir("/somewhere")
                t(
                    "chdir",
                    vec![ArgMatcher::Str("/somewhere".into())],
                    TraceResult::Int(0),
                ),
                // get_cwd() fails
                t("getcwd", vec![], TraceResult::Err(sys::ENOENT)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PWD".into(), "/old".into());
                let outcome = invoke(
                    &mut shell,
                    &["cd".into(), "-Pe".into(), "/somewhere".into()],
                )
                .expect("cd -Pe");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                assert_eq!(shell.get_var("OLDPWD"), Some("/old"));
            },
        );
    }

    #[test]
    fn cd_physical_without_e_uses_curpath_when_getcwd_fails() {
        run_trace(
            vec![
                // current_logical_pwd: getcwd + realpath x2
                t("getcwd", vec![], TraceResult::CwdStr("/old".into())),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/old".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/old".into()),
                ),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/old".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/old".into()),
                ),
                // chdir
                t(
                    "chdir",
                    vec![ArgMatcher::Str("/somewhere".into())],
                    TraceResult::Int(0),
                ),
                // get_cwd fails
                t("getcwd", vec![], TraceResult::Err(sys::ENOENT)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PWD".into(), "/old".into());
                let outcome = invoke(&mut shell, &["cd".into(), "-P".into(), "/somewhere".into()])
                    .expect("cd -P fallback");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("PWD"), Some("/somewhere"));
            },
        );
    }

    #[test]
    fn cd_logical_from_root_pwd() {
        run_trace(
            vec![
                // cd_logical_curpath → current_logical_pwd: getcwd + realpath x2
                t("getcwd", vec![], TraceResult::CwdStr("/".into())),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/".into()),
                ),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/".into()),
                ),
                // old_pwd = current_logical_pwd: getcwd + realpath x2
                t("getcwd", vec![], TraceResult::CwdStr("/".into())),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/".into()),
                ),
                t(
                    "realpath",
                    vec![ArgMatcher::Str("/".into()), ArgMatcher::Any],
                    TraceResult::RealpathStr("/".into()),
                ),
                // chdir("/tmp")
                t(
                    "chdir",
                    vec![ArgMatcher::Str("/tmp".into())],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PWD".into(), "/".into());
                let outcome =
                    invoke(&mut shell, &["cd".into(), "tmp".into()]).expect("cd from root");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("PWD"), Some("/tmp"));
                assert_eq!(shell.get_var("OLDPWD"), Some("/"));
            },
        );
    }

    #[test]
    fn cd_helper_argument_paths_are_split_out() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: cd: too many arguments\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: cd: OLDPWD not set\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: cd: empty directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // resolve_cd_target("target") CDPATH="/cdpath" → stat /cdpath/target → dir
                t(
                    "stat",
                    vec![ArgMatcher::Str("/cdpath/target".into()), ArgMatcher::Any],
                    TraceResult::StatDir,
                ),
                // resolve_cd_target("missing") CDPATH="/cdpath" → stat /cdpath/missing → ENOENT
                t(
                    "stat",
                    vec![ArgMatcher::Str("/cdpath/missing".into()), ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
                // resolve_cd_target("plain") no CDPATH → no stat calls
                // resolve_cd_target("plain") CDPATH=":/cdpath" → stat ./plain → dir found
                t(
                    "stat",
                    vec![ArgMatcher::Str("./plain".into()), ArgMatcher::Any],
                    TraceResult::StatDir,
                ),
                // second VFS block: resolve_cd_target("plain") CDPATH=":/cdpath" → stat ./plain → ENOENT, stat /cdpath/plain → ENOENT
                t(
                    "stat",
                    vec![ArgMatcher::Str("./plain".into()), ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/cdpath/plain".into()), ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(!logical_pwd_is_valid("./relative"));
                parse_cd_target(&shell, &["cd".into(), "one".into(), "two".into()])
                    .expect_err("too many");
                assert_eq!(
                    parse_cd_target(&shell, &["cd".into()]).expect("default target"),
                    (".".to_string(), false, false, false)
                );
                parse_cd_target(&shell, &["cd".into(), "-".into()]).expect_err("missing oldpwd");
                shell.env.insert("OLDPWD".into(), "/tmp/oldpwd".into());
                let error = invoke(&mut shell, &["cd".into(), "".into()]).expect_err("empty cd");
                assert_ne!(error.exit_status(), 0);
                assert!(parse_cd_target(&shell, &["cd".into(), "--".into(), "-".into()]).is_ok());
                let (_, _, physical, check_pwd) =
                    parse_cd_target(&shell, &["cd".into(), "-P".into()]).expect("-P");
                assert!(physical);
                assert!(!check_pwd);
                let (_, _, physical, check_pwd) =
                    parse_cd_target(&shell, &["cd".into(), "-Pe".into()]).expect("-Pe");
                assert!(physical);
                assert!(check_pwd);
                let (_, _, physical, _) =
                    parse_cd_target(&shell, &["cd".into(), "-LP".into()]).expect("-LP");
                assert!(physical);
                let (_, _, physical, _) =
                    parse_cd_target(&shell, &["cd".into(), "-PL".into()]).expect("-PL");
                assert!(!physical);

                // First VFS block equivalent
                let mut shell = test_shell();

                shell.env.insert("CDPATH".into(), "/cdpath".into());
                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "target", false);
                assert_eq!(resolved, PathBuf::from("/cdpath/target"));
                assert_eq!(pwd_target, "/cdpath/target");
                assert!(should_print);

                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "missing", false);
                assert_eq!(resolved, PathBuf::from("missing"));
                assert_eq!(pwd_target, "missing");
                assert!(!should_print);

                shell.env.remove("CDPATH");
                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);

                shell.env.insert("CDPATH".into(), ":/cdpath".into());
                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "plain", false);
                assert!(resolved.ends_with("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);

                // Second VFS block equivalent
                let mut shell = test_shell();
                shell.env.insert("CDPATH".into(), ":/cdpath".into());
                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);
            },
        );
    }

    #[test]
    fn resolve_cd_target_uses_plain_pwd_for_empty_cdpath_prefix() {
        run_trace(
            vec![
                // CDPATH=":" → prefix="" → stat "./plain" → dir
                t(
                    "stat",
                    vec![ArgMatcher::Str("./plain".into()), ArgMatcher::Any],
                    TraceResult::StatDir,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("CDPATH".into(), ":".into());

                let (resolved, pwd_target, should_print) =
                    resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("./plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);
            },
        );
    }

    #[test]
    fn canonicalize_logical_path_handles_all_cases() {
        assert_no_syscalls(|| {
            assert_eq!(canonicalize_logical_path("/usr/.."), "/");
            assert_eq!(canonicalize_logical_path("/a/b/../c"), "/a/c");
            assert_eq!(canonicalize_logical_path("/a/./b"), "/a/b");
            assert_eq!(canonicalize_logical_path("/"), "/");
            assert_eq!(canonicalize_logical_path("/a/b/../../.."), "/");
            assert_eq!(canonicalize_logical_path("/a//b"), "/a/b");
        });
    }

    #[test]
    fn parse_cd_target_rejects_invalid_option() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: cd: invalid option: -Z\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let shell = test_shell();
                parse_cd_target(&shell, &["cd".into(), "-Z".into()]).expect_err("should reject -Z");
            },
        );
    }

    #[test]
    fn trap_sig_prefix_and_new_signals() {
        assert_no_syscalls(|| {
            assert_eq!(
                parse_trap_condition("SIGUSR1"),
                Some(TrapCondition::Signal(sys::SIGUSR1))
            );
            assert_eq!(
                parse_trap_condition("USR2"),
                Some(TrapCondition::Signal(sys::SIGUSR2))
            );
            assert_eq!(
                parse_trap_condition("SIGPIPE"),
                Some(TrapCondition::Signal(sys::SIGPIPE))
            );
            assert_eq!(
                parse_trap_condition("CHLD"),
                Some(TrapCondition::Signal(sys::SIGCHLD))
            );
            assert_eq!(
                parse_trap_condition("SIGTERM"),
                Some(TrapCondition::Signal(sys::SIGTERM))
            );
            assert_eq!(
                parse_trap_condition("SIGILL"),
                Some(TrapCondition::Signal(sys::SIGILL))
            );
            assert_eq!(
                parse_trap_condition("SIGFPE"),
                Some(TrapCondition::Signal(sys::SIGFPE))
            );
            assert_eq!(
                parse_trap_condition("SIGKILL"),
                Some(TrapCondition::Signal(sys::SIGKILL))
            );
            assert_eq!(
                parse_trap_condition("SIGSEGV"),
                Some(TrapCondition::Signal(sys::SIGSEGV))
            );
            assert_eq!(
                parse_trap_condition("BUS"),
                Some(TrapCondition::Signal(sys::SIGBUS))
            );
            assert_eq!(
                parse_trap_condition("SYS"),
                Some(TrapCondition::Signal(sys::SIGSYS))
            );
            assert_eq!(
                parse_trap_condition("TSTP"),
                Some(TrapCondition::Signal(sys::SIGTSTP))
            );
            assert_eq!(
                parse_trap_condition("TTIN"),
                Some(TrapCondition::Signal(sys::SIGTTIN))
            );
            assert_eq!(
                parse_trap_condition("TTOU"),
                Some(TrapCondition::Signal(sys::SIGTTOU))
            );
            assert_eq!(
                parse_trap_condition("CONT"),
                Some(TrapCondition::Signal(sys::SIGCONT))
            );
            assert_eq!(
                parse_trap_condition("4"),
                Some(TrapCondition::Signal(sys::SIGILL))
            );
            assert_eq!(
                parse_trap_condition("8"),
                Some(TrapCondition::Signal(sys::SIGFPE))
            );
            assert_eq!(
                parse_trap_condition("9"),
                Some(TrapCondition::Signal(sys::SIGKILL))
            );
            assert_eq!(
                parse_trap_condition("10"),
                Some(TrapCondition::Signal(sys::SIGUSR1))
            );
            assert_eq!(
                parse_trap_condition("11"),
                Some(TrapCondition::Signal(sys::SIGSEGV))
            );
            assert_eq!(
                parse_trap_condition("12"),
                Some(TrapCondition::Signal(sys::SIGUSR2))
            );
            assert_eq!(
                parse_trap_condition("13"),
                Some(TrapCondition::Signal(sys::SIGPIPE))
            );
            assert_eq!(
                parse_trap_condition("17"),
                Some(TrapCondition::Signal(sys::SIGCHLD))
            );
            assert_eq!(
                parse_trap_condition("18"),
                Some(TrapCondition::Signal(sys::SIGCONT))
            );
            assert_eq!(
                parse_trap_condition("20"),
                Some(TrapCondition::Signal(sys::SIGTSTP))
            );
            assert_eq!(
                parse_trap_condition("21"),
                Some(TrapCondition::Signal(sys::SIGTTIN))
            );
            assert_eq!(
                parse_trap_condition("22"),
                Some(TrapCondition::Signal(sys::SIGTTOU))
            );

            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGUSR1)),
                "USR1"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGUSR2)),
                "USR2"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGPIPE)),
                "PIPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGCHLD)),
                "CHLD"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGILL)),
                "ILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGFPE)),
                "FPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGKILL)),
                "KILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGSEGV)),
                "SEGV"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGBUS)),
                "BUS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGSYS)),
                "SYS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGCONT)),
                "CONT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTSTP)),
                "TSTP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTTIN)),
                "TTIN"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTTOU)),
                "TTOU"
            );
        });
    }

    #[test]
    fn ignored_on_entry_prevents_trap_in_non_interactive_shell() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .ignored_on_entry
                    .insert(TrapCondition::Signal(sys::SIGHUP));
                shell.interactive = false;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGHUP),
                        Some(TrapAction::Command("echo caught".into())),
                    )
                    .expect("should silently succeed");
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(sys::SIGHUP))
                        .is_none()
                );

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Command("echo caught".into())),
                    )
                    .expect("set SIGTERM");
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(sys::SIGTERM))
                        .is_some()
                );

                shell.interactive = true;
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("interactive can override");
                assert_eq!(
                    shell.trap_action(TrapCondition::Signal(sys::SIGTERM)),
                    Some(&TrapAction::Ignore)
                );
            },
        );
    }

    #[test]
    fn umask_symbolic_s_and_x_uppercase() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask("u+s", 0o022), Some(0o022));
            assert_eq!(parse_umask_mask("u+X", 0o022), Some(0o022));
            assert_eq!(parse_umask_mask("g+s", 0o022), Some(0o022));
        });
    }

    #[test]
    fn dot_path_search_sources_readable_file() {
        run_trace(
            vec![
                // resolve_dot_path("dot-script.sh") → search PATH="/scripts"
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/scripts/dot-script.sh".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/scripts/dot-script.sh".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(0),
                ),
                // source_path → read_file: open, read(data), read(EOF), close
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/scripts/dot-script.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(100),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(100), ArgMatcher::Any],
                    TraceResult::Bytes(b"M6_DOT=loaded\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(100), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(100)], TraceResult::Int(0)),
                // resolve_dot_path("missing-dot.sh") → stat fails
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/scripts/missing-dot.sh".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/scripts".into());
                let status =
                    invoke(&mut shell, &[".".into(), "dot-script.sh".into()]).expect("dot path");
                assert!(matches!(status, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("M6_DOT"), Some("loaded"));
                assert!(resolve_dot_path(&shell, "missing-dot.sh").is_err());
            },
        );
    }

    #[test]
    fn jobs_option_and_operand_parsing_are_split_out() {
        assert_no_syscalls(|| {
            assert_eq!(
                parse_jobs_options(&["jobs".into(), "-p".into(), "%1".into()]).expect("jobs -p"),
                (JobsMode::PidOnly, 2)
            );
            assert_eq!(
                parse_jobs_options(&["jobs".into(), "--".into(), "%1".into()]).expect("jobs --"),
                (JobsMode::Normal, 2)
            );
            assert_eq!(
                parse_jobs_options(&["jobs".into(), "-l".into()]).expect("jobs -l"),
                (JobsMode::Long, 2)
            );
            let mut shell = test_shell();
            use crate::shell::JobState;
            shell.jobs.push(crate::shell::Job {
                id: 1,
                command: "sleep 1".into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Running,
                saved_termios: None,
            });
            shell.jobs.push(crate::shell::Job {
                id: 2,
                command: "sleep 2".into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Running,
                saved_termios: None,
            });
            assert_eq!(
                parse_jobs_operands(&["%1".into(), "%2".into()], &shell).expect("job ids"),
                Some(vec![1, 2])
            );
            assert!(parse_jobs_operands(&["bad".into()], &shell).is_err());
        });
    }

    #[test]
    fn job_display_pid_prefers_child_pid_when_no_pgid() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let handle = sys::ChildHandle {
                pid: 3001,
                stdout_fd: None,
            };
            shell.register_background_job("sleep".into(), None, vec![handle]);
            assert_eq!(job_display_pid(&shell.jobs[0]), Some(3001));
        });
    }

    #[test]
    fn jobs_invalid_inputs_return_status_one() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: jobs: invalid option: -z\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: jobs: invalid job id: bad\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                assert!(matches!(
                    invoke(&mut shell, &["jobs".into(), "-z".into()]).expect("bad jobs"),
                    BuiltinOutcome::Status(1)
                ));
                assert!(matches!(
                    invoke(&mut shell, &["jobs".into(), "bad".into()]).expect("bad job operand"),
                    BuiltinOutcome::Status(1)
                ));
            },
        );
    }

    #[test]
    fn jobs_selected_finished_job_path_is_split_out() {
        run_trace(
            vec![
                // reap_jobs → try_wait_child(3001) → waitpid(3001, WNOHANG) → exited(7)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(3001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(7),
                ),
                // jobs → reap_jobs → try_wait_child(3002) → waitpid(3002, WNOHANG) → still running
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(3002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // "sleep" job reuses id 1 → selected by %{finished_id} → prints Running
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] + Running sleep\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let finished_handle = sys::ChildHandle {
                    pid: 3001,
                    stdout_fd: None,
                };
                let finished_id =
                    shell.register_background_job("done".into(), None, vec![finished_handle]);
                shell.reap_jobs();
                let running_handle = sys::ChildHandle {
                    pid: 3002,
                    stdout_fd: None,
                };
                shell.register_background_job("sleep".into(), None, vec![running_handle]);
                assert!(matches!(
                    jobs(&mut shell, &["jobs".into(), format!("%{finished_id}")]),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn unalias_invalid_option_is_split_out() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: unalias: invalid option: -x\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &["unalias".into(), "-x".into()]).expect_err("unalias invalid");
            },
        );
    }

    #[test]
    fn unalias_requires_name_after_double_dash() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: unalias: name required\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &["unalias".into(), "--".into()]).expect_err("unalias -- only");
            },
        );
    }

    #[test]
    fn wait_fg_bg_success_paths_are_exercised() {
        run_trace(
            vec![
                // bg %1 → continue_job (stopped → Running) → kill(-3001, SIGCONT)
                t(
                    "kill",
                    vec![ArgMatcher::Int(3001), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
                // bg prints "[1] sleep"
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] sleep\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // wait %1 → wait_on_job_index → waitpid(3001, WUNTRACED)
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(3001),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
                // fg prints "sleep"
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"sleep\n".to_vec())],
                    TraceResult::Auto,
                ),
                // fg %2 (running job) → continue_job → kill(3002, SIGCONT)
                t(
                    "kill",
                    vec![ArgMatcher::Int(3002), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
                // fg %2 → wait_for_job → waitpid(3002, WUNTRACED)
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(3002),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let handle = sys::ChildHandle {
                    pid: 3001,
                    stdout_fd: None,
                };
                let id = shell.register_background_job("sleep".into(), None, vec![handle]);
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = crate::shell::JobState::Stopped(sys::SIGTSTP);

                let outcome = invoke(&mut shell, &["bg".into(), format!("%{id}")]).expect("bg");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));

                let outcome = invoke(&mut shell, &["wait".into(), format!("%{id}")]).expect("wait");
                assert!(matches!(outcome, BuiltinOutcome::Status(_)));

                let handle = sys::ChildHandle {
                    pid: 3002,
                    stdout_fd: None,
                };
                let id = shell.register_background_job("sleep".into(), None, vec![handle]);
                let outcome = invoke(&mut shell, &["fg".into(), format!("%{id}")]).expect("fg");
                assert!(matches!(outcome, BuiltinOutcome::Status(_)));
            },
        );
    }

    #[test]
    fn wait_without_explicit_job_uses_all_jobs() {
        run_trace(
            vec![
                // wait_for_all_jobs → wait_for_job_operand(1) → waitpid(3001)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(3001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                // wait_for_job_operand(2) → waitpid(3002)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(3002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    "first".into(),
                    None,
                    vec![sys::ChildHandle {
                        pid: 3001,
                        stdout_fd: None,
                    }],
                );
                shell.register_background_job(
                    "second".into(),
                    None,
                    vec![sys::ChildHandle {
                        pid: 3002,
                        stdout_fd: None,
                    }],
                );

                let outcome = invoke(&mut shell, &["wait".into()]).expect("wait all");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn unset_function_removes_function() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.functions.insert(
                "ll".into(),
                crate::syntax::Command::Simple(crate::syntax::SimpleCommand {
                    assignments: Vec::new().into_boxed_slice(),

                    words: vec![literal("printf"), literal("ok")].into_boxed_slice(),
                    redirections: Vec::new().into_boxed_slice(),
                }),
            );
            invoke(&mut shell, &["unset".into(), "-f".into(), "ll".into()])
                .expect("unset function");
            assert!(!shell.functions.contains_key("ll"));
        });
    }

    #[test]
    fn exec_errors_on_nul_in_program() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: exec: invalid argument\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &["exec".into(), "bad\0program".into()])
                    .expect_err("exec error");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn exec_builtin_success_path_can_be_simulated() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/usr/bin/echo".into()),
                        ArgMatcher::Int(libc::F_OK as i64),
                    ],
                    TraceResult::Int(0),
                ),
                // exec echo hello → PATH resolution then execve(absolute_path, ...)
                t(
                    "execve",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["exec".into(), "echo".into(), "hello".into()])
                    .expect("exec");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn run_with_empty_argv_returns_zero() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[]).expect("empty argv");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn shift_succeeds_with_positional_params() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.positional = vec!["a".into(), "b".into()];
            let outcome = invoke(&mut shell, &["shift".into()]).expect("shift");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(shell.positional, vec!["b".to_string()]);
        });
    }

    #[test]
    fn export_bare_name_without_value() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &["export".into(), "ONLY_NAME".into()]).expect("export bare name");
            assert!(shell.exported.contains("ONLY_NAME"));
        });
    }

    #[test]
    fn eval_reports_syntax_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: line 1: unterminated single quote\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let error = invoke(
                    &mut shell,
                    &["eval".into(), "echo".into(), "'unterminated".into()],
                )
                .expect_err("bad eval");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn dot_errors_on_missing_file() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/definitely/missing-meiksh-dot-file".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: .: /definitely/missing-meiksh-dot-file: not found\n".to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let error = invoke(
                    &mut shell,
                    &[".".into(), "/definitely/missing-meiksh-dot-file".into()],
                )
                .expect_err("missing dot file");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn dot_rejects_too_many_arguments() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: .: too many arguments\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &[".".into(), "one".into(), "two".into()])
                    .expect_err("dot args");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn set_no_args_lists_variables() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"A_VAR=one\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"B_VAR=two\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("B_VAR".into(), "two".into());
                shell.env.insert("A_VAR".into(), "one".into());
                assert!(matches!(
                    invoke(&mut shell, &["set".into()]).expect("set list"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn set_minus_o_lists_options() {
        let writes: Vec<_> = vec![
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"allexport off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"errexit off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"hashall off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"monitor off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"noclobber off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"noglob off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"noexec off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"notify off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"nounset off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"pipefail off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"verbose off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"xtrace off\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
        run_trace(writes, || {
            let mut shell = test_shell();
            assert!(matches!(
                invoke(&mut shell, &["set".into(), "-o".into()]).expect("set -o"),
                BuiltinOutcome::Status(0)
            ));
        });
    }

    #[test]
    fn set_plus_o_lists_options_in_reinput_format() {
        let writes: Vec<_> = vec![
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o allexport\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o errexit\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o hashall\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o monitor\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o noclobber\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o noglob\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o noexec\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o notify\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o nounset\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o pipefail\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o verbose\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"set +o xtrace\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
        run_trace(writes, || {
            let mut shell = test_shell();
            assert!(matches!(
                invoke(&mut shell, &["set".into(), "+o".into()]).expect("set +o"),
                BuiltinOutcome::Status(0)
            ));
        });
    }

    #[test]
    fn jobs_shows_done_and_running_entries() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(3001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1]   Done\tsleep 1\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    "sleep 1".into(),
                    None,
                    vec![crate::sys::ChildHandle {
                        pid: 3001,
                        stdout_fd: None,
                    }],
                );
                assert!(matches!(
                    invoke(&mut shell, &["jobs".into()]).expect("jobs"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    #[test]
    fn jobs_skips_unselected_running_job() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] + Running sleep 100\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(crate::shell::Job {
                    id: 1,
                    command: "sleep 99".into(),
                    pgid: None,
                    last_pid: Some(4001),
                    last_status: None,
                    children: vec![crate::sys::ChildHandle {
                        pid: 4001,
                        stdout_fd: None,
                    }],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                shell.jobs.push(crate::shell::Job {
                    id: 2,
                    command: "sleep 100".into(),
                    pgid: None,
                    last_pid: Some(4002),
                    last_status: None,
                    children: vec![crate::sys::ChildHandle {
                        pid: 4002,
                        stdout_fd: None,
                    }],
                    state: crate::shell::JobState::Running,
                    saved_termios: None,
                });
                assert!(matches!(
                    invoke(&mut shell, &["jobs".into(), "%2".into()]).expect("jobs %2"),
                    BuiltinOutcome::Status(0)
                ));
            },
        );
    }

    fn make_job(id: usize, pid: sys::Pid, cmd: &str) -> crate::shell::Job {
        crate::shell::Job {
            id,
            command: cmd.into(),
            pgid: Some(pid),
            last_pid: Some(pid),
            last_status: None,
            children: vec![crate::sys::ChildHandle {
                pid,
                stdout_fd: None,
            }],
            state: crate::shell::JobState::Running,
            saved_termios: None,
        }
    }

    #[test]
    fn jobs_displays_running_stopped_and_done_markers() {
        run_trace(
            vec![
                // reap_jobs: job 1 (Running) → try_wait_child returns still running
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                // reap_jobs: job 2 (Stopped) → try_wait_child checks for WCONTINUED
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                // reap_jobs: job 3 (Running) → try_wait_child returns exited(42)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5003), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(42),
                ),
                // finished: job 3 Done(42) → stdout
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[3]   Done(42)\tfalse\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // live: job 1 Running → stdout
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] - Running sleep 10\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // live: job 2 Stopped → stdout
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] + Stopped (SIGTSTP) cat\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push({
                    let mut j = make_job(1, 5001, "sleep 10");
                    j.state = crate::shell::JobState::Running;
                    j
                });
                shell.jobs.push({
                    let mut j = make_job(2, 5002, "cat");
                    j.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                    j
                });
                shell.jobs.push(make_job(3, 5003, "false"));

                let outcome = invoke(&mut shell, &["jobs".into()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_pid_only_mode_prints_pgid() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5010), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"5010\n".to_vec())],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5010, "sleep 100"));
                let outcome = invoke(&mut shell, &["jobs".into(), "-p".into()]).expect("jobs -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_long_mode_includes_pid() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5020), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] + 5020 Running sleep 200\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5020, "sleep 200"));
                let outcome = invoke(&mut shell, &["jobs".into(), "-l".into()]).expect("jobs -l");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_done_nonzero_status_in_finished_list() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5030), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(7),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1]   Done(7)\texit7\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5030, "exit7"));
                let outcome = invoke(&mut shell, &["jobs".into()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_skips_finished_in_pid_only_mode() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(5040), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0),
            )],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5040, "done-cmd"));
                let outcome = invoke(&mut shell, &["jobs".into(), "-p".into()]).expect("jobs -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn fg_with_monitor_waits_for_job() {
        run_trace(
            vec![
                // fg prints the command
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"sleep 5\n".to_vec())],
                    TraceResult::Auto,
                ),
                // continue_job: send SIGCONT (tcsetpgrp skipped: owns_terminal=false)
                t(
                    "kill",
                    vec![ArgMatcher::Int(-6001), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
                // wait_for_job: waitpid on child (foreground_handoff skipped: owns_terminal=false)
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(6001),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let mut job = make_job(1, 6001, "sleep 5");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["fg".into(), "%1".into()]).expect("fg");
                assert!(matches!(outcome, BuiltinOutcome::Status(_)));
            },
        );
    }

    #[test]
    fn fg_no_current_job_returns_error() {
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
                let result = invoke(&mut shell, &["fg".into()]);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn bg_with_stopped_job_sends_sigcont() {
        run_trace(
            vec![
                t(
                    "kill",
                    vec![ArgMatcher::Int(-6010), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] sleep 99\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let mut job = make_job(1, 6010, "sleep 99");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["bg".into()]).expect("bg");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn bg_no_stopped_job_returns_error() {
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
                let result = invoke(&mut shell, &["bg".into()]);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn kill_list_signals() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"HUP INT QUIT ILL ABRT FPE KILL BUS USR1 SEGV USR2 PIPE ALRM TERM CHLD STOP CONT TRAP TSTP TTIN TTOU SYS\n".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "-l".into()]).expect("kill -l");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_list_translates_exit_code() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"INT\n".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "-l".into(), "130".into()])
                    .expect("kill -l 130");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_list_unknown_code_errors() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: unknown signal: 999\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "-l".into(), "999".into()])
                    .expect("kill -l 999");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_list_invalid_not_number_errors() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: invalid exit status: abc\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "-l".into(), "abc".into()])
                    .expect("kill -l abc");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_sends_signal_to_pid() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(12345), ArgMatcher::Int(sys::SIGTERM as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["kill".into(), "12345".into()]).expect("kill pid");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_with_named_signal() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(12345), ArgMatcher::Int(sys::SIGINT as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &["kill".into(), "-s".into(), "INT".into(), "12345".into()],
                )
                .expect("kill -s INT");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_with_numeric_signal_shorthand() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(12345), ArgMatcher::Int(9)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "-9".into(), "12345".into()])
                    .expect("kill -9");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_job_specifier_sends_to_pgid() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(-7001), ArgMatcher::Int(sys::SIGTERM as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 7001, "sleep 999"));
                let outcome = invoke(&mut shell, &["kill".into(), "%1".into()]).expect("kill %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_job_specifier_no_such_job() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: %99: no such job\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "%99".into()]).expect("kill %99");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_invalid_pid() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: invalid pid: notapid\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["kill".into(), "notapid".into()]).expect("kill notapid");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_no_args_shows_usage() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(
                        b"meiksh: kill: usage: kill [-s sigspec | -signum] pid... | -l [exit_status]\n"
                            .to_vec(),
                    ),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into()]).expect("kill");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn kill_no_pid_after_signal_shows_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: no process id specified\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["kill".into(), "-9".into()]).expect("kill -9 (no pid)");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn kill_with_double_dash_separator() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(8888), ArgMatcher::Int(sys::SIGTERM as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "--".into(), "8888".into()])
                    .expect("kill -- pid");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_s_requires_signal_name() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: -s requires a signal name\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["kill".into(), "-s".into()]).expect("kill -s (no arg)");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn kill_unknown_signal_name_returns_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: kill: unknown signal: BOGUS\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let result = invoke(
                    &mut shell,
                    &["kill".into(), "-s".into(), "BOGUS".into(), "1".into()],
                );
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn kill_process_not_found_reports_error() {
        run_trace(
            vec![
                t(
                    "kill",
                    vec![ArgMatcher::Int(99999), ArgMatcher::Int(sys::SIGTERM as i64)],
                    TraceResult::Err(sys::ESRCH),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: kill: (99999): No such process\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &["kill".into(), "99999".into()]).expect("kill 99999");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_job_process_not_found_reports_error() {
        run_trace(
            vec![
                t(
                    "kill",
                    vec![ArgMatcher::Int(-7010), ArgMatcher::Int(sys::SIGTERM as i64)],
                    TraceResult::Err(sys::ESRCH),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: kill: (7010): No such process\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 7010, "dead"));
                let outcome =
                    invoke(&mut shell, &["kill".into(), "%1".into()]).expect("kill %1 dead");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn kill_only_double_dash_no_pid() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"meiksh: kill: no process id specified\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "--".into()]).expect("kill --");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn parse_kill_signal_recognizes_sigprefix() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let sig = parse_kill_signal(&shell, "SIGTERM").expect("SIGTERM");
            assert_eq!(sig, sys::SIGTERM);
            let sig = parse_kill_signal(&shell, "TERM").expect("TERM");
            assert_eq!(sig, sys::SIGTERM);
            let sig = parse_kill_signal(&shell, "9").expect("9");
            assert_eq!(sig, 9);
        });
    }

    #[test]
    fn parse_kill_signal_bogus_returns_error() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: kill: unknown signal: BOGUS\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let shell = test_shell();
                assert!(parse_kill_signal(&shell, "BOGUS").is_err());
            },
        );
    }

    #[test]
    fn resolve_job_id_previous_and_substring() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(make_job(1, 8001, "sleep 10"));
            let mut j2 = make_job(2, 8002, "cat file");
            j2.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
            shell.jobs.push(j2);
            let mut j3 = make_job(3, 8003, "grep pattern");
            j3.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
            shell.jobs.push(j3);

            assert_eq!(resolve_job_id(&shell, Some("%-")), Some(2));
            assert_eq!(resolve_job_id(&shell, Some("%?file")), Some(2));
            assert_eq!(resolve_job_id(&shell, Some("%99")), None);
        });
    }

    #[test]
    fn jobs_stopped_job_format_state() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5050), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] + Stopped (SIGTSTP) vim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let mut job = make_job(1, 5050, "vim");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["jobs".into()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_long_mode_stopped_job() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5060), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] + 5060 Stopped (SIGTSTP) vim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let mut job = make_job(1, 5060, "vim");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["jobs".into(), "-l".into()]).expect("jobs -l");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn format_job_state_done_nonzero() {
        assert_no_syscalls(|| {
            let job = crate::shell::Job {
                id: 1,
                command: "bad".into(),
                pgid: Some(100),
                last_pid: Some(100),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Done(5),
                saved_termios: None,
            };
            let (state_str, pid_str) = format_job_state(&job);
            assert_eq!(state_str, "Done(5)");
            assert_eq!(pid_str, "100");
        });
    }

    #[test]
    fn format_job_state_done_zero() {
        assert_no_syscalls(|| {
            let job = crate::shell::Job {
                id: 1,
                command: "ok".into(),
                pgid: Some(200),
                last_pid: Some(200),
                last_status: None,
                children: vec![],
                state: crate::shell::JobState::Done(0),
                saved_termios: None,
            };
            let (state_str, _) = format_job_state(&job);
            assert_eq!(state_str, "Done");
        });
    }

    #[test]
    fn job_current_marker_returns_correct_symbols() {
        assert_no_syscalls(|| {
            assert_eq!(job_current_marker(1, Some(1), Some(2)), '+');
            assert_eq!(job_current_marker(2, Some(1), Some(2)), '-');
            assert_eq!(job_current_marker(3, Some(1), Some(2)), ' ');
        });
    }

    #[test]
    fn jobs_finished_job_prints_done_line_when_selected() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5070), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1]   Done\tdone-cmd\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5070, "done-cmd"));
                let outcome = invoke(&mut shell, &["jobs".into(), "%1".into()]).expect("jobs %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_finished_job_skipped_when_not_selected() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5080), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5081), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] + Running running-cmd\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5080, "done-cmd"));
                shell.jobs.push(make_job(2, 5081, "running-cmd"));
                let outcome = invoke(&mut shell, &["jobs".into(), "%2".into()]).expect("jobs %2");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn jobs_stopped_job_in_finished_list_skips_done_block() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5090), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(5090), ArgMatcher::Any, ArgMatcher::Int(11)],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] + Stopped (SIGTSTP) vim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 5090, "vim"));
                let outcome = invoke(&mut shell, &["jobs".into(), "%1".into()]).expect("jobs %1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_job_with_zero_pid_does_nothing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let mut job = make_job(1, 0, "empty");
            job.pgid = None;
            job.children.clear();
            job.last_pid = Some(0);
            shell.jobs.push(job);
            let outcome =
                invoke(&mut shell, &["kill".into(), "%1".into()]).expect("kill %1 zero-pid");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn parse_kill_signal_zero_via_sig_prefix() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let sig = parse_kill_signal(&shell, "SIG0").expect("SIG0");
            assert_eq!(sig, 0);
        });
    }

    #[test]
    fn kill_matches_job_by_last_pid() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(-9999), ArgMatcher::Int(sys::SIGTERM as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let mut job = make_job(1, 9999, "bg");
                job.children.clear();
                job.last_pid = Some(9999);
                job.pgid = Some(9999);
                shell.jobs.push(job);
                let outcome =
                    invoke(&mut shell, &["kill".into(), "9999".into()]).expect("kill last_pid");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_negative_pid_sends_directly() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(-1), ArgMatcher::Int(sys::SIGTERM as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &["kill".into(), "--".into(), "-1".into()])
                    .expect("kill -1");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn search_path_uses_default_path() {
        run_trace(
            vec![t(
                "access",
                vec![ArgMatcher::Str("/usr/bin/ls".into()), ArgMatcher::Int(0)],
                TraceResult::Int(0),
            )],
            || {
                let shell = test_shell();
                let result = search_path("ls", &shell, true, path_exists);
                assert_eq!(result, Some(PathBuf::from("/usr/bin/ls")));
            },
        );
    }

    #[test]
    fn search_path_empty_dir_maps_to_dot() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("./myscript".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "getcwd",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::CwdStr("/home".into()),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "".into());
                let result = search_path("myscript", &shell, false, path_exists);
                assert_eq!(result, Some(PathBuf::from("/home/myscript")));
            },
        );
    }

    #[test]
    fn absolute_path_resolves_relative() {
        run_trace(
            vec![t(
                "getcwd",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::CwdStr("/work".into()),
            )],
            || {
                let result = absolute_path(Path::new("relative.sh"));
                assert_eq!(result, Some(PathBuf::from("/work/relative.sh")));
            },
        );
    }

    #[test]
    fn execute_command_utility_spawn_enoent_and_other_error() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/cmd".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/cmd".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("fork", vec![], TraceResult::Err(sys::ENOENT)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: command: cmd: not found\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // Second call: fork fails with EIO (non-ENOENT)
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/cmd".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/cmd".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("fork", vec![], TraceResult::Err(sys::EIO)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"meiksh: command: cmd: Input/output error\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/usr/bin".into());

                let outcome =
                    execute_command_utility(&mut shell, &["cmd".into()], false).expect("enoent");
                assert!(matches!(outcome, BuiltinOutcome::Status(127)));

                let outcome =
                    execute_command_utility(&mut shell, &["cmd".into()], false).expect("eio");
                assert!(matches!(outcome, BuiltinOutcome::Status(126)));
            },
        );
    }

    #[test]
    fn execute_command_utility_with_default_path() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/ls".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Str("/usr/bin/ls".into()), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // spawn_child: fork succeeds
                t_fork(
                    TraceResult::Pid(2000),
                    vec![
                        t(
                            "setenv",
                            vec![ArgMatcher::Any, ArgMatcher::Any],
                            TraceResult::Int(0),
                        ),
                        t(
                            "execvp",
                            vec![ArgMatcher::Any, ArgMatcher::Any],
                            TraceResult::Int(-1),
                        ),
                    ],
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(2000), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    execute_command_utility(&mut shell, &["ls".into()], true).expect("command -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn expand_assignment_tilde_covers_unset_home_and_user_lookup() {
        run_trace(
            vec![t(
                "getpwnam",
                vec![ArgMatcher::Str("bob".into())],
                TraceResult::Str("/home/bob".into()),
            )],
            || {
                let shell = test_shell();
                assert_eq!(super::expand_assignment_tilde(&shell, "~/docs"), "~/docs");
                assert_eq!(
                    super::expand_assignment_tilde(&shell, "~bob/docs"),
                    "/home/bob/docs"
                );
            },
        );
    }

    #[test]
    fn exec_not_found_program_returns_error() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/usr/bin/nonexistent".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/bin/nonexistent".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: exec: nonexistent: not found\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &["exec".into(), "nonexistent".into()])
                    .expect_err("exec not found");
                assert_eq!(error.exit_status(), 127);
            },
        );
    }

    #[test]
    fn jobs_displays_signaled_job() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::SignaledSig(15),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1]   Terminated (SIGTERM)\tkilled\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(make_job(1, 7001, "killed"));
                let outcome = invoke(&mut shell, &["jobs".into()]).expect("jobs");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn kill_sigcont_updates_job_state() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(-8001), ArgMatcher::Int(sys::SIGCONT as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let mut job = make_job(1, 8001, "stopped");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["kill".into(), "-CONT".into(), "%1".into()])
                    .expect("kill -CONT");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    shell.jobs[0].state,
                    crate::shell::JobState::Running
                ));
            },
        );
    }

    #[test]
    fn kill_sigcont_then_reap_jobs_keeps_job_running() {
        run_trace(
            vec![
                t(
                    "kill",
                    vec![ArgMatcher::Int(-8001), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(8001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::ContinuedStatus,
                ),
            ],
            || {
                let mut shell = test_shell();
                let mut job = make_job(1, 8001, "stopped");
                job.state = crate::shell::JobState::Stopped(sys::SIGTSTP);
                shell.jobs.push(job);
                let outcome = invoke(&mut shell, &["kill".into(), "-CONT".into(), "%1".into()])
                    .expect("kill -CONT");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                let finished = shell.reap_jobs();
                assert!(finished.is_empty());
                assert!(matches!(
                    shell.jobs[0].state,
                    crate::shell::JobState::Running
                ));
            },
        );
    }

    #[test]
    fn needs_quoting_and_shell_quote_if_needed_coverage() {
        assert!(!super::needs_quoting("simple"));
        assert!(!super::needs_quoting("path/to.file-1+2:3,4"));
        assert!(super::needs_quoting("has space"));
        assert!(super::needs_quoting(""));
        assert!(super::needs_quoting("quo'te"));

        let cow = super::shell_quote_if_needed("hello");
        assert!(matches!(cow, std::borrow::Cow::Borrowed("hello")));

        let cow = super::shell_quote_if_needed("has space");
        assert!(matches!(cow, std::borrow::Cow::Owned(_)));
        assert_eq!(cow.as_ref(), "'has space'");
    }

    #[test]
    fn trap_double_dash_lists_and_sets() {
        run_trace(
            vec![t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGUSR1 as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let result = invoke(&mut shell, &["trap".into(), "--".into()]).expect("trap --");
                assert!(matches!(result, BuiltinOutcome::Status(0)));

                let result = invoke(
                    &mut shell,
                    &["trap".into(), "--".into(), "echo hi".into(), "USR1".into()],
                )
                .expect("trap -- action cond");
                assert!(matches!(result, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    shell.trap_actions.get(&TrapCondition::Signal(sys::SIGUSR1)),
                    Some(TrapAction::Command(cmd)) if cmd.as_ref() == "echo hi"
                ));
            },
        );
    }
}
