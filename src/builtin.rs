use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::shell::{Shell, ShellError, TrapAction, TrapCondition};
use crate::sys;

#[derive(Debug)]
pub enum BuiltinOutcome {
    Status(i32),
    Exit(i32),
    Return(i32),
    Break(usize),
    Continue(usize),
}

pub fn run(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
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
        "exec" => exec_builtin(argv)?,
        "jobs" => jobs(shell, argv),
        "fg" => fg(shell, argv)?,
        "bg" => bg(shell, argv)?,
        "wait" => wait(shell, argv)?,
        "read" => read(shell, argv)?,
        "alias" => alias(shell, argv)?,
        "unalias" => unalias(shell, argv)?,
        "return" => return_builtin(shell, argv)?,
        "break" => break_builtin(shell, argv)?,
        "continue" => continue_builtin(shell, argv)?,
        "times" => times(),
        "trap" => trap(shell, argv),
        "umask" => umask(argv)?,
        "command" => command(shell, argv)?,
        _ => BuiltinOutcome::Status(127),
    };

    Ok(outcome)
}

pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        ":"
            | "."
            | "alias"
            | "bg"
            | "break"
            | "cd"
            | "command"
            | "continue"
            | "eval"
            | "exec"
            | "exit"
            | "export"
            | "false"
            | "fg"
            | "jobs"
            | "pwd"
            | "read"
            | "readonly"
            | "return"
            | "set"
            | "shift"
            | "times"
            | "trap"
            | "true"
            | "unalias"
            | "umask"
            | "unset"
            | "wait"
    )
}


const DEFAULT_COMMAND_PATH: &str = "/usr/bin:/bin";

fn cd(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (target, print_new_pwd) = parse_cd_target(shell, argv)?;
    let (resolved_target, pwd_target, print_new_pwd) = resolve_cd_target(shell, &target, print_new_pwd);
    let old_pwd = current_logical_pwd(shell)?;
    sys::change_dir(&resolved_target.display().to_string())?;
    let new_pwd = cd_pwd_value(&pwd_target)?;
    shell.set_var("OLDPWD", old_pwd)?;
    shell.set_var("PWD", new_pwd.clone())?;
    if print_new_pwd {
        println!("{new_pwd}");
    }
    Ok(BuiltinOutcome::Status(0))
}

fn cd_pwd_value(target: &str) -> Result<String, ShellError> {
    if logical_pwd_is_valid(target) {
        return Ok(target.to_string());
    }
    Ok(sys::get_cwd()?)
}

fn parse_cd_target(shell: &Shell, argv: &[String]) -> Result<(String, bool), ShellError> {
    let mut index = 1usize;
    if argv.get(index).is_some_and(|arg| arg == "--") {
        index += 1;
    }
    if argv.len() > index + 1 {
        return Err(ShellError {
            message: "cd: too many arguments".to_string(),
        });
    }
    let Some(target) = argv.get(index) else {
        return Ok((
            shell.get_var("HOME").unwrap_or_else(|| ".".to_string()),
            false,
        ));
    };
    if target.is_empty() {
        return Err(ShellError {
            message: "cd: empty directory".to_string(),
        });
    }
    if target == "-" {
        return Ok((
            shell.get_var("OLDPWD").ok_or_else(|| ShellError {
                message: "cd: OLDPWD not set".to_string(),
            })?,
            true,
        ));
    }
    if target.starts_with('-') {
        return Err(ShellError {
            message: format!("cd: invalid option: {target}"),
        });
    }
    Ok((target.clone(), false))
}

fn resolve_cd_target(shell: &Shell, target: &str, print_new_pwd: bool) -> (PathBuf, String, bool) {
    if print_new_pwd || target.starts_with('/') || target == "." || target == ".." {
        return (PathBuf::from(target), target.to_string(), print_new_pwd);
    }

    let Some(cdpath) = shell.get_var("CDPATH") else {
        return (PathBuf::from(target), target.to_string(), print_new_pwd);
    };

    for prefix in cdpath.split(':') {
        let base = if prefix.is_empty() { PathBuf::from(".") } else { PathBuf::from(prefix) };
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
                eprintln!("pwd: invalid option: {arg}");
                return Ok(BuiltinOutcome::Status(1));
            }
            _ => {
                eprintln!("pwd: too many arguments");
                return Ok(BuiltinOutcome::Status(1));
            }
        }
    }

    println!("{}", pwd_output(shell, logical)?);
    Ok(BuiltinOutcome::Status(0))
}

fn exit(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let status = argv
        .get(1)
        .map(|value| value.parse::<i32>())
        .transpose()
        .map_err(|_| ShellError {
            message: "exit: numeric argument required".to_string(),
        })?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Exit(status))
}

fn export(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag("export", argv)?;
    if print || index == argv.len() {
        for line in exported_lines(shell) {
            println!("{line}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.export_var(name, Some(value.to_string()))?;
        } else {
            shell.export_var(item, None)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn readonly(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag("readonly", argv)?;
    if print || index == argv.len() {
        for line in readonly_lines(shell) {
            println!("{line}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.set_var(name, value.to_string())?;
            shell.mark_readonly(name);
        } else {
            shell.mark_readonly(item);
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn unset(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let (target, index) = parse_unset_target(argv)?;
    let mut status = 0;
    for item in &argv[index..] {
        match target {
            UnsetTarget::Variable => {
                if let Err(error) = shell.unset_var(item) {
                    eprintln!("unset: {}", error.message);
                    status = 1;
                }
            }
            UnsetTarget::Function => {
                shell.functions.remove(item);
            }
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn set(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.env.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            println!("{name}={value}");
        }
    } else {
        let mut index = 1usize;
        while let Some(arg) = argv.get(index) {
            match arg.as_str() {
                "-o" | "+o" => {
                    let reinput = arg == "+o";
                    if let Some(name) = argv.get(index + 1) {
                        if let Err(error) = shell.options.set_named_option(name, !reinput) {
                            eprintln!("set: {}", error.display_message());
                            return BuiltinOutcome::Status(error.exit_status());
                        }
                        index += 2;
                    } else {
                        for (name, enabled) in shell.options.reportable_options() {
                            if reinput {
                                let prefix = if enabled { '-' } else { '+' };
                                println!("set {prefix}o {name}");
                            } else {
                                println!("{name} {}", if enabled { "on" } else { "off" });
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
                        match ch {
                            'a' => shell.options.allexport = enabled,
                            'C' => shell.options.noclobber = enabled,
                            'f' => shell.options.noglob = enabled,
                            'n' => shell.options.syntax_check_only = enabled,
                            'u' => shell.options.nounset = enabled,
                            'v' => shell.options.verbose = enabled,
                            _ => {
                                eprintln!("set: invalid option: {ch}");
                                return BuiltinOutcome::Status(2);
                            }
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
        .map_err(|_| ShellError {
            message: "shift: numeric argument required".to_string(),
        })?
        .unwrap_or(1);
    if count > shell.positional.len() {
        return Ok(BuiltinOutcome::Status(1));
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
    let path = argv.get(1).ok_or_else(|| ShellError {
        message: ".: filename argument required".to_string(),
    })?;
    if argv.len() > 2 {
        return Err(ShellError {
            message: ".: too many arguments".to_string(),
        });
    }
    let resolved = resolve_dot_path(shell, path)?;
    let status = shell.source_path(&resolved)?;
    Ok(BuiltinOutcome::Status(status))
}

fn resolve_dot_path(shell: &Shell, path: &str) -> Result<PathBuf, ShellError> {
    if path.contains('/') {
        let candidate = PathBuf::from(path);
        if readable_regular_file(&candidate) {
            return Ok(candidate);
        }
        return Err(ShellError {
            message: format!(".: {path}: not found"),
        });
    }
    search_path(path, shell, false, readable_regular_file).ok_or_else(|| ShellError {
        message: format!(".: {path}: not found"),
    })
}

fn exec_builtin(argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() <= 1 {
        return Ok(BuiltinOutcome::Status(0));
    }
    sys::exec_replace(&argv[1], &argv[1..]).map_err(ShellError::from)?;
    Ok(BuiltinOutcome::Status(0))
}

fn return_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.function_depth == 0 {
        return Err(ShellError {
            message: "return: not in a function".to_string(),
        });
    }
    if argv.len() > 2 {
        return Err(ShellError {
            message: "return: too many arguments".to_string(),
        });
    }
    let status = argv
        .get(1)
        .map(|value| value.parse::<i32>())
        .transpose()
        .map_err(|_| ShellError {
            message: "return: numeric argument required".to_string(),
        })?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Return(status))
}

fn break_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(ShellError {
            message: "break: only meaningful in a loop".to_string(),
        });
    }
    let levels = parse_loop_count("break", argv)?;
    Ok(BuiltinOutcome::Break(levels.min(shell.loop_depth)))
}

fn continue_builtin(shell: &Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(ShellError {
            message: "continue: only meaningful in a loop".to_string(),
        });
    }
    let levels = parse_loop_count("continue", argv)?;
    Ok(BuiltinOutcome::Continue(levels.min(shell.loop_depth)))
}

fn parse_loop_count(name: &str, argv: &[String]) -> Result<usize, ShellError> {
    if argv.len() > 2 {
        return Err(ShellError {
            message: format!("{name}: too many arguments"),
        });
    }
    let levels = argv
        .get(1)
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|_| ShellError {
            message: format!("{name}: numeric argument required"),
        })?
        .unwrap_or(1);
    if levels == 0 {
        return Err(ShellError {
            message: format!("{name}: numeric argument required"),
        });
    }
    Ok(levels)
}

fn jobs(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    let (pid_only, index) = match parse_jobs_options(argv) {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            return BuiltinOutcome::Status(1);
        }
    };
    let selected = match parse_jobs_operands(&argv[index..]) {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            return BuiltinOutcome::Status(1);
        }
    };
    let finished = shell.reap_jobs();
    let selected_contains = |id: usize| selected.as_ref().map_or(true, |ids| ids.contains(&id));

    if !pid_only {
        for (id, status) in finished {
            if selected_contains(id) {
                println!("[{id}] Done {status}");
            }
        }
    }
    for job in &shell.jobs {
        if !selected_contains(job.id) {
            continue;
        }
        if pid_only {
            if let Some(pid) = job_display_pid(job) {
                println!("{pid}");
            }
        } else {
            println!("[{}] Running {}", job.id, job.command);
        }
    }
    BuiltinOutcome::Status(0)
}

fn parse_jobs_options(argv: &[String]) -> Result<(bool, usize), String> {
    let mut pid_only = false;
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
            "-p" => pid_only = true,
            _ => return Err(format!("jobs: invalid option: {arg}")),
        }
        index += 1;
    }
    Ok((pid_only, index))
}

fn parse_jobs_operands(operands: &[String]) -> Result<Option<Vec<usize>>, String> {
    if operands.is_empty() {
        return Ok(None);
    }
    let mut ids = Vec::new();
    for operand in operands {
        let Some(id) = parse_job_id_operand(Some(operand.as_str())) else {
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
    let id = parse_job_id_operand(argv.get(1).map(String::as_str))
        .or_else(|| shell.jobs.last().map(|job| job.id))
        .ok_or_else(|| ShellError {
            message: "fg: no current job".to_string(),
        })?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        println!("{}", job.command);
    }
    shell.continue_job(id)?;
    let status = shell.wait_for_job(id)?;
    Ok(BuiltinOutcome::Status(status))
}

fn bg(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let id = parse_job_id_operand(argv.get(1).map(String::as_str))
        .or_else(|| shell.jobs.last().map(|job| job.id))
        .ok_or_else(|| ShellError {
            message: "bg: no current job".to_string(),
        })?;
    shell.continue_job(id)?;
    if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
        println!("[{id}] {}", job.command);
    }
    Ok(BuiltinOutcome::Status(0))
}

fn wait(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        return Ok(BuiltinOutcome::Status(shell.wait_for_all_jobs()?));
    }
    let mut status = 0;
    for operand in &argv[1..] {
        status = match parse_wait_operand(operand) {
            Ok(WaitOperand::Job(id)) => shell.wait_for_job_operand(id)?,
            Ok(WaitOperand::Pid(pid)) => shell.wait_for_pid_operand(pid)?,
            Err(message) => {
                eprintln!("{message}");
                1
            }
        };
    }
    Ok(BuiltinOutcome::Status(status))
}

#[derive(Clone, Copy)]
struct ReadOptions {
    raw: bool,
    delimiter: u8,
}

fn read(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO)?;
    let mut stdin = sys::FdReader::new(sys::STDIN_FILENO);
    read_with_input(shell, argv, &mut stdin)
}

fn read_with_input<R: Read>(
    shell: &mut Shell,
    argv: &[String],
    input: &mut R,
) -> Result<BuiltinOutcome, ShellError> {
    let Some((options, vars)) = parse_read_options(argv) else {
        eprintln!("read: invalid usage");
        return Ok(BuiltinOutcome::Status(2));
    };
    if vars.is_empty() {
        eprintln!("read: variable name required");
        return Ok(BuiltinOutcome::Status(2));
    }

    let (pieces, hit_delimiter) = match read_logical_line(shell, options, input) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("read: {error}");
            return Ok(BuiltinOutcome::Status(2));
        }
    };
    let values = split_read_assignments(&pieces, &vars, shell.get_var("IFS"));
    for (name, value) in vars.iter().zip(values) {
        if let Err(error) = shell.set_var(name, value) {
            eprintln!("read: {}", error.message);
            return Ok(BuiltinOutcome::Status(2));
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

fn read_logical_line<R: Read>(
    shell: &Shell,
    options: ReadOptions,
    input: &mut R,
) -> io::Result<(Vec<(String, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_quoted = false;

    loop {
        let mut byte = [0u8; 1];
        let count = input.read(&mut byte)?;
        if count == 0 {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, false));
        }
        let ch = byte[0];
        if !options.raw && ch == b'\\' {
            let count = input.read(&mut byte)?;
            if count == 0 {
                current.push('\\');
                push_read_piece(&mut pieces, &mut current, current_quoted);
                return Ok((pieces, false));
            }
            let escaped = byte[0];
            if escaped == b'\n' || escaped == options.delimiter {
                push_read_piece(&mut pieces, &mut current, current_quoted);
                current_quoted = false;
                if shell.options.force_interactive
                    || (sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO))
                {
                    let prompt = shell.get_var("PS2").unwrap_or_else(|| "> ".to_string());
                    eprint!("{prompt}");
                    let _ = io::stderr().flush();
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

    let ifs_ws: Vec<char> = ifs.chars().filter(|ch| matches!(ch, ' ' | '\t' | '\n')).collect();
    let ifs_other: Vec<char> = ifs.chars().filter(|ch| !matches!(ch, ' ' | '\t' | '\n')).collect();
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
                debug_assert!(!current.is_empty(), "leading IFS whitespace should already be skipped");
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

fn alias(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.aliases.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            println!("{}", format_alias_definition(name, value));
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.aliases.insert(name.to_string(), value.to_string());
        } else if let Some(value) = shell.aliases.get(item) {
            println!("{}", format_alias_definition(item, value));
        } else {
            eprintln!("alias: {item}: not found");
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

fn unalias(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Err(ShellError {
            message: "unalias: name required".to_string(),
        });
    }
    if argv.len() == 2 && argv[1] == "-a" {
        shell.aliases.clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv[1].starts_with('-') && argv[1] != "-" && argv[1] != "--" {
        return Err(ShellError {
            message: format!("unalias: invalid option: {}", argv[1]),
        });
    }
    let start = usize::from(argv[1] == "--") + 1;
    if start >= argv.len() {
        return Err(ShellError {
            message: "unalias: name required".to_string(),
        });
    }
    let mut status = 0;
    for item in &argv[start..] {
        if shell.aliases.remove(item).is_none() {
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn times() -> BuiltinOutcome {
    match (sys::process_times(), sys::clock_ticks_per_second()) {
        (Ok(times), Ok(ticks_per_second)) => {
            println!(
                "{} {}",
                format_times_value(times.user_ticks, ticks_per_second),
                format_times_value(times.system_ticks, ticks_per_second)
            );
            println!(
                "{} {}",
                format_times_value(times.child_user_ticks, ticks_per_second),
                format_times_value(times.child_system_ticks, ticks_per_second)
            );
            BuiltinOutcome::Status(0)
        }
        (Err(error), _) | (_, Err(error)) => {
            eprintln!("times: {error}");
            BuiltinOutcome::Status(1)
        }
    }
}

fn trap(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    match trap_impl(shell, argv) {
        Ok(status) => BuiltinOutcome::Status(status),
        Err(error) => {
            eprintln!("trap: {}", error.message);
            BuiltinOutcome::Status(1)
        }
    }
}

#[derive(Clone, Copy)]
enum WaitOperand {
    Job(usize),
    Pid(sys::Pid),
}

fn parse_job_id_operand(operand: Option<&str>) -> Option<usize> {
    operand?.trim_start_matches('%').parse::<usize>().ok()
}

fn parse_wait_operand(operand: &str) -> Result<WaitOperand, String> {
    if let Some(rest) = operand.strip_prefix('%') {
        return rest
            .parse::<usize>()
            .map(WaitOperand::Job)
            .map_err(|_| format!("wait: invalid job id: {operand}"));
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
    if is_unsigned_decimal(&argv[1]) {
        for condition in &argv[1..] {
            if let Some(condition) = parse_trap_condition(condition) {
                shell.set_trap(condition, None)?;
            } else {
                eprintln!("trap: invalid condition: {condition}");
                return Ok(1);
            }
        }
        return Ok(0);
    }
    let action = &argv[1];
    if argv.len() == 2 {
        return Err(ShellError {
            message: "condition argument required".to_string(),
        });
    }
    let trap_action = parse_trap_action(action);
    let mut status = 0;
    for condition in &argv[2..] {
        let Some(condition) = parse_trap_condition(condition) else {
            eprintln!("trap: invalid condition: {condition}");
            status = 1;
            continue;
        };
        shell.set_trap(condition, trap_action.clone())?;
    }
    Ok(status)
}

fn print_traps(shell: &Shell, include_defaults: bool, operands: &[String]) -> Result<(), ShellError> {
    let conditions = if operands.is_empty() {
        if include_defaults {
            supported_trap_conditions()
        } else {
            shell.trap_actions.keys().copied().collect()
        }
    } else {
        let mut parsed = Vec::new();
        for operand in operands {
            let Some(condition) = parse_trap_condition(operand) else {
                return Err(ShellError {
                    message: format!("invalid condition: {operand}"),
                });
            };
            parsed.push(condition);
        }
        parsed
    };
    for condition in conditions {
        if let Some(action) = trap_output_action(shell, condition, include_defaults, !operands.is_empty()) {
            println!("trap -- {action} {}", format_trap_condition(condition));
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
        _ => Some(TrapAction::Command(action.to_string())),
    }
}

fn parse_trap_condition(text: &str) -> Option<TrapCondition> {
    match text {
        "0" | "EXIT" => Some(TrapCondition::Exit),
        "HUP" | "1" => Some(TrapCondition::Signal(sys::SIGHUP)),
        "INT" | "2" => Some(TrapCondition::Signal(sys::SIGINT)),
        "QUIT" | "3" => Some(TrapCondition::Signal(sys::SIGQUIT)),
        "ABRT" | "6" => Some(TrapCondition::Signal(sys::SIGABRT)),
        "ALRM" | "14" => Some(TrapCondition::Signal(sys::SIGALRM)),
        "TERM" | "15" => Some(TrapCondition::Signal(sys::SIGTERM)),
        _ => None,
    }
}

fn format_trap_condition(condition: TrapCondition) -> String {
    match condition {
        TrapCondition::Exit => "EXIT".to_string(),
        TrapCondition::Signal(sys::SIGHUP) => "HUP".to_string(),
        TrapCondition::Signal(sys::SIGINT) => "INT".to_string(),
        TrapCondition::Signal(sys::SIGQUIT) => "QUIT".to_string(),
        TrapCondition::Signal(sys::SIGABRT) => "ABRT".to_string(),
        TrapCondition::Signal(sys::SIGALRM) => "ALRM".to_string(),
        TrapCondition::Signal(sys::SIGTERM) => "TERM".to_string(),
        TrapCondition::Signal(signal) => signal.to_string(),
    }
}

fn trap_output_action(
    shell: &Shell,
    condition: TrapCondition,
    include_defaults: bool,
    explicit_operand: bool,
) -> Option<String> {
    match shell.trap_action(condition) {
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

fn umask(argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
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
                eprintln!("umask: invalid option: {arg}");
                return Ok(BuiltinOutcome::Status(1));
            }
            _ => break,
        }
    }

    let current = sys::current_umask() as u16;
    if index == argv.len() {
        if symbolic_output {
            println!("{}", format_umask_symbolic(current));
        } else {
            println!("{current:04o}");
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    if index + 1 != argv.len() {
        eprintln!("umask: too many arguments");
        return Ok(BuiltinOutcome::Status(1));
    }

    let Some(mask) = parse_umask_mask(&argv[index], current) else {
        eprintln!("umask: invalid mask: {}", argv[index]);
        return Ok(BuiltinOutcome::Status(1));
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
        eprintln!("command: utility name required");
        return Ok(BuiltinOutcome::Status(command_usage_status(mode)));
    };

    if mode != CommandMode::Execute && index + 1 != argv.len() {
        eprintln!("command: too many arguments");
        return Ok(BuiltinOutcome::Status(1));
    }

    match mode {
        CommandMode::QueryShort => {
            let Some(line) = command_short_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            println!("{line}");
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::QueryVerbose => {
            let Some(line) = command_verbose_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            println!("{line}");
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::Execute => execute_command_utility(shell, &argv[index..], use_default_path),
    }
}

#[cfg(test)]
fn which(name: &str, shell: &Shell) -> Option<PathBuf> {
    which_in_path(name, shell, false)
}

fn parse_declaration_listing_flag(name: &str, argv: &[String]) -> Result<(bool, usize), ShellError> {
    if argv.len() >= 2 && argv[1] == "-p" {
        if argv.len() > 2 {
            return Err(ShellError {
                message: format!("{name}: -p does not accept operands"),
            });
        }
        return Ok((true, 2));
    }
    if let Some(arg) = argv.get(1)
        && arg.starts_with('-')
        && arg != "-"
        && arg != "--"
    {
        return Err(ShellError {
            message: format!("{name}: invalid option: {arg}"),
        });
    }
    Ok((false, 1))
}

fn exported_lines(shell: &Shell) -> Vec<String> {
    shell.exported
        .iter()
        .map(|name| declaration_line("export", name, shell.get_var(name)))
        .collect()
}

fn readonly_lines(shell: &Shell) -> Vec<String> {
    shell.readonly
        .iter()
        .map(|name| declaration_line("readonly", name, shell.get_var(name)))
        .collect()
}

fn declaration_line(prefix: &str, name: &str, value: Option<String>) -> String {
    match value {
        Some(value) => format!("{prefix} {name}={}", shell_quote(&value)),
        None => format!("{prefix} {name}"),
    }
}

fn pwd_output(shell: &Shell, logical: bool) -> Result<String, ShellError> {
    if logical {
        return current_logical_pwd(shell);
    }
    Ok(sys::get_cwd()?)
}

fn current_logical_pwd(shell: &Shell) -> Result<String, ShellError> {
    let cwd = sys::get_cwd()?;
    if let Some(pwd) = shell.get_var("PWD")
        && logical_pwd_is_valid(&pwd)
        && paths_match_logically(&pwd, &cwd)
    {
        return Ok(pwd);
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

fn parse_unset_target(argv: &[String]) -> Result<(UnsetTarget, usize), ShellError> {
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
                    return Err(ShellError {
                        message: format!("unset: invalid option: -{ch}"),
                    });
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

fn command_verbose_description(shell: &Shell, name: &str, use_default_path: bool) -> Option<String> {
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

fn describe_command(shell: &Shell, name: &str, use_default_path: bool) -> Option<CommandDescription> {
    if let Some(value) = shell.aliases.get(name) {
        return Some(CommandDescription::Alias(value.clone()));
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
        return match run(shell, argv) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                eprintln!("{}", error.message);
                Ok(BuiltinOutcome::Status(1))
            }
        };
    }

    let Some(path) = which_in_path(name, shell, use_default_path) else {
        eprintln!("command: {name}: not found");
        return Ok(BuiltinOutcome::Status(127));
    };

    let path_str = path.display().to_string();
    if sys::access_path(&path_str, sys::X_OK).is_err() {
        eprintln!("command: {name}: Permission denied");
        return Ok(BuiltinOutcome::Status(126));
    }

    let mut child_env = shell.env_for_child();
    if use_default_path {
        child_env.insert("PATH".to_string(), DEFAULT_COMMAND_PATH.to_string());
    }
    let env_pairs: Vec<(&str, &str)> = child_env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let argv_strs: Vec<&str> = argv.iter().map(String::as_str).collect();

    match sys::run_to_status(&path_str, &argv_strs, Some(&env_pairs)) {
        Ok(status) => Ok(BuiltinOutcome::Status(status)),
        Err(error) if error.raw_os_error() == Some(2) => {
            eprintln!("command: {name}: not found");
            Ok(BuiltinOutcome::Status(127))
        }
        Err(error) => {
            eprintln!("command: {name}: {error}");
            Ok(BuiltinOutcome::Status(126))
        }
    }
}

fn which_in_path(name: &str, shell: &Shell, use_default_path: bool) -> Option<PathBuf> {
    search_path(name, shell, use_default_path, path_exists)
}

fn search_path(name: &str, shell: &Shell, use_default_path: bool, predicate: fn(&Path) -> bool) -> Option<PathBuf> {
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
            .or_else(|| std::env::var("PATH").ok())
            .unwrap_or_default()
    };

    for dir in path_env.split(':') {
        let base = if dir.is_empty() { PathBuf::from(".") } else { PathBuf::from(dir) };
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

pub fn is_special_builtin(name: &str) -> bool {
    matches!(
        name,
        "." | ":" | "break" | "continue" | "eval" | "exec" | "exit" | "export" | "readonly"
            | "return" | "set" | "shift" | "times" | "trap" | "unset"
    )
}

fn is_reserved_word_name(word: &str) -> bool {
    matches!(
        word,
        "!"
            | "{"
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
    use std::fs;
    use std::io::{self, Cursor};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command as ProcessCommand;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::sys::test_support::VfsBuilder;
    use crate::test_utils::{cwd_lock, meiksh_bin_path};

    fn child_to_handle(child: std::process::Child) -> sys::ChildHandle {
        sys::ChildHandle {
            pid: child.id() as sys::Pid,
            stdout_fd: None,
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
            last_status: 3,
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

    fn literal(raw: &str) -> Word {
        Word { raw: raw.to_string() }
    }

    #[test]
    fn builtin_registry_knows_core_commands() {
        assert!(is_builtin("cd"));
        assert!(is_builtin("export"));
        assert!(is_builtin("read"));
        assert!(is_builtin("umask"));
        assert!(!is_builtin("printf"));
    }

    #[test]
    fn export_and_unset_update_shell_state() {
        let mut shell = test_shell();
        run(&mut shell, &["export".into(), "NAME=value".into()]).expect("export");
        assert_eq!(shell.get_var("NAME").as_deref(), Some("value"));
        assert!(shell.exported.contains("NAME"));

        run(&mut shell, &["unset".into(), "NAME".into()]).expect("unset");
        assert_eq!(shell.get_var("NAME"), None);
        assert!(!shell.exported.contains("NAME"));
    }

    #[test]
    fn readonly_and_shift_error_paths_are_covered() {
        let mut shell = test_shell();
        run(&mut shell, &["readonly".into(), "LOCKED=value".into()]).expect("readonly");
        assert!(shell.readonly.contains("LOCKED"));

        shell.positional = vec!["a".into()];
        let outcome = run(&mut shell, &["shift".into(), "5".into()]).expect("shift");
        assert!(matches!(outcome, BuiltinOutcome::Status(1)));

        let error = run(&mut shell, &["shift".into(), "bad".into()]).expect_err("bad shift");
        assert_eq!(error.message, "shift: numeric argument required");
    }

    #[test]
    fn alias_and_unalias_manage_alias_table() {
        let mut shell = test_shell();
        run(&mut shell, &["alias".into(), "ll=ls -l".into()]).expect("alias");
        run(&mut shell, &["alias".into(), "la=ls -a".into()]).expect("alias");
        assert_eq!(shell.aliases.get("ll").map(String::as_str), Some("ls -l"));

        let outcome = run(&mut shell, &["alias".into(), "ll".into()]).expect("alias query");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["alias".into(), "missing".into()]).expect("missing alias");
        assert!(matches!(outcome, BuiltinOutcome::Status(1)));

        run(&mut shell, &["unalias".into(), "ll".into()]).expect("unalias");
        assert!(!shell.aliases.contains_key("ll"));
        let outcome = run(&mut shell, &["unalias".into(), "missing".into()]).expect("unalias missing");
        assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        let outcome = run(&mut shell, &["unalias".into(), "-a".into()]).expect("unalias all");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.aliases.is_empty());

        let error = run(&mut shell, &["unalias".into()]).expect_err("missing alias");
        assert_eq!(error.message, "unalias: name required");
    }

    #[test]
    fn alias_output_is_shell_quoted_for_reinput() {
        assert_eq!(format_alias_definition("ll", "ls -l"), "ll='ls -l'");
        assert_eq!(format_alias_definition("sq", "a'b"), "sq='a'\\''b'");
        assert_eq!(format_alias_definition("empty", ""), "empty=''");
    }

    #[test]
    fn read_and_umask_helpers_cover_core_parsing_paths() {
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
        assert_eq!(format_times_value(125, 100), "0m1.25s");
    }

    #[test]
    fn read_and_times_error_paths_are_covered() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("boom"))
            }
        }

        let mut shell = test_shell();
        assert!(matches!(
            read_with_input(&mut shell, &["read".into()], &mut Cursor::new(Vec::<u8>::new())).expect("read no vars"),
            BuiltinOutcome::Status(2)
        ));
        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "-d".into(), "xx".into(), "NAME".into()],
                &mut Cursor::new(Vec::<u8>::new()),
            )
            .expect("read bad delim"),
            BuiltinOutcome::Status(2)
        ));
        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "NAME".into()],
                &mut FailingReader,
            )
            .expect("read io error"),
            BuiltinOutcome::Status(2)
        ));

        shell.mark_readonly("LOCKED");
        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "LOCKED".into()],
                &mut Cursor::new(b"value\n".to_vec()),
            )
            .expect("readonly read"),
            BuiltinOutcome::Status(2)
        ));

        shell.options.force_interactive = true;
        shell.env.insert("PS2".into(), "cont> ".into());
        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "JOINED".into()],
                &mut Cursor::new(b"line\\\ncontinued\n".to_vec()),
            )
            .expect("continued read"),
            BuiltinOutcome::Status(0)
        ));
        assert_eq!(shell.get_var("JOINED").as_deref(), Some("linecontinued"));

        shell.options.force_interactive = false;
        let (pieces, hit_delimiter) = read_logical_line(
            &shell,
            ReadOptions { raw: false, delimiter: b'\n' },
            &mut Cursor::new(b"soft\\\nwrap\n".to_vec()),
        )
        .expect("direct read");
        assert!(hit_delimiter);
        assert_eq!(flatten_read_pieces(&pieces), "softwrap");

        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "ESCAPED".into()],
                &mut Cursor::new(b"left\\ right\n".to_vec()),
            )
            .expect("escaped read"),
            BuiltinOutcome::Status(0)
        ));
        assert_eq!(shell.get_var("ESCAPED").as_deref(), Some("left right"));

        assert!(matches!(
            read_with_input(
                &mut shell,
                &["read".into(), "TAIL".into()],
                &mut Cursor::new(b"tail\\".to_vec()),
            )
            .expect("tail read"),
            BuiltinOutcome::Status(1)
        ));
        assert_eq!(shell.get_var("TAIL").as_deref(), Some("tail\\"));

        sys::test_support::with_times_error_for_test(|| {
            assert!(matches!(times(), BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn umask_error_paths_and_symbolic_modes_are_covered() {
        let mut shell = test_shell();
        assert!(matches!(
            run(&mut shell, &["umask".into(), "-Z".into()]).expect("bad option"),
            BuiltinOutcome::Status(1)
        ));
        let saved = sys::current_umask();
        assert!(matches!(
            run(&mut shell, &["umask".into(), "--".into(), "077".into()]).expect("double dash"),
            BuiltinOutcome::Status(0)
        ));
        sys::set_umask(saved);
        assert!(matches!(
            run(&mut shell, &["umask".into(), "077".into(), "022".into()]).expect("too many"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(&mut shell, &["umask".into(), "u+s".into()]).expect("bad symbolic"),
            BuiltinOutcome::Status(1)
        ));

        assert_eq!(parse_umask_mask("-w", 0o022), Some(0o222));
        assert_eq!(parse_umask_mask("a+r", 0o777), Some(0o333));
        assert_eq!(parse_umask_mask("g=u", 0o022), Some(0o002));
        assert_eq!(parse_umask_mask("u!r", 0o022), None);
        assert_eq!(parse_umask_mask(",,", 0o022), None);
    }

    #[test]
    fn exit_and_command_report_expected_results() {
        let mut shell = test_shell();
        let outcome = run(&mut shell, &["exit".into()]).expect("exit");
        assert!(matches!(outcome, BuiltinOutcome::Exit(3)));

        let error = run(&mut shell, &["exit".into(), "bad".into()]).expect_err("bad exit");
        assert_eq!(error.message, "exit: numeric argument required");

        let outcome = run(&mut shell, &["command".into(), "export".into()]).expect("command");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["command".into()]).expect("missing utility");
        assert!(matches!(outcome, BuiltinOutcome::Status(127)));
    }

    #[test]
    fn control_flow_builtins_validate_context_and_arguments() {
        let mut shell = test_shell();
        let error = run(&mut shell, &["return".into()]).expect_err("return outside function");
        assert_eq!(error.message, "return: not in a function");

        shell.function_depth = 1;
        let outcome = run(&mut shell, &["return".into(), "7".into()]).expect("return");
        assert!(matches!(outcome, BuiltinOutcome::Return(7)));
        let error = run(&mut shell, &["return".into(), "bad".into()]).expect_err("bad return");
        assert_eq!(error.message, "return: numeric argument required");
        let error = run(&mut shell, &["return".into(), "1".into(), "2".into()]).expect_err("return args");
        assert_eq!(error.message, "return: too many arguments");

        shell.function_depth = 0;
        let error = run(&mut shell, &["break".into()]).expect_err("break outside loop");
        assert_eq!(error.message, "break: only meaningful in a loop");
        let error = run(&mut shell, &["continue".into()]).expect_err("continue outside loop");
        assert_eq!(error.message, "continue: only meaningful in a loop");

        shell.loop_depth = 2;
        let outcome = run(&mut shell, &["break".into(), "9".into()]).expect("break");
        assert!(matches!(outcome, BuiltinOutcome::Break(2)));
        let outcome = run(&mut shell, &["continue".into(), "2".into()]).expect("continue");
        assert!(matches!(outcome, BuiltinOutcome::Continue(2)));
        let error = run(&mut shell, &["continue".into(), "0".into()]).expect_err("bad continue");
        assert_eq!(error.message, "continue: numeric argument required");
        let error = run(&mut shell, &["break".into(), "1".into(), "2".into()]).expect_err("break args");
        assert_eq!(error.message, "break: too many arguments");
        let error = run(&mut shell, &["continue".into(), "bad".into()]).expect_err("continue numeric");
        assert_eq!(error.message, "continue: numeric argument required");
    }

    #[test]
    fn wait_and_job_control_fail_cleanly_without_jobs() {
        let mut shell = test_shell();
        let wait_outcome = run(&mut shell, &["wait".into(), "%bad".into()]).expect("bad wait");
        assert!(matches!(wait_outcome, BuiltinOutcome::Status(1)));

        let fg_error = run(&mut shell, &["fg".into()]).expect_err("fg");
        assert_eq!(fg_error.message, "fg: no current job");

        let bg_error = run(&mut shell, &["bg".into()]).expect_err("bg");
        assert_eq!(bg_error.message, "bg: no current job");
    }

    #[test]
    fn cd_set_eval_dot_and_exec_noop_paths_work() {
        let _guard = cwd_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut shell = test_shell();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("meiksh-cd-{unique}"));
        fs::create_dir_all(&dir).expect("mkdir");

        let cwd = std::env::current_dir().expect("cwd");
        run(&mut shell, &["cd".into(), dir.display().to_string()]).expect("cd");
        assert_eq!(
            std::fs::canonicalize(std::env::current_dir().expect("cwd")).expect("canonical cwd"),
            std::fs::canonicalize(&dir).expect("canonical dir")
        );
        std::env::set_current_dir(&cwd).expect("restore cwd");
        let _ = fs::remove_dir_all(&dir);

        let outcome = run(&mut shell, &["set".into(), "alpha".into(), "beta".into()]).expect("set");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.positional, vec!["alpha".to_string(), "beta".to_string()]);

        let outcome = run(&mut shell, &["set".into(), "--".into(), "gamma".into(), "delta".into()])
            .expect("set --");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.positional, vec!["gamma".to_string(), "delta".to_string()]);

        let outcome = run(&mut shell, &["set".into(), "-C".into()]).expect("set -C");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.noclobber);

        let outcome = run(&mut shell, &["set".into(), "+C".into()]).expect("set +C");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.noclobber);

        let outcome = run(&mut shell, &["set".into(), "-f".into()]).expect("set -f");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.noglob);

        let outcome = run(&mut shell, &["set".into(), "+f".into()]).expect("set +f");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.noglob);

        let outcome = run(&mut shell, &["set".into(), "-Cf".into()]).expect("set -Cf");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.noclobber);
        assert!(shell.options.noglob);

        let outcome = run(&mut shell, &["set".into(), "-C".into(), "--".into(), "epsilon".into()])
            .expect("set -C --");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.noclobber);
        assert_eq!(shell.positional, vec!["epsilon".to_string()]);

        let outcome = run(&mut shell, &["set".into(), "-a".into()]).expect("set -a");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        run(&mut shell, &["eval".into(), "AUTO=42".into()]).expect("allexport eval");
        assert!(shell.exported.contains("AUTO"));

        let outcome = run(&mut shell, &["set".into(), "-o".into(), "noexec".into()]).expect("set -o noexec");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.syntax_check_only);

        let outcome = run(&mut shell, &["set".into(), "+o".into(), "noexec".into()]).expect("set +o noexec");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.syntax_check_only);

        let outcome = run(&mut shell, &["set".into(), "-n".into()]).expect("set -n");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.syntax_check_only);

        let outcome = run(&mut shell, &["set".into(), "+n".into()]).expect("set +n");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.syntax_check_only);

        let outcome = run(&mut shell, &["set".into(), "-u".into()]).expect("set -u");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.nounset);

        let outcome = run(&mut shell, &["set".into(), "+u".into()]).expect("set +u");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.nounset);

        let outcome = run(&mut shell, &["set".into(), "-v".into()]).expect("set -v");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.verbose);

        let outcome = run(&mut shell, &["set".into(), "+v".into()]).expect("set +v");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(!shell.options.verbose);

        shell.last_status = 0;
        let outcome = run(&mut shell, &["eval".into(), "VALUE=42".into()]).expect("eval");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("42"));

        let script = std::env::temp_dir().join(format!("meiksh-dot-{unique}.sh"));
        fs::write(&script, "FROM_DOT=1\n").expect("write");
        let outcome = run(&mut shell, &[".".into(), script.display().to_string()]).expect("dot");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.get_var("FROM_DOT").as_deref(), Some("1"));
        let _ = fs::remove_file(script);

        let outcome = run(&mut shell, &["exec".into()]).expect("exec no-op");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
    }

    #[test]
    fn set_reports_named_options_and_rejects_invalid_ones() {
        let mut shell = test_shell();
        let report = ProcessCommand::new(&shell.current_exe)
            .args(["-a", "-C", "-c", "set -o"])
            .output()
            .expect("run set -o");
        let stdout = String::from_utf8_lossy(&report.stdout);
        assert!(stdout.contains("allexport on"));
        assert!(stdout.contains("noclobber on"));
        assert!(stdout.contains("noglob off"));
        assert!(stdout.contains("noexec off"));
        assert!(stdout.contains("nounset off"));
        assert!(stdout.contains("verbose off"));

        let restore = ProcessCommand::new(&shell.current_exe)
            .args(["-a", "-C", "-c", "set +o"])
            .output()
            .expect("run set +o");
        let stdout = String::from_utf8_lossy(&restore.stdout);
        assert!(stdout.contains("set -o allexport"));
        assert!(stdout.contains("set -o noclobber"));
        assert!(stdout.contains("set +o noglob"));
        assert!(stdout.contains("set +o noexec"));
        assert!(stdout.contains("set +o nounset"));
        assert!(stdout.contains("set +o verbose"));

        let outcome = run(&mut shell, &["set".into(), "-z".into()]).expect("invalid set");
        assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        let outcome = run(&mut shell, &["set".into(), "-o".into(), "pipefail".into()]).expect("invalid set -o");
        assert!(matches!(outcome, BuiltinOutcome::Status(2)));
    }

    #[test]
    fn dot_requires_filename_and_unknown_builtin_returns_127() {
        let mut shell = test_shell();
        let error = run(&mut shell, &[".".into()]).expect_err("dot missing arg");
        assert_eq!(error.message, ".: filename argument required");

        let outcome = run(&mut shell, &["not-a-builtin".into()]).expect("unknown");
        assert!(matches!(outcome, BuiltinOutcome::Status(127)));
    }

    #[test]
    fn lookup_helpers_cover_reporting_paths() {
        VfsBuilder::new()
            .file_with_mode("/bin/sh", b"", 0o755)
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/definitely/missing".into());
                shell.aliases.insert("ll".into(), "ls -l".into());
                shell.functions.insert(
                    "greet".into(),
                    crate::syntax::Command::Simple(crate::syntax::SimpleCommand {
                        assignments: Vec::new(),
                        words: vec![literal("printf"), literal("hello")],
                        redirections: Vec::new(),
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
            });
    }

    #[test]
    fn command_and_listing_error_paths_are_covered() {
        let _guard = cwd_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());

        assert!(matches!(
            run(&mut shell, &["pwd".into(), "-Z".into()]).expect("pwd invalid"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(&mut shell, &["pwd".into(), "extra".into()]).expect("pwd extra"),
            BuiltinOutcome::Status(1)
        ));

        shell.env.insert("RO".into(), "1".into());
        shell.readonly.insert("RO".into());
        assert!(matches!(
            run(&mut shell, &["unset".into(), "RO".into()]).expect("unset readonly"),
            BuiltinOutcome::Status(1)
        ));

        let export_error = parse_declaration_listing_flag(
            "export",
            &["export".into(), "-p".into(), "NAME".into()],
        )
        .expect_err("export -p operands");
        assert_eq!(export_error.message, "export: -p does not accept operands");

        let readonly_error =
            parse_declaration_listing_flag("readonly", &["readonly".into(), "-x".into()])
                .expect_err("readonly invalid");
        assert_eq!(readonly_error.message, "readonly: invalid option: -x");

        assert_eq!(
            parse_unset_target(&["unset".into(), "--".into(), "NAME".into()]).expect("unset --"),
            (UnsetTarget::Variable, 2)
        );
        let unset_error =
            parse_unset_target(&["unset".into(), "-z".into()]).expect_err("unset invalid");
        assert_eq!(unset_error.message, "unset: invalid option: -z");

        assert_eq!(
            parse_command_options(&["command".into(), "--".into(), "echo".into()]),
            (false, CommandMode::Execute, 2)
        );
        assert_eq!(
            parse_command_options(&["command".into(), "-p".into(), "sh".into()]),
            (true, CommandMode::Execute, 2)
        );
        assert_eq!(command_usage_status(CommandMode::QueryShort), 1);

        assert!(matches!(
            run(
                &mut shell,
                &["command".into(), "-v".into(), "one".into(), "two".into()]
            )
            .expect("command too many args"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(&mut shell, &["command".into(), "-v".into()]).expect("command query missing"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(
                &mut shell,
                &["command".into(), "-v".into(), "meiksh-not-real".into()]
            )
            .expect("command query missing name"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(&mut shell, &["command".into(), "meiksh-not-real".into()]).expect("command missing"),
            BuiltinOutcome::Status(127)
        ));
        assert!(matches!(
            run(&mut shell, &["command".into(), "return".into()]).expect("command builtin error"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(
                &mut shell,
                &["command".into(), "-p".into(), "sh".into(), "-c".into(), "exit 0".into()]
            )
            .expect("command -p"),
            BuiltinOutcome::Status(0)
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

        let sh_path = command_short_description(&shell, "sh", false).expect("command -v sh");
        assert!(Path::new(&sh_path).is_absolute());
        let sh_verbose = command_verbose_description(&shell, "sh", false).expect("command -V sh");
        assert!(sh_verbose.starts_with("sh is /"));
        assert_eq!(
            describe_command(&shell, "command", false),
            Some(CommandDescription::RegularBuiltin)
        );
        assert!(which_in_path("sh", &shell, true).expect("default path sh").is_absolute());
        assert!(which_in_path("./definitely-missing", &shell, false).is_none());
        assert!(absolute_path(Path::new("relative")).expect("absolute path").is_absolute());

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("meiksh-command-errors-{unique}"));
        fs::create_dir_all(&dir).expect("mkdir");

        let noexec = dir.join("plain-file");
        fs::write(&noexec, "echo plain\n").expect("write noexec");
        fs::set_permissions(&noexec, fs::Permissions::from_mode(0o644)).expect("chmod noexec");
        assert!(matches!(
            run(
                &mut shell,
                &["command".into(), noexec.display().to_string()]
            )
            .expect("command plain file"),
            BuiltinOutcome::Status(126)
        ));

        let missing_interp = dir.join("missing-interpreter");
        fs::write(&missing_interp, "#!/definitely/missing-interpreter\n").expect("write script");
        fs::set_permissions(&missing_interp, fs::Permissions::from_mode(0o755))
            .expect("chmod script");
        assert!(matches!(
            run(
                &mut shell,
                &["command".into(), missing_interp.display().to_string()]
            )
            .expect("command missing interpreter"),
            BuiltinOutcome::Status(127)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn command_and_which_cover_real_lookup_path() {
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());

        let path = which("sh", &shell).expect("lookup sh");
        assert!(path.is_absolute());
        assert!(path.ends_with("sh"));

        let outcome = run(&mut shell, &["command".into(), "sh".into(), "-c".into(), "exit 0".into()]).expect("command sh");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
    }

    #[test]
    fn reporting_and_listing_builtins_execute_successfully() {
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
        shell.exported.insert("ONLY_NAME".into());
        shell.aliases.insert("ll".into(), "ls -l".into());

        assert!(matches!(run(&mut shell, &["pwd".into()]).expect("pwd"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["export".into()]).expect("export list"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["readonly".into(), "FLAG".into()]).expect("readonly"), BuiltinOutcome::Status(0)));
        assert!(shell.readonly.contains("FLAG"));
        assert!(matches!(run(&mut shell, &["set".into()]).expect("set list"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["alias".into()]).expect("alias list"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["times".into()]).expect("times"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["trap".into()]).expect("trap"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["trap".into(), "echo".into(), "INT".into()]).expect("trap set"), BuiltinOutcome::Status(0)));
        assert!(matches!(run(&mut shell, &["jobs".into()]).expect("jobs"), BuiltinOutcome::Status(0)));
    }

    #[test]
    fn trap_helpers_cover_listing_reset_and_invalid_paths() {
        let mut shell = test_shell();

        assert_eq!(trap_impl(&mut shell, &["trap".into(), "-p".into()]).expect("trap -p"), 0);
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
        assert_eq!(trap_impl(&mut shell, &["trap".into(), "".into(), "TERM".into()]).expect("trap ignore"), 0);
        assert_eq!(trap_impl(&mut shell, &["trap".into(), "-".into(), "TERM".into()]).expect("trap default"), 0);
        assert_eq!(trap_impl(&mut shell, &["trap".into(), "1".into(), "1".into()]).expect("numeric reset"), 0);
        assert_eq!(
            trap_impl(&mut shell, &["trap".into(), "-p".into(), "EXIT".into(), "INT".into()]).expect("trap -p operands"),
            0
        );
        shell
            .set_trap(TrapCondition::Signal(sys::SIGTERM), Some(TrapAction::Ignore))
            .expect("set ignore");
        shell
            .set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command("printf hi".into())))
            .expect("set command");
        print_traps(&shell, false, &[]).expect("print non-default traps");
        print_traps(&shell, false, &["EXIT".into()]).expect("skip default trap");
        assert!(print_traps(&shell, false, &["BAD".into()]).is_err());
        assert_eq!(trap_impl(&mut shell, &["trap".into(), "printf hi".into(), "BAD".into()]).expect("invalid set"), 1);
        assert_eq!(trap_impl(&mut shell, &["trap".into(), "999".into()]).expect("invalid reset"), 1);
        assert!(trap_impl(&mut shell, &["trap".into(), "printf hi".into()]).is_err());
        assert!(matches!(
            trap(&mut shell, &["trap".into(), "printf hi".into()]),
            BuiltinOutcome::Status(1)
        ));
        assert_eq!(trap_output_action(&shell, TrapCondition::Exit, false, false), None);
        assert!(matches!(parse_wait_operand("bad"), Err(message) if message.contains("invalid process id")));
        assert_eq!(format_trap_condition(TrapCondition::Signal(99)), "99");
        assert_eq!(supported_trap_conditions().len(), 7);
        assert_eq!(parse_trap_condition("BAD"), None);
    }

    #[test]
    fn cd_dash_updates_pwd_and_oldpwd() {
        let _guard = cwd_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::current_dir().expect("cwd");
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("meiksh-m6-builtins-{unique}"));
        let previous = root.join("previous");
        fs::create_dir_all(&previous).expect("mkdir previous");

        let mut shell = test_shell();
        shell.env.insert("OLDPWD".into(), previous.display().to_string());
        shell.env.insert("PWD".into(), original.display().to_string());
        let original_display = original.display().to_string();

        let outcome = run(&mut shell, &["cd".into(), "-".into()]).expect("cd dash");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(
            PathBuf::from(shell.get_var("PWD").expect("pwd after cd"))
                .canonicalize()
                .expect("canonical pwd"),
            std::env::current_dir()
                .expect("cwd after cd")
                .canonicalize()
                .expect("canonical cwd")
        );
        assert_eq!(shell.get_var("OLDPWD").as_deref(), Some(original_display.as_str()));

        std::env::set_current_dir(&original).expect("restore cwd");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cd_helper_argument_paths_are_split_out() {
        let mut shell = test_shell();
        assert!(cd_pwd_value("./relative").is_ok());
        assert_eq!(
            parse_cd_target(&shell, &["cd".into(), "one".into(), "two".into()])
                .expect_err("too many")
                .message,
            "cd: too many arguments"
        );
        assert_eq!(
            parse_cd_target(&shell, &["cd".into()]).expect("default target"),
            (".".to_string(), false)
        );
        assert_eq!(
            parse_cd_target(&shell, &["cd".into(), "-".into()])
                .expect_err("missing oldpwd")
                .message,
            "cd: OLDPWD not set"
        );
        shell.env.insert("OLDPWD".into(), "/tmp/oldpwd".into());
        let error = run(&mut shell, &["cd".into(), "".into()]).expect_err("empty cd");
        assert_eq!(error.message, "cd: empty directory");
        assert!(parse_cd_target(&shell, &["cd".into(), "--".into(), "-".into()]).is_ok());
        assert!(parse_cd_target(&shell, &["cd".into(), "-P".into()]).is_err());

        VfsBuilder::new()
            .dir("/cdpath/target")
            .dir("/work/plain")
            .cwd("/work")
            .run(|| {
                let mut shell = test_shell();

                shell.env.insert("CDPATH".into(), "/cdpath".into());
                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "target", false);
                assert_eq!(resolved, PathBuf::from("/cdpath/target"));
                assert_eq!(pwd_target, "/cdpath/target");
                assert!(should_print);

                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "missing", false);
                assert_eq!(resolved, PathBuf::from("missing"));
                assert_eq!(pwd_target, "missing");
                assert!(!should_print);

                shell.env.remove("CDPATH");
                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);

                shell.env.insert("CDPATH".into(), ":/cdpath".into());
                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "plain", false);
                assert!(resolved.ends_with("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);
            });

        VfsBuilder::new()
            .dir("/cdpath/target")
            .cwd("/work")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("CDPATH".into(), ":/cdpath".into());
                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);
            });
    }

    #[test]
    fn resolve_cd_target_uses_plain_pwd_for_empty_cdpath_prefix() {
        VfsBuilder::new()
            .dir("/work/plain")
            .cwd("/work")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("CDPATH".into(), ":".into());

                let (resolved, pwd_target, should_print) = resolve_cd_target(&shell, "plain", false);
                assert_eq!(resolved, PathBuf::from("./plain"));
                assert_eq!(pwd_target, "plain");
                assert!(!should_print);
            });
    }

    #[test]
    fn dot_path_search_sources_readable_file() {
        VfsBuilder::new()
            .file("/scripts/dot-script.sh", b"M6_DOT=loaded\n")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/scripts".into());
                let status = run(&mut shell, &[".".into(), "dot-script.sh".into()]).expect("dot path");
                assert!(matches!(status, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var("M6_DOT").as_deref(), Some("loaded"));
                assert!(resolve_dot_path(&shell, "missing-dot.sh").is_err());
            });
    }

    #[test]
    fn jobs_option_and_operand_parsing_are_split_out() {
        assert_eq!(
            parse_jobs_options(&["jobs".into(), "-p".into(), "%1".into()]).expect("jobs -p"),
            (true, 2)
        );
        assert_eq!(
            parse_jobs_options(&["jobs".into(), "--".into(), "%1".into()]).expect("jobs --"),
            (false, 2)
        );
        assert_eq!(
            parse_jobs_operands(&["%1".into(), "%2".into()]).expect("job ids"),
            Some(vec![1, 2])
        );
        assert!(parse_jobs_options(&["jobs".into(), "-l".into()]).is_err());
        assert!(parse_jobs_operands(&["bad".into()]).is_err());
    }

    #[test]
    fn job_display_pid_prefers_child_pid_when_no_pgid() {
        let mut shell = test_shell();
        let child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let child_pid = child.id() as sys::Pid;
        shell.launch_background_job("sleep".into(), None, vec![child_to_handle(child)]);
        assert_eq!(job_display_pid(&shell.jobs[0]), Some(child_pid));
    }

    #[test]
    fn jobs_invalid_inputs_return_status_one() {
        let mut shell = test_shell();
        assert!(matches!(
            run(&mut shell, &["jobs".into(), "-l".into()]).expect("bad jobs"),
            BuiltinOutcome::Status(1)
        ));
        assert!(matches!(
            run(&mut shell, &["jobs".into(), "bad".into()]).expect("bad job operand"),
            BuiltinOutcome::Status(1)
        ));
    }

    #[test]
    fn jobs_selected_finished_job_path_is_split_out() {
        let mut shell = test_shell();
        let finished_child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "exit 7"])
            .spawn()
            .expect("spawn finished");
        let finished_handle = child_to_handle(finished_child);
        let _ = sys::wait_pid(finished_handle.pid, false);
        let finished_id = shell.launch_background_job("done".into(), None, vec![finished_handle]);
        let running_child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn running");
        shell.launch_background_job("sleep".into(), None, vec![child_to_handle(running_child)]);
        assert!(matches!(
            jobs(&mut shell, &["jobs".into(), format!("%{finished_id}")]),
            BuiltinOutcome::Status(0)
        ));
    }

    #[test]
    fn unalias_invalid_option_is_split_out() {
        let mut shell = test_shell();
        let error = run(&mut shell, &["unalias".into(), "-x".into()]).expect_err("unalias invalid");
        assert_eq!(error.message, "unalias: invalid option: -x");
    }

    #[test]
    fn unalias_requires_name_after_double_dash() {
        let mut shell = test_shell();
        assert_eq!(
            run(&mut shell, &["unalias".into(), "--".into()])
                .expect_err("unalias -- only")
                .message,
            "unalias: name required"
        );
    }

    #[test]
    fn wait_fg_bg_success_paths_are_exercised() {
        let mut shell = test_shell();
        let child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("sleep".into(), None, vec![child_to_handle(child)]);

        let outcome = run(&mut shell, &["bg".into(), format!("%{id}")]).expect("bg");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["wait".into(), format!("%{id}")]).expect("wait");
        assert!(matches!(outcome, BuiltinOutcome::Status(_)));

        let child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("sleep".into(), None, vec![child_to_handle(child)]);
        let outcome = run(&mut shell, &["fg".into(), format!("%{id}")]).expect("fg");
        assert!(matches!(outcome, BuiltinOutcome::Status(_)));
    }

    #[test]
    fn wait_without_explicit_job_uses_all_jobs() {
        let mut shell = test_shell();
        let child_a = std::process::Command::new(&shell.current_exe)
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        let child_b = std::process::Command::new(&shell.current_exe)
            .args(["-c", "exit 3"])
            .spawn()
            .expect("spawn");
        shell.launch_background_job("first".into(), None, vec![child_to_handle(child_a)]);
        shell.launch_background_job("second".into(), None, vec![child_to_handle(child_b)]);

        let outcome = run(&mut shell, &["wait".into()]).expect("wait all");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.jobs.is_empty());
    }

    #[test]
    fn unset_function_branch_and_exec_error_path_are_covered() {
        let mut shell = test_shell();
        shell.functions.insert(
            "ll".into(),
            crate::syntax::Command::Simple(crate::syntax::SimpleCommand {
                assignments: Vec::new(),
                words: vec![literal("printf"), literal("ok")],
                redirections: Vec::new(),
            }),
        );
        run(&mut shell, &["unset".into(), "-f".into(), "ll".into()]).expect("unset function");
        assert!(!shell.functions.contains_key("ll"));

        let error = run(&mut shell, &["exec".into(), "bad\0program".into()]).expect_err("exec error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn exec_builtin_success_path_can_be_simulated() {
        fn fake_execvp(_file: *const std::os::raw::c_char, _argv: *const *const std::os::raw::c_char) -> i32 {
            0
        }

        crate::sys::test_support::with_execvp_for_test(fake_execvp, || {
            let mut shell = test_shell();
            let outcome = run(&mut shell, &["exec".into(), "echo".into(), "hello".into()]).expect("exec");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn covers_empty_run_and_shift_success() {
        let mut shell = test_shell();
        let outcome = run(&mut shell, &[]).expect("empty argv");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        shell.positional = vec!["a".into(), "b".into()];
        let outcome = run(&mut shell, &["shift".into()]).expect("shift");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.positional, vec!["b".to_string()]);
    }

    #[test]
    fn cd_home_export_name_eval_error_and_dot_missing_are_covered() {
        let _guard = cwd_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut shell = test_shell();

        run(&mut shell, &["export".into(), "ONLY_NAME".into()]).expect("export bare name");
        assert!(shell.exported.contains("ONLY_NAME"));

        let error = run(&mut shell, &["eval".into(), "echo".into(), "'unterminated".into()]).expect_err("bad eval");
        assert!(!error.message.is_empty());

        let error = run(
            &mut shell,
            &[".".into(), "/definitely/missing-meiksh-dot-file".into()],
        )
        .expect_err("missing dot file");
        assert!(!error.message.is_empty());
        let error = run(&mut shell, &[".".into(), "one".into(), "two".into()]).expect_err("dot args");
        assert_eq!(error.message, ".: too many arguments");
    }
}
