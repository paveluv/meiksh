use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::shell::{Shell, ShellError};
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
        "pwd" => pwd()?,
        "exit" => exit(shell, argv)?,
        "export" => export(shell, argv)?,
        "readonly" => readonly(shell, argv)?,
        "unset" => unset(shell, argv)?,
        "set" => set(shell, argv),
        "shift" => shift(shell, argv)?,
        "eval" => eval(shell, argv)?,
        "." => dot(shell, argv)?,
        "exec" => exec_builtin(argv)?,
        "jobs" => jobs(shell),
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

fn cd(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let target = argv
        .get(1)
        .cloned()
        .or_else(|| shell.get_var("HOME"))
        .unwrap_or_else(|| ".".to_string());
    env::set_current_dir(&target)?;
    shell.set_var("PWD", env::current_dir()?.display().to_string())?;
    Ok(BuiltinOutcome::Status(0))
}

fn pwd() -> Result<BuiltinOutcome, ShellError> {
    println!("{}", env::current_dir()?.display());
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
    if argv.len() == 1 {
        for name in &shell.exported {
            if let Some(value) = shell.get_var(name) {
                println!("export {}={}", name, value);
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.export_var(name, Some(value.to_string()))?;
        } else {
            shell.export_var(item, None)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn readonly(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    for item in &argv[1..] {
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
    for item in &argv[1..] {
        if shell.aliases.remove(item).is_none() {
            shell.unset_var(item)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
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
                "--" => {
                    shell.set_positional(argv[index + 1..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
                _ if (arg.starts_with('-') || arg.starts_with('+')) && arg != "-" && arg != "+" => {
                    let enabled = arg.starts_with('-');
                    for ch in arg[1..].chars() {
                        match ch {
                            'C' => shell.options.noclobber = enabled,
                            'f' => shell.options.noglob = enabled,
                            _ => {
                                shell.set_positional(argv[index..].to_vec());
                                return BuiltinOutcome::Status(0);
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
    let status = shell.source_path(&PathBuf::from(path))?;
    Ok(BuiltinOutcome::Status(status))
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

fn jobs(shell: &mut Shell) -> BuiltinOutcome {
    shell.print_jobs();
    BuiltinOutcome::Status(0)
}

fn fg(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let id = argv
        .get(1)
        .and_then(|value| value.trim_start_matches('%').parse::<usize>().ok())
        .or_else(|| shell.jobs.last().map(|job| job.id))
        .ok_or_else(|| ShellError {
            message: "fg: no current job".to_string(),
        })?;
    shell.continue_job(id)?;
    let status = shell.wait_for_job(id)?;
    Ok(BuiltinOutcome::Status(status))
}

fn bg(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let id = argv
        .get(1)
        .and_then(|value| value.trim_start_matches('%').parse::<usize>().ok())
        .or_else(|| shell.jobs.last().map(|job| job.id))
        .ok_or_else(|| ShellError {
            message: "bg: no current job".to_string(),
        })?;
    shell.continue_job(id)?;
    Ok(BuiltinOutcome::Status(0))
}

fn wait(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        let ids: Vec<usize> = shell.jobs.iter().map(|job| job.id).collect();
        let mut last = 0;
        for id in ids {
            last = shell.wait_for_job(id)?;
        }
        return Ok(BuiltinOutcome::Status(last));
    }
    let id = argv[1].trim_start_matches('%').parse::<usize>().map_err(|_| ShellError {
        message: "wait: invalid job id".to_string(),
    })?;
    let status = shell.wait_for_job(id)?;
    Ok(BuiltinOutcome::Status(status))
}

#[derive(Clone, Copy)]
struct ReadOptions {
    raw: bool,
    delimiter: u8,
}

fn read(shell: &mut Shell, argv: &[String]) -> Result<BuiltinOutcome, ShellError> {
    let mut stdin = io::stdin().lock();
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
    for item in &argv[1..] {
        shell.aliases.remove(item);
    }
    Ok(BuiltinOutcome::Status(0))
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

fn trap(_shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    if argv.len() == 1 {
        return BuiltinOutcome::Status(0);
    }
    BuiltinOutcome::Status(0)
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
    let name = argv.get(1).ok_or_else(|| ShellError {
        message: "command: utility name required".to_string(),
    })?;
    if is_builtin(name) {
        println!("{name}");
        return Ok(BuiltinOutcome::Status(0));
    }
    let path = which(name, shell).ok_or_else(|| ShellError {
        message: format!("command: {name}: not found"),
    })?;
    println!("{}", path.display());
    Ok(BuiltinOutcome::Status(0))
}

fn which(name: &str, shell: &Shell) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return path.exists().then_some(path);
    }
    let path_env = shell
        .get_var("PATH")
        .or_else(|| env::var("PATH").ok())
        .unwrap_or_default();
    for dir in path_env.split(':') {
        let path = PathBuf::from(dir).join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use std::collections::{BTreeSet, HashMap};
    use std::fs;
    use std::io::{self, Cursor};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    use crate::test_utils::meiksh_bin_path;

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
            current_exe: meiksh_bin_path(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        }
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
        assert_eq!(shell.aliases.get("ll").map(String::as_str), Some("ls -l"));

        let outcome = run(&mut shell, &["alias".into(), "ll".into()]).expect("alias query");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["alias".into(), "missing".into()]).expect("missing alias");
        assert!(matches!(outcome, BuiltinOutcome::Status(1)));

        run(&mut shell, &["unalias".into(), "ll".into()]).expect("unalias");
        assert!(!shell.aliases.contains_key("ll"));

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

        sys::with_times_error_for_test(|| {
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

        let error = run(&mut shell, &["command".into()]).expect_err("missing utility");
        assert_eq!(error.message, "command: utility name required");
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
        let wait_error = run(&mut shell, &["wait".into(), "%bad".into()]).expect_err("bad wait");
        assert_eq!(wait_error.message, "wait: invalid job id");

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

        let outcome = run(&mut shell, &["set".into(), "+x".into(), "zeta".into()]).expect("set +x");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert_eq!(shell.positional, vec!["+x".to_string(), "zeta".to_string()]);

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
    fn dot_requires_filename_and_unknown_builtin_returns_127() {
        let mut shell = test_shell();
        let error = run(&mut shell, &[".".into()]).expect_err("dot missing arg");
        assert_eq!(error.message, ".: filename argument required");

        let outcome = run(&mut shell, &["not-a-builtin".into()]).expect("unknown");
        assert!(matches!(outcome, BuiltinOutcome::Status(127)));
    }

    #[test]
    fn which_resolves_paths_and_command_reports_missing_binary() {
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/definitely/missing".into());

        let path = which("/bin/sh", &shell).expect("path lookup");
        assert_eq!(path, PathBuf::from("/bin/sh"));

        let error = run(&mut shell, &["command".into(), "meiksh-not-real".into()]).expect_err("missing command");
        assert_eq!(error.message, "command: meiksh-not-real: not found");
    }

    #[test]
    fn command_and_which_cover_real_lookup_path() {
        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());

        let path = which("sh", &shell).expect("lookup sh");
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
    fn wait_fg_bg_success_paths_are_exercised() {
        let mut shell = test_shell();
        let child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("sleep".into(), vec![child]);

        let outcome = run(&mut shell, &["bg".into(), format!("%{id}")]).expect("bg");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["wait".into(), format!("%{id}")]).expect("wait");
        assert!(matches!(outcome, BuiltinOutcome::Status(_)));

        let child = std::process::Command::new(&shell.current_exe)
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("sleep".into(), vec![child]);
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
        shell.launch_background_job("first".into(), vec![child_a]);
        shell.launch_background_job("second".into(), vec![child_b]);

        let outcome = run(&mut shell, &["wait".into()]).expect("wait all");
        assert!(matches!(outcome, BuiltinOutcome::Status(3)));
        assert!(shell.jobs.is_empty());
    }

    #[test]
    fn unset_alias_branch_and_exec_error_path_are_covered() {
        let mut shell = test_shell();
        shell.aliases.insert("ll".into(), "ls -l".into());
        run(&mut shell, &["unset".into(), "ll".into()]).expect("unset alias");
        assert!(!shell.aliases.contains_key("ll"));

        let error = run(&mut shell, &["exec".into(), "bad\0program".into()]).expect_err("exec error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn exec_builtin_success_path_can_be_simulated() {
        fn fake_execvp(_file: *const std::os::raw::c_char, _argv: *const *const std::os::raw::c_char) -> i32 {
            0
        }

        crate::sys::with_execvp_for_test(fake_execvp, || {
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
    }
}
