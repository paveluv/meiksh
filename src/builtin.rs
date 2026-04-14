use std::collections::BTreeSet;

use crate::bstr::{self, BStrExt, ByteWriter};
use crate::shell::{OptionError, Shell, ShellError, TrapAction, TrapCondition, VarError};
use crate::sys;

fn remove_file_bytes(path: &[u8]) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let _ = std::fs::remove_file(OsStr::from_bytes(path));
}

fn write_stderr(msg: &[u8]) {
    let _ = sys::write_all_fd(sys::STDERR_FILENO, msg);
}

fn write_stdout(msg: &[u8]) {
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, msg);
}

fn write_stdout_line(msg: &[u8]) {
    let mut buf = msg.to_vec();
    buf.push(b'\n');
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, &buf);
}

fn diag_status(shell: &Shell, status: i32, msg: &[u8]) -> BuiltinOutcome {
    shell.diagnostic(status, msg);
    BuiltinOutcome::Status(status)
}

fn diag_status_syserr(shell: &Shell, status: i32, prefix: &[u8], e: &sys::SysError) -> BuiltinOutcome {
    let msg = ByteWriter::new()
        .bytes(prefix)
        .bytes(&e.strerror())
        .finish();
    shell.diagnostic(status, &msg);
    BuiltinOutcome::Status(status)
}

fn parse_usize(s: &[u8]) -> Option<usize> {
    let val = bstr::parse_i64(s)?;
    if val >= 0 {
        Some(val as usize)
    } else {
        None
    }
}

fn parse_i32(s: &[u8]) -> Option<i32> {
    let val = bstr::parse_i64(s)?;
    if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
        Some(val as i32)
    } else {
        None
    }
}

fn var_error_msg(prefix: &[u8], e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": readonly variable: ")
            .bytes(name)
            .finish(),
    }
}

fn option_error_msg(prefix: &[u8], e: &OptionError) -> Vec<u8> {
    match e {
        OptionError::InvalidShort(ch) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": invalid option: ")
            .byte(*ch)
            .finish(),
        OptionError::InvalidName(name) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": invalid option: ")
            .bytes(name)
            .finish(),
    }
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
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    if argv.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }

    let outcome = match argv[0].as_slice() {
        b":" | b"true" => BuiltinOutcome::Status(0),
        b"false" => BuiltinOutcome::Status(1),
        b"[" | b"test" => test_builtin(shell, argv)?,
        b"echo" => echo_builtin(shell, argv)?,
        b"printf" => printf_builtin(shell, argv)?,
        b"cd" => cd(shell, argv)?,
        b"pwd" => pwd(shell, argv)?,
        b"exit" => exit(shell, argv)?,
        b"export" => export(shell, argv)?,
        b"readonly" => readonly(shell, argv)?,
        b"unset" => unset(shell, argv)?,
        b"set" => set(shell, argv),
        b"shift" => shift(shell, argv)?,
        b"eval" => eval(shell, argv)?,
        b"." => dot(shell, argv)?,
        b"exec" => exec_builtin(shell, argv, cmd_assignments)?,
        b"jobs" => jobs(shell, argv),
        b"fg" => fg(shell, argv)?,
        b"bg" => bg(shell, argv)?,
        b"wait" => wait(shell, argv)?,
        b"kill" => kill(shell, argv)?,
        b"read" => read(shell, argv)?,
        b"getopts" => getopts(shell, argv)?,
        b"alias" => alias(shell, argv)?,
        b"unalias" => unalias(shell, argv)?,
        b"return" => return_builtin(shell, argv)?,
        b"break" => break_builtin(shell, argv)?,
        b"continue" => continue_builtin(shell, argv)?,
        b"times" => times(shell),
        b"trap" => trap(shell, argv),
        b"umask" => umask(shell, argv)?,
        b"command" => command(shell, argv)?,
        b"type" => type_builtin(shell, argv)?,
        b"hash" => hash(shell, argv)?,
        b"fc" => fc(shell, argv)?,
        b"ulimit" => ulimit(shell, argv)?,
        _ => BuiltinOutcome::Status(127),
    };

    Ok(outcome)
}

const BUILTIN_NAMES: &[&[u8]] = &[
    b".", b":", b"[", b"alias", b"bg", b"break", b"cd", b"command", b"continue", b"echo", b"eval",
    b"exec", b"exit", b"export", b"false", b"fc", b"fg", b"getopts", b"hash", b"jobs", b"kill",
    b"printf", b"pwd", b"read", b"readonly", b"return", b"set", b"shift", b"test", b"times",
    b"trap", b"true", b"type", b"ulimit", b"umask", b"unalias", b"unset", b"wait",
];

pub fn is_builtin(name: &[u8]) -> bool {
    BUILTIN_NAMES.binary_search(&name).is_ok()
}

const DEFAULT_COMMAND_PATH: &[u8] = b"/usr/bin:/bin";

fn cd(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (target, print_new_pwd, physical, check_pwd) = parse_cd_target(shell, argv)?;
    let (resolved_target, _, print_new_pwd) = resolve_cd_target(shell, &target, print_new_pwd);
    let curpath = if physical {
        resolved_target.clone()
    } else {
        cd_logical_curpath(shell, &resolved_target)?
    };

    let old_pwd = current_logical_pwd(shell)?;
    sys::change_dir(&curpath).map_err(|e| shell.diagnostic(1, &e.strerror()))?;

    let new_pwd = if physical {
        match sys::get_cwd() {
            Ok(cwd) => cwd,
            Err(_) if check_pwd => {
                shell
                    .set_var(b"OLDPWD", old_pwd)
                    .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
                return Ok(BuiltinOutcome::Status(1));
            }
            Err(_) => curpath.clone(),
        }
    } else {
        curpath.clone()
    };

    shell
        .set_var(b"OLDPWD", old_pwd)
        .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
    shell
        .set_var(b"PWD", new_pwd.clone())
        .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
    if print_new_pwd {
        write_stdout_line(&new_pwd);
    }
    Ok(BuiltinOutcome::Status(0))
}

fn cd_logical_curpath(shell: &Shell, target: &[u8]) -> Result<Vec<u8>, ShellError> {
    let curpath = if target.first() == Some(&b'/') {
        target.to_vec()
    } else {
        let pwd = current_logical_pwd(shell)?;
        if pwd.last() == Some(&b'/') {
            let mut r = pwd;
            r.extend_from_slice(target);
            r
        } else {
            let mut r = pwd;
            r.push(b'/');
            r.extend_from_slice(target);
            r
        }
    };
    Ok(canonicalize_logical_path(&curpath))
}

fn canonicalize_logical_path(path: &[u8]) -> Vec<u8> {
    let mut components: Vec<&[u8]> = Vec::new();
    for part in path.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return b"/".to_vec();
    }
    let mut result = Vec::new();
    for component in &components {
        result.push(b'/');
        result.extend_from_slice(component);
    }
    result
}

fn parse_cd_target(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<(Vec<u8>, bool, bool, bool), ShellError> {
    let mut index = 1usize;
    let mut physical = false;
    let mut check_pwd = false;
    while let Some(arg) = argv.get(index) {
        if arg == b"--" {
            index += 1;
            break;
        }
        if arg.first() != Some(&b'-') || arg == b"-" {
            break;
        }
        for &ch in &arg[1..] {
            match ch {
                b'L' => {
                    physical = false;
                    check_pwd = false;
                }
                b'P' => physical = true,
                b'e' => check_pwd = true,
                _ => {
                    let msg = ByteWriter::new()
                        .bytes(b"cd: invalid option: -")
                        .byte(ch)
                        .finish();
                    return Err(shell.diagnostic(1, &msg));
                }
            }
        }
        index += 1;
    }
    if !physical {
        check_pwd = false;
    }
    if argv.len() > index + 1 {
        return Err(shell.diagnostic(1, b"cd: too many arguments"));
    }
    let Some(target) = argv.get(index) else {
        let home = shell
            .get_var(b"HOME")
            .ok_or_else(|| shell.diagnostic(1, b"cd: HOME not set"))?;
        return Ok((home.to_vec(), false, physical, check_pwd));
    };
    if target.is_empty() {
        return Err(shell.diagnostic(1, b"cd: empty directory"));
    }
    if target == b"-" {
        return Ok((
            shell
                .get_var(b"OLDPWD")
                .ok_or_else(|| shell.diagnostic(1, b"cd: OLDPWD not set"))?
                .to_vec(),
            true,
            physical,
            check_pwd,
        ));
    }
    Ok((target.clone(), false, physical, check_pwd))
}

fn resolve_cd_target(shell: &Shell, target: &[u8], print_new_pwd: bool) -> (Vec<u8>, Vec<u8>, bool) {
    if print_new_pwd || target.first() == Some(&b'/') {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    }
    let first_component = target.split(|&b| b == b'/').next().unwrap_or(b"");
    if first_component == b"." || first_component == b".." {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    }

    let Some(cdpath) = shell.get_var(b"CDPATH") else {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    };

    for prefix in cdpath.split(|&b| b == b':') {
        let candidate = if prefix.is_empty() {
            let mut c = b"./".to_vec();
            c.extend_from_slice(target);
            c
        } else {
            let mut c = prefix.to_vec();
            c.push(b'/');
            c.extend_from_slice(target);
            c
        };
        if sys::is_directory(&candidate) {
            let should_print = print_new_pwd || !prefix.is_empty();
            let pwd_target = if prefix.is_empty() {
                target.to_vec()
            } else {
                candidate.clone()
            };
            return (candidate, pwd_target, should_print);
        }
    }

    (target.to_vec(), target.to_vec(), print_new_pwd)
}

fn pwd(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut logical = true;
    for arg in &argv[1..] {
        match arg.as_slice() {
            b"-L" => logical = true,
            b"-P" => logical = false,
            _ if arg.first() == Some(&b'-') => {
                let msg = ByteWriter::new()
                    .bytes(b"pwd: invalid option: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
            _ => {
                return Ok(diag_status(shell, 1, b"pwd: too many arguments"));
            }
        }
    }

    let output = pwd_output(shell, logical)?;
    write_stdout_line(&output);
    Ok(BuiltinOutcome::Status(0))
}

fn exit(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let status = match argv.get(1) {
        Some(value) => parse_i32(value)
            .ok_or_else(|| shell.diagnostic(2, b"exit: numeric argument required"))?,
        None => shell.last_status,
    };
    Ok(BuiltinOutcome::Exit(status))
}

fn expand_assignment_tilde(shell: &Shell, value: &[u8]) -> Vec<u8> {
    if value.first() != Some(&b'~') {
        return value.to_vec();
    }
    let slash_pos = value.iter().position(|&b| b == b'/');
    let prefix_end = slash_pos.unwrap_or(value.len());
    let user = &value[1..prefix_end];
    let replacement = if user.is_empty() {
        match shell.get_var(b"HOME") {
            Some(h) => h.to_vec(),
            None => return value.to_vec(),
        }
    } else {
        match sys::home_dir_for_user(user) {
            Some(dir) => dir,
            None => return value.to_vec(),
        }
    };
    let suffix = &value[prefix_end..];
    let mut result = replacement;
    result.extend_from_slice(suffix);
    result
}

fn export(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, b"export", argv)?;
    if print || index == argv.len() {
        for line in exported_lines(shell) {
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            let value = expand_assignment_tilde(shell, value);
            shell.export_var(name, Some(value))?;
        } else {
            shell.export_var(item, None)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn readonly(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, b"readonly", argv)?;
    if print || index == argv.len() {
        for line in readonly_lines(shell) {
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            let value = expand_assignment_tilde(shell, value);
            shell
                .set_var(name, value)
                .map_err(|e| shell.diagnostic(1, &var_error_msg(b"readonly", &e)))?;
            shell.mark_readonly(name);
        } else {
            shell.mark_readonly(item);
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

fn unset(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (target, index) = parse_unset_target(shell, argv)?;
    let mut status = 0;
    for item in &argv[index..] {
        match target {
            UnsetTarget::Variable => {
                if let Err(error) = shell.unset_var(item) {
                    shell.diagnostic(1, &var_error_msg(b"unset", &error));
                    status = 1;
                }
            }
            UnsetTarget::Function => {
                shell.functions.remove(item.as_slice());
            }
        }
    }
    if status != 0 {
        Ok(BuiltinOutcome::UtilityError(status))
    } else {
        Ok(BuiltinOutcome::Status(status))
    }
}

fn set(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.env.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            let quoted = shell_quote_if_needed(value);
            let mut line = name.clone();
            line.push(b'=');
            line.extend_from_slice(&quoted);
            write_stdout_line(&line);
        }
    } else {
        let mut index = 1usize;
        while let Some(arg) = argv.get(index) {
            match arg.as_slice() {
                b"-o" | b"+o" => {
                    let reinput = arg == b"+o";
                    if let Some(name) = argv.get(index + 1) {
                        if let Err(e) = shell.options.set_named_option(name, !reinput) {
                            return BuiltinOutcome::UtilityError(
                                shell.diagnostic(2, &option_error_msg(b"set", &e)).exit_status(),
                            );
                        }
                        index += 2;
                    } else {
                        for (name, enabled) in shell.options.reportable_options() {
                            if reinput {
                                let prefix = if enabled { b'-' } else { b'+' };
                                let line = ByteWriter::new()
                                    .bytes(b"set ")
                                    .byte(prefix)
                                    .bytes(b"o ")
                                    .bytes(name)
                                    .finish();
                                write_stdout_line(&line);
                            } else {
                                let line = ByteWriter::new()
                                    .bytes(name)
                                    .byte(b' ')
                                    .bytes(if enabled { b"on" } else { b"off" })
                                    .finish();
                                write_stdout_line(&line);
                            }
                        }
                        return BuiltinOutcome::Status(0);
                    }
                }
                b"--" => {
                    shell.set_positional(argv[index + 1..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
                _ if (arg.first() == Some(&b'-') || arg.first() == Some(&b'+'))
                    && arg != b"-"
                    && arg != b"+" =>
                {
                    let enabled = arg.first() == Some(&b'-');
                    for &ch in &arg[1..] {
                        if let Err(e) = shell.options.set_short_option(ch, enabled) {
                            return BuiltinOutcome::UtilityError(
                                shell.diagnostic(2, &option_error_msg(b"set", &e)).exit_status(),
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

fn shift(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let count = argv
        .get(1)
        .map(|value| parse_usize(value).ok_or(()))
        .transpose()
        .map_err(|_| shell.diagnostic(1, b"shift: numeric argument required"))?
        .unwrap_or(1);
    if count > shell.positional.len() {
        let msg = ByteWriter::new()
            .bytes(b"shift: ")
            .usize_val(count)
            .bytes(b": shift count out of range")
            .finish();
        return Ok(diag_status(shell, 1, &msg));
    }
    shell.positional.drain(0..count);
    Ok(BuiltinOutcome::Status(0))
}

fn eval(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let source = bstr::join_bstrings(&argv[1..].to_vec(), b" ");
    let status = shell.execute_string(&source)?;
    Ok(BuiltinOutcome::Status(status))
}

fn dot(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let path = argv
        .get(1)
        .ok_or_else(|| shell.diagnostic(2, b".: filename argument required"))?;
    if argv.len() > 2 {
        return Err(shell.diagnostic(2, b".: too many arguments"));
    }
    let resolved = match resolve_dot_path(shell, path) {
        Ok(p) => p,
        Err(_) => {
            let msg = ByteWriter::new()
                .bytes(b".: ")
                .bytes(path)
                .bytes(b": not found")
                .finish();
            return Err(shell.diagnostic(1, &msg));
        }
    };
    let status = shell.source_path(&resolved)?;
    Ok(BuiltinOutcome::Status(status))
}

fn resolve_dot_path(shell: &Shell, path: &[u8]) -> Result<Vec<u8>, ()> {
    if path.contains_byte(b'/') {
        if readable_regular_file(path) {
            return Ok(path.to_vec());
        }
        return Err(());
    }
    search_path(path, shell, false, readable_regular_file).ok_or(())
}

fn exec_builtin(
    shell: &Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    let args = if argv.get(1).map(|s| s.as_slice()) == Some(b"--") {
        &argv[2..]
    } else {
        &argv[1..]
    };
    if args.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }
    if args.iter().any(|s| s.contains_byte(0)) {
        return Err(shell.diagnostic(1, b"exec: invalid argument"));
    }
    let Some(program_path) = which_in_path(&args[0], shell, true) else {
        let msg = ByteWriter::new()
            .bytes(b"exec: ")
            .bytes(&args[0])
            .bytes(b": not found")
            .finish();
        return Err(shell.diagnostic(127, &msg));
    };
    let env = shell.env_for_exec_utility(cmd_assignments);
    sys::exec_replace_with_env(&program_path, &args.to_vec(), &env)
        .map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    Ok(BuiltinOutcome::Status(0))
}

fn return_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if shell.function_depth == 0 && shell.source_depth == 0 {
        return Err(shell.diagnostic(1, b"return: not in a function"));
    }
    if argv.len() > 2 {
        return Err(shell.diagnostic(1, b"return: too many arguments"));
    }
    let status = argv
        .get(1)
        .map(|value| parse_i32(value).ok_or(()))
        .transpose()
        .map_err(|_| shell.diagnostic(1, b"return: numeric argument required"))?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Return(status))
}

fn break_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, b"break: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, b"break", argv)?;
    Ok(BuiltinOutcome::Break(levels.min(shell.loop_depth)))
}

fn continue_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, b"continue: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, b"continue", argv)?;
    Ok(BuiltinOutcome::Continue(levels.min(shell.loop_depth)))
}

fn parse_loop_count(shell: &Shell, name: &[u8], argv: &[Vec<u8>]) -> Result<usize, ShellError> {
    if argv.len() > 2 {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": too many arguments")
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    let levels = argv
        .get(1)
        .map(|value| parse_usize(value).ok_or(()))
        .transpose()
        .map_err(|_| {
            let msg = ByteWriter::new()
                .bytes(name)
                .bytes(b": numeric argument required")
                .finish();
            shell.diagnostic(1, &msg)
        })?
        .unwrap_or(1);
    if levels == 0 {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": numeric argument required")
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    Ok(levels)
}

fn jobs(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
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

fn job_current_marker(id: usize, current: Option<usize>, previous: Option<usize>) -> u8 {
    if Some(id) == current {
        b'+'
    } else if Some(id) == previous {
        b'-'
    } else {
        b' '
    }
}

fn format_job_state(job: &crate::shell::Job) -> (Vec<u8>, Vec<u8>) {
    let pid_str = job_display_pid(job)
        .map(|p| bstr::i64_to_bytes(p as i64))
        .unwrap_or_default();
    let state = match job.state {
        crate::shell::JobState::Running => b"Running".to_vec(),
        crate::shell::JobState::Stopped(sig) => {
            ByteWriter::new()
                .bytes(b"Stopped (")
                .bytes(sys::signal_name(sig))
                .byte(b')')
                .finish()
        }
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
enum JobsMode {
    Normal,
    Long,
    PidOnly,
}

fn parse_jobs_options(argv: &[Vec<u8>]) -> Result<(JobsMode, usize), Vec<u8>> {
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
                    .finish())
            }
        }
        index += 1;
    }
    Ok((mode, index))
}

fn parse_jobs_operands(operands: &[Vec<u8>], shell: &Shell) -> Result<Option<Vec<usize>>, Vec<u8>> {
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

fn job_display_pid(job: &crate::shell::Job) -> Option<sys::Pid> {
    job.pgid
        .or_else(|| job.children.first().map(|child| child.pid))
        .or_else(|| job.last_pid)
}

fn fg(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
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

fn bg(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
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

fn wait(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
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

fn kill(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Ok(diag_status(
            shell,
            2,
            b"kill: usage: kill [-s sigspec | -signum] pid... | -l [exit_status]",
        ));
    }

    let mut args = &argv[1..];
    if args[0] == b"-l" || args[0] == b"-L" {
        if args.len() == 1 {
            let names: Vec<&[u8]> = sys::all_signal_names()
                .iter()
                .map(|(name, _)| {
                    let n = *name;
                    if n.starts_with(b"SIG") { &n[3..] } else { n }
                })
                .collect();
            let line = bstr::join_bytes(&names, b' ');
            write_stdout_line(&line);
            return Ok(BuiltinOutcome::Status(0));
        }
        for arg in &args[1..] {
            if let Some(code) = parse_i32(arg) {
                let sig = if code > 128 { code - 128 } else { code };
                let name = sys::signal_name(sig);
                if name != b"UNKNOWN" {
                    write_stdout_line(&name[3..]);
                } else {
                    let msg = ByteWriter::new()
                        .bytes(b"kill: unknown signal: ")
                        .bytes(arg)
                        .finish();
                    return Ok(diag_status(shell, 1, &msg));
                }
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"kill: invalid exit status: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let mut signal = sys::SIGTERM;
    if args[0] == b"-s" {
        if args.len() < 3 {
            return Ok(diag_status(shell, 2, b"kill: -s requires a signal name"));
        }
        signal = parse_kill_signal(shell, &args[1])?;
        args = &args[2..];
    } else if args[0].first() == Some(&b'-') && args[0] != b"--" {
        let spec = &args[0][1..];
        signal = parse_kill_signal(shell, spec)?;
        args = &args[1..];
    }

    if args.is_empty() || (args.len() == 1 && args[0] == b"--") {
        return Ok(diag_status(shell, 2, b"kill: no process id specified"));
    }
    if args[0] == b"--" {
        args = &args[1..];
    }

    let mut status = 0;
    for operand in args {
        if operand.first() == Some(&b'%') {
            let resolved_id = resolve_job_id(shell, Some(operand));
            let resolved = resolved_id.and_then(|id| shell.jobs.iter().find(|j| j.id == id));
            if let Some(job) = resolved {
                let pid = job
                    .pgid
                    .unwrap_or_else(|| job.children.first().map(|c| c.pid).unwrap_or(0));
                if pid != 0 {
                    if sys::send_signal(-pid, signal).is_err() {
                        let msg = ByteWriter::new()
                            .bytes(b"kill: (")
                            .i64_val(pid as i64)
                            .bytes(b"): No such process")
                            .finish();
                        shell.diagnostic(1, &msg);
                        status = 1;
                    } else if signal == sys::SIGCONT && resolved_id.is_some() {
                        let id = resolved_id.unwrap();
                        if let Some(j) = shell.jobs.iter_mut().find(|j| j.id == id) {
                            j.state = crate::shell::JobState::Running;
                        }
                    }
                }
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"kill: ")
                    .bytes(operand)
                    .bytes(b": no such job")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        } else if let Some(pid) = bstr::parse_i64(operand).map(|v| v as sys::Pid) {
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
                let msg = ByteWriter::new()
                    .bytes(b"kill: (")
                    .i64_val(pid as i64)
                    .bytes(b"): No such process")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        } else {
            let msg = ByteWriter::new()
                .bytes(b"kill: invalid pid: ")
                .bytes(operand)
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn parse_kill_signal(shell: &Shell, spec: &[u8]) -> Result<i32, ShellError> {
    if let Some(num) = bstr::parse_i64(spec) {
        return Ok(num as i32);
    }
    let upper = spec.to_ascii_uppercase();
    let name = if upper.starts_with(b"SIG") {
        &upper[3..]
    } else {
        &upper
    };
    for (n, sig) in sys::all_signal_names() {
        let cmp_name = if n.starts_with(b"SIG") { &n[3..] } else { *n };
        if cmp_name == name {
            return Ok(*sig);
        }
    }
    if name == b"0" {
        return Ok(0);
    }
    let msg = ByteWriter::new()
        .bytes(b"kill: unknown signal: ")
        .bytes(spec)
        .finish();
    Err(shell.diagnostic(1, &msg))
}

#[derive(Clone, Copy)]
struct ReadOptions {
    raw: bool,
    delimiter: u8,
}

fn read(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    read_with_input(shell, argv, sys::STDIN_FILENO)
}

fn read_with_input(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    input_fd: i32,
) -> Result<BuiltinOutcome, ShellError> {
    let Some((options, vars)) = parse_read_options(argv) else {
        return Ok(diag_status(shell, 2, b"read: invalid usage"));
    };
    let vars = if vars.is_empty() {
        vec![b"REPLY".to_vec()]
    } else {
        vars
    };

    let (pieces, hit_delimiter) = match read_logical_line(shell, options, input_fd) {
        Ok(result) => result,
        Err(error) => {
            let msg = ByteWriter::new()
                .bytes(b"read: ")
                .bytes(&error.strerror())
                .finish();
            return Ok(diag_status(shell, 2, &msg));
        }
    };
    let values =
        split_read_assignments(&pieces, &vars, shell.get_var(b"IFS").map(|s| s.to_vec()));
    for (name, value) in vars.iter().zip(values) {
        if let Err(error) = shell.set_var(name, value) {
            let msg = var_error_msg(b"read", &error);
            return Ok(diag_status(shell, 2, &msg));
        }
    }
    Ok(BuiltinOutcome::Status(if hit_delimiter { 0 } else { 1 }))
}

fn parse_read_options(argv: &[Vec<u8>]) -> Option<(ReadOptions, Vec<Vec<u8>>)> {
    let mut options = ReadOptions {
        raw: false,
        delimiter: b'\n',
    };
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"--" => {
                index += 1;
                break;
            }
            b"-r" => {
                options.raw = true;
                index += 1;
            }
            b"-d" => {
                let delim = argv.get(index + 1)?;
                options.delimiter = if delim.is_empty() {
                    0
                } else if delim.len() == 1 {
                    delim[0]
                } else {
                    return None;
                };
                index += 2;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => return None,
            _ => break,
        }
    }
    Some((options, argv[index..].to_vec()))
}

fn read_logical_line(
    shell: &Shell,
    options: ReadOptions,
    input_fd: i32,
) -> sys::SysResult<(Vec<(Vec<u8>, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = Vec::new();
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
                current.push(b'\\');
                push_read_piece(&mut pieces, &mut current, current_quoted);
                return Ok((pieces, false));
            }
            let escaped = byte[0];
            if escaped == b'\n' || escaped == options.delimiter {
                push_read_piece(&mut pieces, &mut current, current_quoted);
                current_quoted = false;
                if shell.is_interactive() {
                    let prompt = shell.get_var(b"PS2").unwrap_or(b"> ");
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, prompt);
                }
                continue;
            }
            push_read_piece(&mut pieces, &mut current, current_quoted);
            current_quoted = true;
            current.push(escaped);
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
        current.push(ch);
    }
}

fn push_read_piece(pieces: &mut Vec<(Vec<u8>, bool)>, current: &mut Vec<u8>, quoted: bool) {
    if current.is_empty() {
        return;
    }
    if let Some((last, last_quoted)) = pieces.last_mut() {
        if *last_quoted == quoted {
            last.extend_from_slice(current);
            current.clear();
            return;
        }
    }
    pieces.push((std::mem::take(current), quoted));
}

fn split_read_assignments(
    pieces: &[(Vec<u8>, bool)],
    vars: &[Vec<u8>],
    ifs_value: Option<Vec<u8>>,
) -> Vec<Vec<u8>> {
    if vars.is_empty() {
        return Vec::new();
    }
    let ifs = ifs_value.unwrap_or_else(|| b" \t\n".to_vec());
    if ifs.is_empty() {
        let mut values = vec![flatten_read_pieces(pieces)];
        values.resize(vars.len(), Vec::new());
        return values;
    }

    let ifs_ws: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|&ch| matches!(ch, b' ' | b'\t' | b'\n'))
        .collect();
    let ifs_other: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|&ch| !matches!(ch, b' ' | b'\t' | b'\n'))
        .collect();
    let chars = flatten_read_chars(pieces);
    if vars.len() == 1 {
        return vec![trim_read_ifs_whitespace(&chars, &ifs_ws)];
    }

    let mut values = Vec::new();
    let mut index = 0usize;
    skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
    while index < chars.len() && values.len() + 1 < vars.len() {
        let mut current = Vec::new();
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
    values.resize(vars.len(), Vec::new());
    values
}

fn flatten_read_pieces(pieces: &[(Vec<u8>, bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (part, _) in pieces {
        out.extend_from_slice(part);
    }
    out
}

fn flatten_read_chars(pieces: &[(Vec<u8>, bool)]) -> Vec<(u8, bool)> {
    let mut chars = Vec::new();
    for (text, quoted) in pieces {
        for &ch in text.iter() {
            chars.push((ch, *quoted));
        }
    }
    chars
}

fn skip_read_ifs_whitespace(chars: &[(u8, bool)], ifs_ws: &[u8], index: &mut usize) {
    while *index < chars.len() && !chars[*index].1 && ifs_ws.contains(&chars[*index].0) {
        *index += 1;
    }
}

fn trim_read_ifs_whitespace(chars: &[(u8, bool)], ifs_ws: &[u8]) -> Vec<u8> {
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

fn getopts_set(shell: &mut Shell, name: &[u8], value: Vec<u8>) -> Result<(), BuiltinOutcome> {
    shell
        .set_var(name, value)
        .map_err(|e| diag_status(shell, 2, &var_error_msg(b"getopts", &e)))
}

fn getopts(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 3 {
        return Ok(diag_status(
            shell,
            2,
            b"getopts: usage: getopts optstring name [arg ...]",
        ));
    }
    let optstring = &argv[1];
    let name = &argv[2];
    let silent = optstring.first() == Some(&b':');
    let opts = if silent {
        &optstring[1..]
    } else {
        optstring.as_slice()
    };

    let params: Vec<Vec<u8>> = if argv.len() > 3 {
        argv[3..].to_vec()
    } else {
        shell.positional.clone()
    };

    let optind: usize = shell
        .get_var(b"OPTIND")
        .and_then(|s| parse_usize(s))
        .unwrap_or(1);

    let charind: usize = shell
        .get_var(b"_GETOPTS_CIND")
        .and_then(|s| parse_usize(s))
        .unwrap_or(0);

    match getopts_inner(shell, name, opts, silent, &params, optind, charind) {
        Ok(status) => Ok(status),
        Err(outcome) => Ok(outcome),
    }
}

fn getopts_inner(
    shell: &mut Shell,
    name: &[u8],
    opts: &[u8],
    silent: bool,
    params: &[Vec<u8>],
    optind: usize,
    charind: usize,
) -> Result<BuiltinOutcome, BuiltinOutcome> {
    if optind < 1 || optind > params.len() {
        getopts_set(shell, name, b"?".to_vec())?;
        let _ = shell.unset_var(b"OPTARG");
        getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((params.len() + 1) as u64))?;
        return Ok(BuiltinOutcome::Status(1));
    }

    let arg = &params[optind - 1];
    let arg_bytes: &[u8] = arg.as_slice();

    if charind == 0 {
        if arg == b"--" {
            getopts_set(shell, name, b"?".to_vec())?;
            let _ = shell.unset_var(b"OPTARG");
            getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 1) as u64))?;
            shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
            return Ok(BuiltinOutcome::Status(1));
        }
        if arg_bytes.len() < 2 || arg_bytes[0] != b'-' {
            getopts_set(shell, name, b"?".to_vec())?;
            let _ = shell.unset_var(b"OPTARG");
            getopts_set(shell, b"OPTIND", bstr::u64_to_bytes(optind as u64))?;
            shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
            return Ok(BuiltinOutcome::Status(1));
        }
    }

    let ci = if charind == 0 { 1 } else { charind };
    let opt_byte = arg_bytes[ci];
    let next_ci = ci + 1;

    if let Some(pos) = opts.iter().position(|&b| b == opt_byte) {
        let takes_arg = opts.get(pos + 1) == Some(&b':');

        if takes_arg {
            if next_ci < arg_bytes.len() {
                let optarg: Vec<u8> = arg_bytes[next_ci..].to_vec();
                getopts_set(shell, b"OPTARG", optarg)?;
                getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 1) as u64))?;
                shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
            } else if optind < params.len() {
                let optarg = params[optind].clone();
                getopts_set(shell, b"OPTARG", optarg)?;
                getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 2) as u64))?;
                shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
            } else {
                if silent {
                    getopts_set(shell, name, b":".to_vec())?;
                    getopts_set(shell, b"OPTARG", vec![opt_byte])?;
                } else {
                    let msg = ByteWriter::new()
                        .bytes(&shell.shell_name)
                        .bytes(b": option requires an argument -- ")
                        .byte(opt_byte)
                        .byte(b'\n')
                        .finish();
                    write_stderr(&msg);
                    getopts_set(shell, name, b"?".to_vec())?;
                    let _ = shell.unset_var(b"OPTARG");
                }
                getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 1) as u64))?;
                shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
                return Ok(BuiltinOutcome::Status(0));
            }
        } else {
            let _ = shell.unset_var(b"OPTARG");
            if next_ci < arg_bytes.len() {
                shell
                    .env
                    .insert(b"_GETOPTS_CIND".to_vec(), bstr::u64_to_bytes(next_ci as u64));
            } else {
                getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 1) as u64))?;
                shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
            }
        }
        getopts_set(shell, name, vec![opt_byte])?;
        Ok(BuiltinOutcome::Status(0))
    } else {
        if silent {
            getopts_set(shell, b"OPTARG", vec![opt_byte])?;
        } else {
            let msg = ByteWriter::new()
                .bytes(&shell.shell_name)
                .bytes(b": illegal option -- ")
                .byte(opt_byte)
                .byte(b'\n')
                .finish();
            write_stderr(&msg);
            let _ = shell.unset_var(b"OPTARG");
        }
        getopts_set(shell, name, b"?".to_vec())?;
        if next_ci < arg_bytes.len() {
            shell
                .env
                .insert(b"_GETOPTS_CIND".to_vec(), bstr::u64_to_bytes(next_ci as u64));
        } else {
            getopts_set(shell, b"OPTIND", bstr::u64_to_bytes((optind + 1) as u64))?;
            shell.env.remove(b"_GETOPTS_CIND" as &[u8]);
        }
        Ok(BuiltinOutcome::Status(0))
    }
}

fn alias(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.aliases.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            let line = format_alias_definition(name, value);
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            shell.aliases.insert(name.into(), value.into());
        } else if let Some(value) = shell.aliases.get(item.as_slice()) {
            let line = format_alias_definition(item, value);
            write_stdout_line(&line);
        } else {
            let msg = ByteWriter::new()
                .bytes(b"alias: ")
                .bytes(item)
                .bytes(b": not found")
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn format_alias_definition(name: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    out.push(b'=');
    out.extend_from_slice(&shell_quote(value));
    out
}

fn shell_quote(value: &[u8]) -> Vec<u8> {
    if value.is_empty() {
        return b"''".to_vec();
    }
    let mut out = Vec::new();
    out.push(b'\'');
    for &b in value {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
    out.push(b'\'');
    out
}

fn needs_quoting(value: &[u8]) -> bool {
    value.is_empty()
        || value.iter().any(|&b| {
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

fn shell_quote_if_needed(value: &[u8]) -> Vec<u8> {
    if needs_quoting(value) {
        shell_quote(value)
    } else {
        value.to_vec()
    }
}

fn unalias(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Err(shell.diagnostic(1, b"unalias: name required"));
    }
    if argv.len() == 2 && argv[1] == b"-a" {
        shell.aliases.clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv[1].first() == Some(&b'-') && argv[1] != b"-" && argv[1] != b"--" {
        let msg = ByteWriter::new()
            .bytes(b"unalias: invalid option: ")
            .bytes(&argv[1])
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    let start = usize::from(argv[1] == b"--") + 1;
    if start >= argv.len() {
        return Err(shell.diagnostic(1, b"unalias: name required"));
    }
    let mut status = 0;
    for item in &argv[start..] {
        if shell.aliases.remove(item.as_slice()).is_none() {
            let msg = ByteWriter::new()
                .bytes(b"unalias: ")
                .bytes(item)
                .bytes(b": not found")
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn times(shell: &Shell) -> BuiltinOutcome {
    match (sys::process_times(), sys::clock_ticks_per_second()) {
        (Ok(times), Ok(ticks_per_second)) => {
            let line1 = ByteWriter::new()
                .bytes(&format_times_value(times.user_ticks, ticks_per_second))
                .byte(b' ')
                .bytes(&format_times_value(times.system_ticks, ticks_per_second))
                .finish();
            write_stdout_line(&line1);
            let line2 = ByteWriter::new()
                .bytes(&format_times_value(times.child_user_ticks, ticks_per_second))
                .byte(b' ')
                .bytes(&format_times_value(times.child_system_ticks, ticks_per_second))
                .finish();
            write_stdout_line(&line2);
            BuiltinOutcome::Status(0)
        }
        (Err(error), _) | (_, Err(error)) => {
            diag_status_syserr(shell, 1, b"times: ", &error)
        }
    }
}

fn trap(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
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

fn resolve_job_id(shell: &Shell, operand: Option<&[u8]>) -> Option<usize> {
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

fn parse_wait_operand(operand: &[u8], shell: &Shell) -> Result<WaitOperand, Vec<u8>> {
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

fn trap_impl(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<i32, ShellError> {
    if argv.len() == 1 {
        print_traps(shell, false, &[])?;
        return Ok(0);
    }
    if argv[1] == b"-p" {
        print_traps(shell, true, &argv[2..])?;
        return Ok(0);
    }
    let (action_index, conditions_start) = if argv[1] == b"--" {
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
                let msg = ByteWriter::new()
                    .bytes(b"trap: invalid condition: ")
                    .bytes(condition)
                    .finish();
                shell.diagnostic(1, &msg);
                return Ok(1);
            }
        }
        return Ok(0);
    }
    let action = &argv[action_index];
    if argv.len() <= conditions_start {
        return Err(shell.diagnostic(1, b"trap: condition argument required"));
    }
    let trap_action = parse_trap_action(action);
    let mut status = 0;
    for condition in &argv[conditions_start..] {
        let Some(condition) = parse_trap_condition(condition) else {
            let msg = ByteWriter::new()
                .bytes(b"trap: invalid condition: ")
                .bytes(condition)
                .finish();
            shell.diagnostic(1, &msg);
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
    operands: &[Vec<u8>],
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
                let msg = ByteWriter::new()
                    .bytes(b"trap: invalid condition: ")
                    .bytes(operand)
                    .finish();
                return Err(shell.diagnostic(1, &msg));
            };
            parsed.push(condition);
        }
        parsed
    };
    for condition in conditions {
        if let Some(action) =
            trap_output_action(shell, condition, include_defaults, !operands.is_empty())
        {
            let line = ByteWriter::new()
                .bytes(b"trap -- ")
                .bytes(&action)
                .byte(b' ')
                .bytes(&format_trap_condition(condition))
                .finish();
            write_stdout_line(&line);
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

fn parse_trap_action(action: &[u8]) -> Option<TrapAction> {
    match action {
        b"-" => None,
        _ if action.is_empty() => Some(TrapAction::Ignore),
        _ => Some(TrapAction::Command(action.into())),
    }
}

fn parse_trap_condition(text: &[u8]) -> Option<TrapCondition> {
    let name = if text.starts_with(b"SIG") {
        &text[3..]
    } else {
        text
    };
    match name {
        b"0" | b"EXIT" => Some(TrapCondition::Exit),
        b"HUP" | b"1" => Some(TrapCondition::Signal(sys::SIGHUP)),
        b"INT" | b"2" => Some(TrapCondition::Signal(sys::SIGINT)),
        b"QUIT" | b"3" => Some(TrapCondition::Signal(sys::SIGQUIT)),
        b"ILL" | b"4" => Some(TrapCondition::Signal(sys::SIGILL)),
        b"ABRT" | b"6" => Some(TrapCondition::Signal(sys::SIGABRT)),
        b"FPE" | b"8" => Some(TrapCondition::Signal(sys::SIGFPE)),
        b"KILL" | b"9" => Some(TrapCondition::Signal(sys::SIGKILL)),
        b"USR1" | b"10" => Some(TrapCondition::Signal(sys::SIGUSR1)),
        b"SEGV" | b"11" => Some(TrapCondition::Signal(sys::SIGSEGV)),
        b"USR2" | b"12" => Some(TrapCondition::Signal(sys::SIGUSR2)),
        b"PIPE" | b"13" => Some(TrapCondition::Signal(sys::SIGPIPE)),
        b"ALRM" | b"14" => Some(TrapCondition::Signal(sys::SIGALRM)),
        b"TERM" | b"15" => Some(TrapCondition::Signal(sys::SIGTERM)),
        b"CHLD" | b"17" => Some(TrapCondition::Signal(sys::SIGCHLD)),
        b"STOP" | b"19" => Some(TrapCondition::Signal(sys::SIGSTOP)),
        b"CONT" | b"18" => Some(TrapCondition::Signal(sys::SIGCONT)),
        b"TRAP" | b"5" => Some(TrapCondition::Signal(sys::SIGTRAP)),
        b"TSTP" | b"20" => Some(TrapCondition::Signal(sys::SIGTSTP)),
        b"TTIN" | b"21" => Some(TrapCondition::Signal(sys::SIGTTIN)),
        b"TTOU" | b"22" => Some(TrapCondition::Signal(sys::SIGTTOU)),
        b"BUS" => Some(TrapCondition::Signal(sys::SIGBUS)),
        b"SYS" => Some(TrapCondition::Signal(sys::SIGSYS)),
        _ => None,
    }
}

fn format_trap_condition(condition: TrapCondition) -> Vec<u8> {
    match condition {
        TrapCondition::Exit => b"EXIT".to_vec(),
        TrapCondition::Signal(sys::SIGHUP) => b"HUP".to_vec(),
        TrapCondition::Signal(sys::SIGINT) => b"INT".to_vec(),
        TrapCondition::Signal(sys::SIGQUIT) => b"QUIT".to_vec(),
        TrapCondition::Signal(sys::SIGILL) => b"ILL".to_vec(),
        TrapCondition::Signal(sys::SIGABRT) => b"ABRT".to_vec(),
        TrapCondition::Signal(sys::SIGFPE) => b"FPE".to_vec(),
        TrapCondition::Signal(sys::SIGKILL) => b"KILL".to_vec(),
        TrapCondition::Signal(sys::SIGUSR1) => b"USR1".to_vec(),
        TrapCondition::Signal(sys::SIGSEGV) => b"SEGV".to_vec(),
        TrapCondition::Signal(sys::SIGUSR2) => b"USR2".to_vec(),
        TrapCondition::Signal(sys::SIGPIPE) => b"PIPE".to_vec(),
        TrapCondition::Signal(sys::SIGALRM) => b"ALRM".to_vec(),
        TrapCondition::Signal(sys::SIGTERM) => b"TERM".to_vec(),
        TrapCondition::Signal(sys::SIGCHLD) => b"CHLD".to_vec(),
        TrapCondition::Signal(sys::SIGCONT) => b"CONT".to_vec(),
        TrapCondition::Signal(sys::SIGTRAP) => b"TRAP".to_vec(),
        TrapCondition::Signal(sys::SIGTSTP) => b"TSTP".to_vec(),
        TrapCondition::Signal(sys::SIGTTIN) => b"TTIN".to_vec(),
        TrapCondition::Signal(sys::SIGTTOU) => b"TTOU".to_vec(),
        TrapCondition::Signal(sys::SIGBUS) => b"BUS".to_vec(),
        TrapCondition::Signal(sys::SIGSYS) => b"SYS".to_vec(),
        TrapCondition::Signal(signal) => bstr::i64_to_bytes(signal as i64),
    }
}

fn trap_output_action(
    shell: &Shell,
    condition: TrapCondition,
    include_defaults: bool,
    explicit_operand: bool,
) -> Option<Vec<u8>> {
    let action = shell
        .subshell_saved_traps
        .as_ref()
        .and_then(|saved| saved.get(&condition))
        .or_else(|| shell.trap_action(condition));
    match action {
        Some(TrapAction::Ignore) => Some(b"''".to_vec()),
        Some(TrapAction::Command(command)) => Some(shell_quote(command)),
        None if include_defaults || explicit_operand => Some(b"-".to_vec()),
        None => None,
    }
}

fn is_unsigned_decimal(text: &[u8]) -> bool {
    !text.is_empty() && text.iter().all(|&ch| ch.is_ascii_digit())
}

fn format_times_value(ticks: u64, ticks_per_second: u64) -> Vec<u8> {
    let total_seconds = ticks as f64 / ticks_per_second as f64;
    let minutes = (total_seconds / 60.0).floor() as u64;
    let seconds = total_seconds - (minutes * 60) as f64;
    ByteWriter::new()
        .u64_val(minutes)
        .byte(b'm')
        .f64_fixed(seconds, 2)
        .byte(b's')
        .finish()
}

fn umask(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut symbolic_output = false;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"-S" => {
                symbolic_output = true;
                index += 1;
            }
            b"--" => {
                index += 1;
                break;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => {
                let msg = ByteWriter::new()
                    .bytes(b"umask: invalid option: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
            _ => break,
        }
    }

    let current = sys::current_umask() as u16;
    if index == argv.len() {
        if symbolic_output {
            write_stdout_line(&format_umask_symbolic(current));
        } else {
            let line = ByteWriter::new()
                .octal_padded(current as u64, 4)
                .finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    if index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, b"umask: too many arguments"));
    }

    let Some(mask) = parse_umask_mask(&argv[index], current) else {
        let msg = ByteWriter::new()
            .bytes(b"umask: invalid mask: ")
            .bytes(&argv[index])
            .finish();
        return Ok(diag_status(shell, 1, &msg));
    };
    sys::set_umask(mask as sys::FileModeMask);
    Ok(BuiltinOutcome::Status(0))
}

fn parse_umask_mask(mask: &[u8], current_mask: u16) -> Option<u16> {
    if !mask.is_empty() && mask.iter().all(|&ch| matches!(ch, b'0'..=b'7')) {
        let mut val = 0u16;
        for &ch in mask {
            val = val * 8 + (ch - b'0') as u16;
        }
        return Some(val & 0o777);
    }
    parse_symbolic_umask(mask, current_mask)
}

fn parse_symbolic_umask(mask: &[u8], current_mask: u16) -> Option<u16> {
    let mut allowed = (!current_mask) & 0o777;
    for clause in mask.split(|&b| b == b',') {
        if clause.is_empty() {
            return None;
        }
        let (targets, op, perms) = parse_symbolic_clause(clause)?;
        let perm_bits = symbolic_permission_bits(perms, targets, allowed)?;
        if op == b'+' {
            allowed |= perm_bits;
        } else if op == b'-' {
            allowed &= !perm_bits;
        } else {
            allowed = (allowed & !targets) | (perm_bits & targets);
        }
    }
    Some((!allowed) & 0o777)
}

fn parse_symbolic_clause(clause: &[u8]) -> Option<(u16, u8, &[u8])> {
    let mut split_at = 0usize;
    for &ch in clause {
        if matches!(ch, b'u' | b'g' | b'o' | b'a') {
            split_at += 1;
        } else {
            break;
        }
    }
    let (who_text, rest) = clause.split_at(split_at);
    if rest.is_empty() {
        return None;
    }
    let op = rest[0];
    if !matches!(op, b'+' | b'-' | b'=') {
        return None;
    }
    let perms = &rest[1..];
    Some((parse_symbolic_targets(who_text), op, perms))
}

fn parse_symbolic_targets(who_text: &[u8]) -> u16 {
    if who_text.is_empty() {
        return 0o777;
    }
    let mut targets = 0u16;
    for &ch in who_text {
        match ch {
            b'u' => targets |= 0o700,
            b'g' => targets |= 0o070,
            b'o' => targets |= 0o007,
            b'a' => targets |= 0o777,
            _ => {}
        }
    }
    targets
}

fn symbolic_permission_bits(perms: &[u8], targets: u16, allowed: u16) -> Option<u16> {
    let mut bits = 0u16;
    for &ch in perms {
        bits |= match ch {
            b'r' => permission_bits_for_targets(targets, 0o444),
            b'w' => permission_bits_for_targets(targets, 0o222),
            b'x' => permission_bits_for_targets(targets, 0o111),
            b'X' => permission_bits_for_targets(targets, 0o111),
            b's' => 0,
            b'u' => copy_permission_bits(allowed, targets, 0o700),
            b'g' => copy_permission_bits(allowed, targets, 0o070),
            b'o' => copy_permission_bits(allowed, targets, 0o007),
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

fn format_umask_symbolic(mask: u16) -> Vec<u8> {
    ByteWriter::new()
        .bytes(b"u=")
        .bytes(&symbolic_permissions_for_class(mask, 0o700, 6))
        .bytes(b",g=")
        .bytes(&symbolic_permissions_for_class(mask, 0o070, 3))
        .bytes(b",o=")
        .bytes(&symbolic_permissions_for_class(mask, 0o007, 0))
        .finish()
}

fn symbolic_permissions_for_class(mask: u16, class_mask: u16, shift: u16) -> Vec<u8> {
    let allowed = ((!mask) & class_mask) >> shift;
    let mut result = Vec::new();
    if allowed & 0b100 != 0 {
        result.push(b'r');
    }
    if allowed & 0b010 != 0 {
        result.push(b'w');
    }
    if allowed & 0b001 != 0 {
        result.push(b'x');
    }
    result
}

fn command(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (use_default_path, mode, index) = parse_command_options(argv);
    let Some(name) = argv.get(index) else {
        return Ok(diag_status(
            shell,
            command_usage_status(mode),
            b"command: utility name required",
        ));
    };

    if mode != CommandMode::Execute && index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, b"command: too many arguments"));
    }

    match mode {
        CommandMode::QueryShort => {
            let Some(line) = command_short_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            write_stdout_line(&line);
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::QueryVerbose => {
            let Some(line) = command_verbose_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            write_stdout_line(&line);
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::Execute => execute_command_utility(shell, &argv[index..], use_default_path),
    }
}

#[cfg(test)]
fn which(name: &[u8], shell: &Shell) -> Option<Vec<u8>> {
    which_in_path(name, shell, false)
}

fn parse_declaration_listing_flag(
    shell: &Shell,
    name: &[u8],
    argv: &[Vec<u8>],
) -> Result<(bool, usize), ShellError> {
    if argv.len() >= 2 && argv[1] == b"-p" {
        if argv.len() > 2 {
            let msg = ByteWriter::new()
                .bytes(name)
                .bytes(b": -p does not accept operands")
                .finish();
            return Err(shell.diagnostic(1, &msg));
        }
        return Ok((true, 2));
    }
    if let Some(arg) = argv.get(1)
        && arg.first() == Some(&b'-')
        && arg != b"-"
        && arg != b"--"
    {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": invalid option: ")
            .bytes(arg)
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    Ok((false, 1))
}

fn exported_lines(shell: &Shell) -> Vec<Vec<u8>> {
    shell
        .exported
        .iter()
        .map(|name| declaration_line(b"export", name, shell.get_var(name)))
        .collect()
}

fn readonly_lines(shell: &Shell) -> Vec<Vec<u8>> {
    shell
        .readonly
        .iter()
        .map(|name| declaration_line(b"readonly", name, shell.get_var(name)))
        .collect()
}

fn declaration_line(prefix: &[u8], name: &[u8], value: Option<&[u8]>) -> Vec<u8> {
    match value {
        Some(value) => {
            let mut out = prefix.to_vec();
            out.push(b' ');
            out.extend_from_slice(name);
            out.push(b'=');
            out.extend_from_slice(&shell_quote(value));
            out
        }
        None => {
            let mut out = prefix.to_vec();
            out.push(b' ');
            out.extend_from_slice(name);
            out
        }
    }
}

fn pwd_output(shell: &Shell, logical: bool) -> Result<Vec<u8>, ShellError> {
    if logical {
        return current_logical_pwd(shell);
    }
    sys::get_cwd().map_err(|e| shell.diagnostic(1, &e.strerror()))
}

fn current_logical_pwd(shell: &Shell) -> Result<Vec<u8>, ShellError> {
    let cwd = sys::get_cwd().map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    if let Some(pwd) = shell.get_var(b"PWD")
        && logical_pwd_is_valid(pwd)
        && paths_match_logically(pwd, &cwd)
    {
        return Ok(pwd.to_vec());
    }
    Ok(cwd)
}

fn logical_pwd_is_valid(path: &[u8]) -> bool {
    if path.first() != Some(&b'/') {
        return false;
    }
    for component in path.split(|&b| b == b'/') {
        if component == b"." || component == b".." {
            return false;
        }
    }
    true
}

fn paths_match_logically(lhs: &[u8], rhs: &[u8]) -> bool {
    sys::canonicalize(lhs).ok() == sys::canonicalize(rhs).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsetTarget {
    Variable,
    Function,
}

fn parse_unset_target(shell: &Shell, argv: &[Vec<u8>]) -> Result<(UnsetTarget, usize), ShellError> {
    let mut target = UnsetTarget::Variable;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        if arg.first() != Some(&b'-') || arg == b"-" {
            break;
        }
        if arg == b"--" {
            index += 1;
            break;
        }
        for &ch in &arg[1..] {
            match ch {
                b'v' => target = UnsetTarget::Variable,
                b'f' => target = UnsetTarget::Function,
                _ => {
                    let msg = ByteWriter::new()
                        .bytes(b"unset: invalid option: -")
                        .byte(ch)
                        .finish();
                    return Err(shell.diagnostic(1, &msg));
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
    Alias(Vec<u8>),
    Function,
    SpecialBuiltin,
    RegularBuiltin,
    ReservedWord,
    External(Vec<u8>),
}

fn parse_command_options(argv: &[Vec<u8>]) -> (bool, CommandMode, usize) {
    let mut use_default_path = false;
    let mut mode = CommandMode::Execute;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"-p" => {
                use_default_path = true;
                index += 1;
            }
            b"-v" => {
                mode = CommandMode::QueryShort;
                index += 1;
            }
            b"-V" => {
                mode = CommandMode::QueryVerbose;
                index += 1;
            }
            b"--" => {
                index += 1;
                break;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => break,
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

fn command_short_description(shell: &Shell, name: &[u8], use_default_path: bool) -> Option<Vec<u8>> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => {
            let mut out = b"alias ".to_vec();
            out.extend_from_slice(&format_alias_definition(name, &value));
            Some(out)
        }
        CommandDescription::Function
        | CommandDescription::SpecialBuiltin
        | CommandDescription::RegularBuiltin
        | CommandDescription::ReservedWord => Some(name.to_vec()),
        CommandDescription::External(path) => Some(path),
    }
}

fn command_verbose_description(
    shell: &Shell,
    name: &[u8],
    use_default_path: bool,
) -> Option<Vec<u8>> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is an alias for ");
            out.extend_from_slice(&shell_quote(&value));
            Some(out)
        }
        CommandDescription::Function => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a function");
            Some(out)
        }
        CommandDescription::SpecialBuiltin => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a special built-in utility");
            Some(out)
        }
        CommandDescription::RegularBuiltin => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a regular built-in utility");
            Some(out)
        }
        CommandDescription::ReservedWord => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a reserved word");
            Some(out)
        }
        CommandDescription::External(path) => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is ");
            out.extend_from_slice(&path);
            Some(out)
        }
    }
}

fn describe_command(
    shell: &Shell,
    name: &[u8],
    use_default_path: bool,
) -> Option<CommandDescription> {
    if let Some(value) = shell.aliases.get(name) {
        return Some(CommandDescription::Alias(value.to_vec()));
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

fn type_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut status = 0;
    for name in &argv[1..] {
        match command_verbose_description(shell, name, false) {
            Some(desc) => write_stdout_line(&desc),
            None => {
                let msg = ByteWriter::new()
                    .bytes(name)
                    .bytes(b": not found")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn hash(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() >= 2 && argv[1] == b"-r" {
        shell.path_cache.clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv.len() == 1 {
        if shell.path_cache.is_empty() {
            return Ok(BuiltinOutcome::Status(0));
        }
        for (name, path) in &shell.path_cache {
            let line = ByteWriter::new()
                .bytes(name)
                .byte(b'\t')
                .bytes(path)
                .finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for name in &argv[1..] {
        if is_builtin(name) || shell.functions.contains_key(name.as_slice()) {
            continue;
        }
        match search_path(name, shell, false, |p| {
            sys::access_path(p, sys::X_OK).is_ok()
        }) {
            Some(path) => {
                shell.path_cache.insert(name.as_slice().into(), path);
            }
            None => {
                let msg = ByteWriter::new()
                    .bytes(b"hash: ")
                    .bytes(name)
                    .bytes(b": not found")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

fn ulimit_resource_for_option(ch: u8) -> Option<(i32, &'static [u8], u64)> {
    match ch {
        b'c' => Some((sys::RLIMIT_CORE, b"core file size (blocks)", 512)),
        b'd' => Some((sys::RLIMIT_DATA, b"data seg size (kbytes)", 1024)),
        b'f' => Some((sys::RLIMIT_FSIZE, b"file size (blocks)", 512)),
        b'n' => Some((sys::RLIMIT_NOFILE, b"open files", 1)),
        b's' => Some((sys::RLIMIT_STACK, b"stack size (kbytes)", 1024)),
        b't' => Some((sys::RLIMIT_CPU, b"cpu time (seconds)", 1)),
        b'v' => Some((sys::RLIMIT_AS, b"virtual memory (kbytes)", 1024)),
        _ => None,
    }
}

fn format_limit(val: u64) -> Vec<u8> {
    if val == sys::RLIM_INFINITY {
        b"unlimited".to_vec()
    } else {
        bstr::u64_to_bytes(val)
    }
}

fn ulimit(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut use_hard = false;
    let mut use_soft = false;
    let mut report_all = false;
    let mut resource_opt: Option<u8> = None;
    let mut new_limit: Option<&[u8]> = None;

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg.first() == Some(&b'-') && arg.len() > 1 {
            for &ch in &arg[1..] {
                match ch {
                    b'H' => use_hard = true,
                    b'S' => use_soft = true,
                    b'a' => report_all = true,
                    b'c' | b'd' | b'f' | b'n' | b's' | b't' | b'v' => resource_opt = Some(ch),
                    _ => {
                        let msg = ByteWriter::new()
                            .bytes(b"ulimit: invalid option: -")
                            .byte(ch)
                            .finish();
                        return Err(shell.diagnostic(2, &msg));
                    }
                }
            }
        } else {
            new_limit = Some(arg);
        }
        i += 1;
    }

    if !use_hard && !use_soft {
        use_soft = true;
    }

    if report_all {
        for &opt in &[b'c', b'd', b'f', b'n', b's', b't', b'v'] {
            let (resource, desc, unit) = ulimit_resource_for_option(opt).unwrap();
            let (soft, hard) = sys::getrlimit(resource)
                .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
            let val = if use_hard { hard } else { soft };
            let display = if val == sys::RLIM_INFINITY {
                b"unlimited".to_vec()
            } else {
                bstr::u64_to_bytes(val / unit)
            };
            let line = ByteWriter::new()
                .byte(b'-')
                .byte(opt)
                .bytes(b": ")
                .bytes(desc)
                .bytes(&vec![b' '; 40usize.saturating_sub(desc.len())])
                .byte(b' ')
                .bytes(&display)
                .finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let opt = resource_opt.unwrap_or(b'f');
    let (resource, _desc, unit) = ulimit_resource_for_option(opt).unwrap();

    if let Some(val_str) = new_limit {
        let (soft, hard) = sys::getrlimit(resource)
            .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
        let raw_val = if val_str == b"unlimited" {
            sys::RLIM_INFINITY
        } else {
            let Some(n) = bstr::parse_i64(val_str).filter(|&v| v >= 0) else {
                let msg = ByteWriter::new()
                    .bytes(b"ulimit: invalid limit: ")
                    .bytes(val_str)
                    .finish();
                return Err(shell.diagnostic(2, &msg));
            };
            n as u64 * unit
        };
        let new_soft = if use_soft { raw_val } else { soft };
        let new_hard = if use_hard { raw_val } else { hard };
        sys::setrlimit(resource, new_soft, new_hard)
            .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
        return Ok(BuiltinOutcome::Status(0));
    }

    let (soft, hard) =
        sys::getrlimit(resource).map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
    let val = if use_hard { hard } else { soft };
    let display = format_limit(if val == sys::RLIM_INFINITY {
        val
    } else {
        val / unit
    });
    write_stdout_line(&display);
    Ok(BuiltinOutcome::Status(0))
}

fn fc_resolve_operand(history: &[Box<[u8]>], op: &[u8]) -> Option<usize> {
    if let Some(n) = bstr::parse_i64(op) {
        if n > 0 {
            let idx = (n as usize).saturating_sub(1);
            return if idx < history.len() { Some(idx) } else { None };
        }
        let offset = n.unsigned_abs() as usize;
        return history.len().checked_sub(offset);
    }
    history.iter().rposition(|entry| entry.starts_with(op))
}

fn fc(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut list_mode = false;
    let mut suppress_numbers = false;
    let mut reverse = false;
    let mut reexec = false;
    let mut editor: Option<Vec<u8>> = None;
    let mut operands = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == b"--" {
            i += 1;
            operands.extend(argv[i..].iter().cloned());
            break;
        }
        if arg.first() == Some(&b'-')
            && arg.len() > 1
            && !arg[1..].first().map_or(false, |c| c.is_ascii_digit())
        {
            let mut j = 1;
            while j < arg.len() {
                let ch = arg[j];
                match ch {
                    b'l' => list_mode = true,
                    b'n' => suppress_numbers = true,
                    b'r' => reverse = true,
                    b's' => reexec = true,
                    b'e' => {
                        let rest = &arg[j + 1..];
                        if rest.is_empty() {
                            i += 1;
                            if i >= argv.len() {
                                return Err(shell.diagnostic(2, b"fc: -e requires an argument"));
                            }
                            editor = Some(argv[i].clone());
                        } else {
                            editor = Some(rest.to_vec());
                        }
                        break;
                    }
                    _ => {
                        let msg = ByteWriter::new()
                            .bytes(b"fc: invalid option: -")
                            .byte(ch)
                            .finish();
                        return Err(shell.diagnostic(2, &msg));
                    }
                }
                j += 1;
            }
        } else {
            operands.push(arg.clone());
        }
        i += 1;
    }

    let history = &shell.history;
    if history.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }

    if reexec {
        let mut substitution: Option<(&[u8], &[u8])> = None;
        let mut first_operand: Option<&[u8]> = None;
        for op in &operands {
            if let Some((old, new)) = op.split_once_byte(b'=') {
                substitution = Some((old, new));
            } else {
                first_operand = Some(op);
            }
        }
        let idx = match first_operand {
            Some(op) => fc_resolve_operand(history, op).ok_or_else(|| {
                let msg = ByteWriter::new()
                    .bytes(b"fc: no command found matching '")
                    .bytes(op)
                    .bytes(b"'")
                    .finish();
                shell.diagnostic(1, &msg)
            })?,
            None => history.len() - 1,
        };
        let mut cmd = history[idx].to_vec();
        if let Some((old, new)) = substitution {
            if let Some(pos) = cmd.windows(old.len()).position(|w| w == old) {
                let mut replaced = cmd[..pos].to_vec();
                replaced.extend_from_slice(new);
                replaced.extend_from_slice(&cmd[pos + old.len()..]);
                cmd = replaced;
            }
        }
        shell.add_history(&cmd);
        let status = shell
            .execute_string(&cmd)
            .unwrap_or_else(|e| e.exit_status());
        shell.last_status = status;
        return Ok(BuiltinOutcome::Status(status));
    }

    if list_mode {
        let (first, last) = match operands.len() {
            0 => {
                let end = history.len().saturating_sub(1);
                let start = end.saturating_sub(15);
                (start, end)
            }
            1 => {
                let a = fc_resolve_operand(history, &operands[0])
                    .unwrap_or(history.len().saturating_sub(1));
                (a, history.len().saturating_sub(1))
            }
            _ => {
                let a = fc_resolve_operand(history, &operands[0])
                    .unwrap_or(history.len().saturating_sub(1));
                let b = fc_resolve_operand(history, &operands[1])
                    .unwrap_or(history.len().saturating_sub(1));
                (a, b)
            }
        };

        let (lo, hi) = if first <= last {
            (first, last)
        } else {
            (last, first)
        };

        let do_reverse = if first <= last { reverse } else { !reverse };

        if do_reverse {
            for idx in (lo..=hi).rev() {
                if suppress_numbers {
                    let line = ByteWriter::new()
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                } else {
                    let line = ByteWriter::new()
                        .usize_val(idx + 1)
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                }
            }
        } else {
            for idx in lo..=hi {
                if suppress_numbers {
                    let line = ByteWriter::new()
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                } else {
                    let line = ByteWriter::new()
                        .usize_val(idx + 1)
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                }
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let idx = if operands.is_empty() {
        history.len() - 1
    } else {
        fc_resolve_operand(history, &operands[0])
            .ok_or_else(|| shell.diagnostic(1, b"fc: history specification out of range"))?
    };

    let editor_cmd = match editor {
        Some(ref e) => e.as_slice(),
        None => shell.get_var(b"FCEDIT").unwrap_or(b"ed"),
    };

    let tmp_path = ByteWriter::new()
        .bytes(b"/tmp/fc_edit_")
        .i64_val(sys::current_pid() as i64)
        .finish();
    let cmd_text = &history[idx];
    let fd = sys::open_file(
        &tmp_path,
        sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC,
        0o600,
    )
    .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"fc: ", &e))?;
    let _ = sys::write_all_fd(fd, cmd_text);
    let _ = sys::write_all_fd(fd, b"\n");
    let _ = sys::close_fd(fd);

    let edit_cmd = ByteWriter::new()
        .bytes(editor_cmd)
        .byte(b' ')
        .bytes(&tmp_path)
        .finish();
    let edit_status = shell
        .execute_string(&edit_cmd)
        .unwrap_or_else(|e| e.exit_status());
    if edit_status != 0 {
        remove_file_bytes(&tmp_path);
        return Ok(BuiltinOutcome::Status(edit_status));
    }

    let edited =
        sys::read_file(&tmp_path).map_err(|e| shell.diagnostic_prefixed_syserr(1, b"fc: ", &e))?;
    remove_file_bytes(&tmp_path);

    let edited = edited.trim_trailing_newlines();
    if !edited.is_empty() {
        shell.add_history(edited);
        let status = shell
            .execute_string(edited)
            .unwrap_or_else(|e| e.exit_status());
        shell.last_status = status;
        return Ok(BuiltinOutcome::Status(status));
    }

    Ok(BuiltinOutcome::Status(0))
}

fn execute_command_utility(
    shell: &mut Shell,
    argv: &[Vec<u8>],
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
        let msg = ByteWriter::new()
            .bytes(b"command: ")
            .bytes(name)
            .bytes(b": not found")
            .finish();
        return Ok(diag_status(shell, 127, &msg));
    };

    if sys::access_path(&path, sys::X_OK).is_err() {
        let msg = ByteWriter::new()
            .bytes(b"command: ")
            .bytes(name)
            .bytes(b": Permission denied")
            .finish();
        return Ok(diag_status(shell, 126, &msg));
    }

    let mut child_env = shell.env_for_child();
    if use_default_path {
        child_env.retain(|(k, _)| k != b"PATH");
        child_env.push((b"PATH".to_vec(), DEFAULT_COMMAND_PATH.to_vec()));
    }
    let env_pairs: Vec<(&[u8], &[u8])> = child_env
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();
    let argv_strs: Vec<&[u8]> = argv.iter().map(|v| v.as_slice()).collect();

    match sys::spawn_child(
        &path,
        &argv_strs,
        Some(&env_pairs),
        &[],
        None,
        false,
        None,
    ) {
        Ok(handle) => {
            let ws = sys::wait_pid(handle.pid, false)
                .map_err(|e| shell.diagnostic(1, &e.strerror()))?
                .expect("child status");
            Ok(BuiltinOutcome::Status(sys::decode_wait_status(ws.status)))
        }
        Err(error) if error.is_enoent() => {
            let msg = ByteWriter::new()
                .bytes(b"command: ")
                .bytes(name)
                .bytes(b": not found")
                .finish();
            Ok(diag_status(shell, 127, &msg))
        }
        Err(error) => {
            let msg = ByteWriter::new()
                .bytes(b"command: ")
                .bytes(name)
                .bytes(b": ")
                .bytes(&error.strerror())
                .finish();
            Ok(diag_status(shell, 126, &msg))
        }
    }
}

fn which_in_path(name: &[u8], shell: &Shell, use_default_path: bool) -> Option<Vec<u8>> {
    search_path(name, shell, use_default_path, path_exists)
}

fn search_path(
    name: &[u8],
    shell: &Shell,
    use_default_path: bool,
    predicate: fn(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    if name.contains_byte(b'/') {
        if predicate(name) {
            return absolute_path(name);
        }
        return None;
    }

    let path_env_owned;
    let path_env: &[u8] = if use_default_path {
        DEFAULT_COMMAND_PATH
    } else {
        path_env_owned = shell
            .get_var(b"PATH")
            .map(|s| s.to_vec())
            .or_else(|| sys::env_var(b"PATH"))
            .unwrap_or_default();
        &path_env_owned
    };

    for dir in path_env.split(|&b| b == b':') {
        let candidate = if dir.is_empty() {
            let mut c = b"./".to_vec();
            c.extend_from_slice(name);
            c
        } else {
            let mut c = dir.to_vec();
            c.push(b'/');
            c.extend_from_slice(name);
            c
        };
        if predicate(&candidate) {
            return absolute_path(&candidate);
        }
    }
    None
}

fn path_exists(path: &[u8]) -> bool {
    sys::file_exists(path)
}

fn readable_regular_file(path: &[u8]) -> bool {
    sys::is_regular_file(path) && sys::access_path(path, sys::R_OK).is_ok()
}

fn absolute_path(path: &[u8]) -> Option<Vec<u8>> {
    if path.first() == Some(&b'/') {
        return Some(path.to_vec());
    }
    sys::get_cwd().ok().map(|cwd| {
        let mut result = cwd;
        result.push(b'/');
        result.extend_from_slice(path);
        result
    })
}

const SPECIAL_BUILTIN_NAMES: &[&[u8]] = &[
    b".", b":", b"break", b"continue", b"eval", b"exec", b"exit", b"export", b"readonly",
    b"return", b"set", b"shift", b"times", b"trap", b"unset",
];

pub fn is_special_builtin(name: &[u8]) -> bool {
    SPECIAL_BUILTIN_NAMES.binary_search(&name).is_ok()
}

fn is_reserved_word_name(word: &[u8]) -> bool {
    matches!(
        word,
        b"!" | b"{"
            | b"}"
            | b"case"
            | b"do"
            | b"done"
            | b"elif"
            | b"else"
            | b"esac"
            | b"fi"
            | b"for"
            | b"if"
            | b"in"
            | b"then"
            | b"until"
            | b"while"
    )
}

// ---------------------------------------------------------------------------
// test / [ builtin
// ---------------------------------------------------------------------------

fn test_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let is_bracket = argv[0] == b"[";
    let args: &[Vec<u8>] = if is_bracket {
        if argv.last().map(|s| s.as_slice()) != Some(b"]") {
            shell.diagnostic(2, b"[: missing ']'");
            return Ok(BuiltinOutcome::Status(2));
        }
        &argv[1..argv.len() - 1]
    } else {
        &argv[1..]
    };
    let result = match args.len() {
        0 => Ok(false),
        1 => Ok(!args[0].is_empty()),
        2 => test_two_args(shell, &args[0], &args[1]),
        3 => test_three_args(shell, &args[0], &args[1], &args[2]),
        4 => {
            if args[0] == b"!" {
                test_three_args(shell, &args[1], &args[2], &args[3]).map(|r| !r)
            } else {
                shell.diagnostic(2, b"test: too many arguments");
                return Ok(BuiltinOutcome::Status(2));
            }
        }
        _ => {
            shell.diagnostic(2, b"test: too many arguments");
            return Ok(BuiltinOutcome::Status(2));
        }
    };
    match result {
        Ok(true) => Ok(BuiltinOutcome::Status(0)),
        Ok(false) => Ok(BuiltinOutcome::Status(1)),
        Err(msg) => {
            let full = ByteWriter::new()
                .bytes(b"test: ")
                .bytes(&msg)
                .finish();
            shell.diagnostic(2, &full);
            Ok(BuiltinOutcome::Status(2))
        }
    }
}

type TestResult = Result<bool, Vec<u8>>;

fn test_two_args(shell: &Shell, op: &[u8], operand: &[u8]) -> TestResult {
    if op == b"!" {
        return Ok(operand.is_empty());
    }
    test_unary(shell, op, operand)
}

fn test_three_args(_shell: &Shell, left: &[u8], op: &[u8], right: &[u8]) -> TestResult {
    if op == b"=" {
        return Ok(left == right);
    }
    if op == b"!=" {
        return Ok(left != right);
    }
    if op == b">" {
        return Ok(left > right);
    }
    if op == b"<" {
        return Ok(left < right);
    }
    if let Some(r) = test_integer_binary(left, op, right) {
        return r;
    }
    if let Some(r) = test_file_binary(left, op, right) {
        return r;
    }
    if left == b"!" {
        return test_two_args(_shell, op, right).map(|r| !r);
    }
    let mut msg = b"unknown operator: ".to_vec();
    msg.extend_from_slice(op);
    Err(msg)
}

fn test_unary(_shell: &Shell, op: &[u8], operand: &[u8]) -> TestResult {
    match op {
        b"-n" => Ok(!operand.is_empty()),
        b"-z" => Ok(operand.is_empty()),
        b"-b" => Ok(sys::stat_path(operand)
            .map(|s| s.is_block_special())
            .unwrap_or(false)),
        b"-c" => Ok(sys::stat_path(operand)
            .map(|s| s.is_char_special())
            .unwrap_or(false)),
        b"-d" => Ok(sys::stat_path(operand).map(|s| s.is_dir()).unwrap_or(false)),
        b"-e" => Ok(sys::stat_path(operand).is_ok()),
        b"-f" => Ok(sys::stat_path(operand)
            .map(|s| s.is_regular_file())
            .unwrap_or(false)),
        b"-g" => Ok(sys::stat_path(operand)
            .map(|s| s.is_setgid())
            .unwrap_or(false)),
        b"-h" | b"-L" => Ok(sys::lstat_path(operand)
            .map(|s| s.is_symlink())
            .unwrap_or(false)),
        b"-p" => Ok(sys::stat_path(operand)
            .map(|s| s.is_fifo())
            .unwrap_or(false)),
        b"-r" => Ok(sys::access_path(operand, libc::R_OK).is_ok()),
        b"-s" => Ok(sys::stat_path(operand).map(|s| s.size > 0).unwrap_or(false)),
        b"-S" => Ok(sys::stat_path(operand)
            .map(|s| s.is_socket())
            .unwrap_or(false)),
        b"-t" => {
            let fd: i32 = bstr::parse_i64(operand)
                .and_then(|v| if v >= 0 && v <= i32::MAX as i64 { Some(v as i32) } else { None })
                .ok_or_else(|| {
                    let mut msg = operand.to_vec();
                    msg.extend_from_slice(b": not a valid fd");
                    msg
                })?;
            Ok(sys::isatty_fd(fd))
        }
        b"-u" => Ok(sys::stat_path(operand)
            .map(|s| s.is_setuid())
            .unwrap_or(false)),
        b"-w" => Ok(sys::access_path(operand, libc::W_OK).is_ok()),
        b"-x" => Ok(sys::access_path(operand, libc::X_OK).is_ok()),
        _ => {
            let mut msg = b"unknown unary operator: ".to_vec();
            msg.extend_from_slice(op);
            Err(msg)
        }
    }
}

fn test_integer_binary(left: &[u8], op: &[u8], right: &[u8]) -> Option<TestResult> {
    let cmp = match op {
        b"-eq" | b"-ne" | b"-gt" | b"-ge" | b"-lt" | b"-le" => op,
        _ => return None,
    };
    let l: i64 = match bstr::parse_i64(left) {
        Some(v) => v,
        None => {
            let mut msg = left.to_vec();
            msg.extend_from_slice(b": integer expression expected");
            return Some(Err(msg));
        }
    };
    let r: i64 = match bstr::parse_i64(right) {
        Some(v) => v,
        None => {
            let mut msg = right.to_vec();
            msg.extend_from_slice(b": integer expression expected");
            return Some(Err(msg));
        }
    };
    let result = match cmp {
        b"-eq" => l == r,
        b"-ne" => l != r,
        b"-gt" => l > r,
        b"-ge" => l >= r,
        b"-lt" => l < r,
        b"-le" => l <= r,
        _ => unreachable!(),
    };
    Some(Ok(result))
}

fn test_file_binary(left: &[u8], op: &[u8], right: &[u8]) -> Option<TestResult> {
    match op {
        b"-ef" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(a.is_some()
                && b.is_some()
                && a.as_ref().unwrap().same_file(b.as_ref().unwrap())))
        }
        b"-nt" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(match (a, b) {
                (Some(a), Some(b)) => a.newer_than(&b),
                (Some(_), None) => true,
                _ => false,
            }))
        }
        b"-ot" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(match (a, b) {
                (Some(a), Some(b)) => b.newer_than(&a),
                (None, Some(_)) => true,
                _ => false,
            }))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// echo builtin
// ---------------------------------------------------------------------------

fn echo_builtin(_shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut out: Vec<u8> = Vec::new();
    for (i, arg) in argv[1..].iter().enumerate() {
        if i > 0 {
            out.push(b' ');
        }
        out.extend_from_slice(arg);
    }
    out.push(b'\n');
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, &out);
    Ok(BuiltinOutcome::Status(0))
}

// ---------------------------------------------------------------------------
// printf builtin
// ---------------------------------------------------------------------------

fn printf_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        shell.diagnostic(1, b"printf: missing format operand");
        return Ok(BuiltinOutcome::Status(1));
    }
    let format = &argv[1];
    let args = &argv[2..];
    let mut had_error = false;
    let mut arg_idx = 0;
    loop {
        let (output, consumed, stop, error) = printf_format(shell, format, args, arg_idx);
        if !output.is_empty() {
            let _ = sys::write_all_fd(sys::STDOUT_FILENO, &output);
        }
        if error {
            had_error = true;
        }
        if stop {
            break;
        }
        arg_idx += consumed;
        if arg_idx >= args.len() {
            break;
        }
    }
    Ok(BuiltinOutcome::Status(if had_error { 1 } else { 0 }))
}

fn printf_parse_numeric_arg(shell: &Shell, arg: &[u8], had_error: &mut bool) -> (i64, bool) {
    match printf_parse_int(arg) {
        Ok(v) => (v, true),
        Err(msg) => {
            let full = ByteWriter::new()
                .bytes(b"printf: ")
                .bytes(&msg)
                .finish();
            shell.diagnostic(1, &full);
            *had_error = true;
            (0, false)
        }
    }
}

fn printf_check_trailing(shell: &Shell, arg: &[u8], had_error: &mut bool) {
    if !arg.is_empty() && arg[0] != b'\'' && arg[0] != b'"' {
        if printf_find_trailing_garbage(arg).is_some() {
            let msg = ByteWriter::new()
                .bytes(b"printf: \"")
                .bytes(arg)
                .bytes(b"\": not completely converted")
                .finish();
            shell.diagnostic(1, &msg);
            *had_error = true;
        }
    }
}

fn printf_get_arg<'a>(args: &'a [Vec<u8>], base: usize, idx: usize) -> &'a [u8] {
    args.get(base + idx).map(|s| s.as_slice()).unwrap_or(b"")
}

fn printf_parse_int(s: &[u8]) -> Result<i64, Vec<u8>> {
    if s.is_empty() {
        return Ok(0);
    }
    if s[0] == b'\'' || s[0] == b'"' {
        let ch = s.get(1).copied().unwrap_or(0);
        return Ok(ch as i64);
    }
    let (neg, s) = if let Some(rest) = s.strip_prefix(b"-") {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix(b"+") {
        (false, rest)
    } else {
        (false, s)
    };
    let val = if let Some(hex) = s.strip_prefix(b"0x").or_else(|| s.strip_prefix(b"0X")) {
        parse_hex_i64(hex).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    } else if s.first() == Some(&b'0') && s.len() > 1 {
        parse_octal_i64(s).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    } else {
        bstr::parse_i64(s).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    };
    val.map(|v| if neg { -v } else { v })
}

fn parse_hex_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in s {
        let digit = match b {
            b'0'..=b'9' => (b - b'0') as i64,
            b'a'..=b'f' => (b - b'a' + 10) as i64,
            b'A'..=b'F' => (b - b'A' + 10) as i64,
            _ => return None,
        };
        result = result.checked_mul(16)?.checked_add(digit)?;
    }
    Some(result)
}

fn parse_octal_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in s {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        result = result.checked_mul(8)?.checked_add((b - b'0') as i64)?;
    }
    Some(result)
}

fn printf_format(
    shell: &Shell,
    format: &[u8],
    args: &[Vec<u8>],
    arg_base: usize,
) -> (Vec<u8>, usize, bool, bool) {
    let mut out: Vec<u8> = Vec::new();
    let bytes = format;
    let mut i = 0;
    let mut arg_consumed = 0;
    let mut had_error = false;
    let mut stop = false;

    while i < bytes.len() {
        if stop {
            break;
        }
        if bytes[i] == b'\\' {
            let (esc, advance) = printf_format_escape(bytes, i + 1);
            out.extend_from_slice(&esc);
            i += 1 + advance;
            continue;
        }
        if bytes[i] == b'%' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'%' {
                out.push(b'%');
                i += 2;
                continue;
            }
            let spec_start = i;
            i += 1;

            let mut numbered_arg: Option<usize> = None;
            let saved = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'$' && i > saved {
                let n = parse_usize(&bytes[saved..i]).unwrap_or(0);
                if n > 0 {
                    numbered_arg = Some(n - 1);
                }
                i += 1;
            } else {
                i = saved;
            }

            while i < bytes.len() && matches!(bytes[i], b'-' | b'+' | b' ' | b'0' | b'#') {
                i += 1;
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'.' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i >= bytes.len() {
                out.extend_from_slice(&format[spec_start..]);
                break;
            }
            let conv = bytes[i];
            i += 1;
            let full_spec = &format[spec_start..i];

            let arg_index = if let Some(n) = numbered_arg {
                n
            } else {
                let idx = arg_consumed;
                arg_consumed += 1;
                idx
            };
            let arg = printf_get_arg(args, arg_base, arg_index);

            match conv {
                b's' => {
                    let spec_for_rust = remove_byte(full_spec, conv);
                    printf_format_string(&mut out, &spec_for_rust, arg);
                }
                b'b' => {
                    let (expanded, saw_c) = printf_expand_b(arg);
                    let spec_for_rust = remove_byte(full_spec, b'b');
                    printf_format_string(&mut out, &spec_for_rust, &expanded);
                    if saw_c {
                        stop = true;
                    }
                }
                b'c' => {
                    if let Some(&b) = arg.first() {
                        out.push(b);
                    }
                }
                b'd' | b'i' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    let mut c_spec = remove_byte(full_spec, conv);
                    c_spec.extend_from_slice(b"ld");
                    printf_format_signed(&mut out, &c_spec, val);
                }
                b'u' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    let mut c_spec = remove_byte(full_spec, b'u');
                    c_spec.extend_from_slice(b"lu");
                    printf_format_unsigned(&mut out, &c_spec, val as u64);
                }
                b'o' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    printf_format_octal(&mut out, full_spec, val as u64);
                }
                b'x' | b'X' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    printf_format_hex(&mut out, full_spec, val as u64, conv == b'X');
                }
                _ => {
                    out.extend_from_slice(full_spec);
                }
            }
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    (
        out,
        arg_consumed,
        stop || arg_base + arg_consumed >= args.len(),
        had_error,
    )
}

fn remove_byte(s: &[u8], byte: u8) -> Vec<u8> {
    s.iter().copied().filter(|&b| b != byte).collect()
}

fn printf_find_trailing_garbage(s: &[u8]) -> Option<usize> {
    let s = if s.first() == Some(&b'+') || s.first() == Some(&b'-') {
        &s[1..]
    } else {
        s
    };
    if let Some(hex) = s.strip_prefix(b"0x").or_else(|| s.strip_prefix(b"0X")) {
        for (i, &c) in hex.iter().enumerate() {
            if !c.is_ascii_hexdigit() {
                return Some(i + 2);
            }
        }
        return None;
    }
    if s.first() == Some(&b'0') && s.len() > 1 {
        for (i, &c) in s.iter().enumerate().skip(1) {
            if !(b'0'..=b'7').contains(&c) {
                return Some(i);
            }
        }
        return None;
    }
    for (i, &c) in s.iter().enumerate() {
        if !c.is_ascii_digit() {
            return Some(i);
        }
    }
    None
}

fn printf_format_escape(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
    if start >= bytes.len() {
        return (vec![b'\\'], 0);
    }
    match bytes[start] {
        b'\\' => (vec![b'\\'], 1),
        b'a' => (vec![0x07], 1),
        b'b' => (vec![0x08], 1),
        b'f' => (vec![0x0c], 1),
        b'n' => (vec![b'\n'], 1),
        b'r' => (vec![b'\r'], 1),
        b't' => (vec![b'\t'], 1),
        b'v' => (vec![0x0b], 1),
        b'0'..=b'7' => {
            let mut val: u8 = 0;
            let mut count = 0;
            let mut j = start;
            while j < bytes.len() && count < 3 && bytes[j] >= b'0' && bytes[j] <= b'7' {
                val = val.wrapping_mul(8).wrapping_add(bytes[j] - b'0');
                j += 1;
                count += 1;
            }
            (vec![val], count)
        }
        _ => (vec![b'\\', bytes[start]], 1),
    }
}

fn printf_expand_b(s: &[u8]) -> (Vec<u8>, bool) {
    let mut out: Vec<u8> = Vec::new();
    let bytes = s;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 1;
            if i >= bytes.len() {
                out.push(b'\\');
                break;
            }
            match bytes[i] {
                b'\\' => {
                    out.push(b'\\');
                    i += 1;
                }
                b'a' => {
                    out.push(0x07);
                    i += 1;
                }
                b'b' => {
                    out.push(0x08);
                    i += 1;
                }
                b'f' => {
                    out.push(0x0c);
                    i += 1;
                }
                b'n' => {
                    out.push(b'\n');
                    i += 1;
                }
                b'r' => {
                    out.push(b'\r');
                    i += 1;
                }
                b't' => {
                    out.push(b'\t');
                    i += 1;
                }
                b'v' => {
                    out.push(0x0b);
                    i += 1;
                }
                b'c' => return (out, true),
                b'0' => {
                    i += 1;
                    let mut val: u8 = 0;
                    let mut count = 0;
                    while i < bytes.len() && count < 3 && bytes[i] >= b'0' && bytes[i] <= b'7' {
                        val = val.wrapping_mul(8).wrapping_add(bytes[i] - b'0');
                        i += 1;
                        count += 1;
                    }
                    out.push(val);
                }
                _ => {
                    out.push(b'\\');
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    (out, false)
}

fn printf_format_string(out: &mut Vec<u8>, spec: &[u8], s: &[u8]) {
    let spec = if spec.first() == Some(&b'%') { &spec[1..] } else { spec };
    let left = spec.contains_byte(b'-');
    let spec_rest = trim_leading_flags(spec);

    let (width_str, prec_str) = if let Some(dot_pos) = spec_rest.iter().position(|&b| b == b'.') {
        (&spec_rest[..dot_pos], Some(&spec_rest[dot_pos + 1..]))
    } else {
        (spec_rest, None)
    };

    let width: usize = parse_usize(width_str).unwrap_or(0);
    let value = if let Some(prec) = prec_str {
        let max: usize = parse_usize(prec).unwrap_or(usize::MAX);
        if s.len() > max { &s[..max] } else { s }
    } else {
        s
    };

    if left || width <= value.len() {
        out.extend_from_slice(value);
        if left && width > value.len() {
            out.resize(out.len() + width - value.len(), b' ');
        }
    } else {
        out.resize(out.len() + width - value.len(), b' ');
        out.extend_from_slice(value);
    }
}

fn trim_leading_flags(spec: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < spec.len() && matches!(spec[i], b'-' | b'+' | b' ' | b'0' | b'#') {
        i += 1;
    }
    &spec[i..]
}

fn printf_format_signed(out: &mut Vec<u8>, spec: &[u8], val: i64) {
    let spec = if spec.first() == Some(&b'%') { &spec[1..] } else { spec };
    let spec = if spec.ends_with(b"ld") {
        &spec[..spec.len() - 2]
    } else {
        spec
    };
    let left = spec.contains_byte(b'-');
    let zero_pad = spec.contains_byte(b'0') && !left;
    let spec_rest = trim_leading_flags(spec);

    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = bstr::i64_to_bytes(val);

    if width <= num.len() || (!left && !zero_pad) {
        if width > num.len() && !left {
            out.resize(out.len() + width - num.len(), b' ');
        }
        out.extend_from_slice(&num);
    } else if zero_pad {
        if val < 0 {
            out.push(b'-');
            let digits = &num[1..];
            if width > num.len() {
                out.resize(out.len() + width - num.len(), b'0');
            }
            out.extend_from_slice(digits);
        } else {
            out.resize(out.len() + width - num.len(), b'0');
            out.extend_from_slice(&num);
        }
    } else {
        out.extend_from_slice(&num);
    }
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

fn printf_format_unsigned(out: &mut Vec<u8>, spec: &[u8], val: u64) {
    let spec = if spec.first() == Some(&b'%') { &spec[1..] } else { spec };
    let spec = if spec.ends_with(b"lu") {
        &spec[..spec.len() - 2]
    } else {
        spec
    };
    let left = spec.contains_byte(b'-');
    let zero_pad = spec.contains_byte(b'0') && !left;
    let spec_rest = trim_leading_flags(spec);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = bstr::u64_to_bytes(val);
    if zero_pad && width > num.len() {
        out.resize(out.len() + width - num.len(), b'0');
    } else if !left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
    out.extend_from_slice(&num);
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

fn printf_format_octal(out: &mut Vec<u8>, spec: &[u8], val: u64) {
    let spec_inner = if spec.first() == Some(&b'%') { &spec[1..] } else { spec };
    let spec_inner = if spec_inner.last() == Some(&b'o') {
        &spec_inner[..spec_inner.len() - 1]
    } else {
        spec_inner
    };
    let alt = spec_inner.contains_byte(b'#');
    let left = spec_inner.contains_byte(b'-');
    let zero_pad = spec_inner.contains_byte(b'0') && !left;
    let spec_rest = trim_leading_flags(spec_inner);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = if alt && val != 0 {
        let mut r = b"0".to_vec();
        bstr::push_u64_octal(&mut r, val);
        r
    } else if alt {
        b"0".to_vec()
    } else {
        let mut r = Vec::new();
        bstr::push_u64_octal(&mut r, val);
        r
    };
    if zero_pad && width > num.len() {
        out.resize(out.len() + width - num.len(), b'0');
    } else if !left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
    out.extend_from_slice(&num);
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

fn printf_format_hex(out: &mut Vec<u8>, spec: &[u8], val: u64, upper: bool) {
    let suffix = if upper { b'X' } else { b'x' };
    let spec_inner = if spec.first() == Some(&b'%') { &spec[1..] } else { spec };
    let spec_inner = if spec_inner.last() == Some(&suffix) {
        &spec_inner[..spec_inner.len() - 1]
    } else {
        spec_inner
    };
    let alt = spec_inner.contains_byte(b'#');
    let left = spec_inner.contains_byte(b'-');
    let zero_pad = spec_inner.contains_byte(b'0') && !left;
    let spec_rest = trim_leading_flags(spec_inner);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = if upper {
        let mut r = Vec::new();
        bstr::push_u64_hex_upper(&mut r, val);
        r
    } else {
        let mut r = Vec::new();
        bstr::push_u64_hex(&mut r, val);
        r
    };
    let prefix: &[u8] = if alt && val != 0 {
        if upper { b"0X" } else { b"0x" }
    } else {
        b""
    };
    let total = prefix.len() + num.len();
    if zero_pad && width > total {
        out.extend_from_slice(prefix);
        out.resize(out.len() + width - total, b'0');
    } else if !left && width > total {
        out.resize(out.len() + width - total, b' ');
        out.extend_from_slice(prefix);
    } else {
        out.extend_from_slice(prefix);
    }
    out.extend_from_slice(&num);
    if left && width > total {
        out.resize(out.len() + width - total, b' ');
    }
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
            shell_name: (*b"meiksh").into(),
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
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
        }
    }

    fn invoke(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
        super::run(shell, argv, &[])
    }

    fn literal(raw: &[u8]) -> Word {
        Word {
            raw: raw.into(),
            line: 0,
        }
    }

    #[test]
    fn builtin_registry_knows_core_commands() {
        assert_no_syscalls(|| {
            assert!(is_builtin(b"cd"));
            assert!(is_builtin(b"export"));
            assert!(is_builtin(b"read"));
            assert!(is_builtin(b"umask"));
            assert!(is_builtin(b"printf"));
            assert!(is_builtin(b"echo"));
            assert!(is_builtin(b"test"));
            assert!(is_builtin(b"["));
        });
    }

    #[test]
    fn export_updates_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"export".to_vec(), b"NAME=value".to_vec()]).expect("export");
            assert_eq!(shell.get_var(b"NAME"), Some(b"value" as &[u8]));
            assert!(shell.exported.contains(b"NAME" as &[u8]));
        });
    }

    #[test]
    fn unset_removes_variable_and_export_flag() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"export".to_vec(), b"NAME=value".to_vec()]).expect("export");

            invoke(&mut shell, &[b"unset".to_vec(), b"NAME".to_vec()]).expect("unset");
            assert_eq!(shell.get_var(b"NAME"), None);
            assert!(!shell.exported.contains(b"NAME" as &[u8]));
        });
    }

    #[test]
    fn readonly_marks_variable_readonly() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"readonly".to_vec(), b"LOCKED=value".to_vec()]).expect("readonly");
            assert!(shell.readonly.contains(b"LOCKED" as &[u8]));
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

                shell.positional = vec![b"a".to_vec()];
                let outcome = invoke(&mut shell, &[b"shift".to_vec(), b"5".to_vec()]).expect("shift");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                let error =
                    invoke(&mut shell, &[b"shift".to_vec(), b"bad".to_vec()]).expect_err("bad shift");
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
                        ArgMatcher::Bytes(b"meiksh: unalias: missing: not found\n".to_vec()),
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
                invoke(&mut shell, &[b"alias".to_vec(), b"ll=ls -l".to_vec()]).expect("alias");
                invoke(&mut shell, &[b"alias".to_vec(), b"la=ls -a".to_vec()]).expect("alias");
                assert_eq!(shell.aliases.get(b"ll" as &[u8]).map(|s| &**s), Some(b"ls -l" as &[u8]));

                let outcome =
                    invoke(&mut shell, &[b"alias".to_vec(), b"ll".to_vec()]).expect("alias query");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));

                let outcome =
                    invoke(&mut shell, &[b"alias".to_vec(), b"missing".to_vec()]).expect("missing alias");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                invoke(&mut shell, &[b"unalias".to_vec(), b"ll".to_vec()]).expect("unalias");
                assert!(!shell.aliases.contains_key(b"ll" as &[u8]));
                let outcome = invoke(&mut shell, &[b"unalias".to_vec(), b"missing".to_vec()])
                    .expect("unalias missing");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                let outcome =
                    invoke(&mut shell, &[b"unalias".to_vec(), b"-a".to_vec()]).expect("unalias all");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(shell.aliases.is_empty());

                let error = invoke(&mut shell, &[b"unalias".to_vec()]).expect_err("missing alias");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn alias_output_is_shell_quoted_for_reinput() {
        assert_no_syscalls(|| {
            assert_eq!(format_alias_definition(b"ll", b"ls -l"), b"ll='ls -l'");
            assert_eq!(format_alias_definition(b"sq", b"a'b"), b"sq='a'\\''b'");
            assert_eq!(format_alias_definition(b"empty", b""), b"empty=''");
        });
    }

    #[test]
    fn read_options_and_assignments_parsing() {
        assert_no_syscalls(|| {
            let (options, vars) = parse_read_options(&[
                b"read".to_vec(),
                b"-r".to_vec(),
                b"-d".to_vec(),
                b",".to_vec(),
                b"A".to_vec(),
                b"B".to_vec(),
            ])
            .expect("read options");
            assert!(options.raw);
            assert_eq!(options.delimiter, b',');
            assert_eq!(vars, vec![b"A".to_vec(), b"B".to_vec()]);
            assert_eq!(
                parse_read_options(&[b"read".to_vec(), b"-d".to_vec(), b"".to_vec(), b"NUL".to_vec()])
                    .expect("read nul delim")
                    .0
                    .delimiter,
                0
            );
            assert_eq!(
                parse_read_options(&[b"read".to_vec(), b"--".to_vec(), b"NAME".to_vec()])
                    .expect("read dash dash")
                    .1,
                vec![b"NAME".to_vec()]
            );

            assert_eq!(
                split_read_assignments(
                    &[(b"alpha beta gamma".to_vec(), false)],
                    &[b"FIRST".to_vec(), b"SECOND".to_vec()],
                    None,
                ),
                vec![b"alpha".to_vec(), b"beta gamma".to_vec()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"  alpha beta  ".to_vec(), false)],
                    &[b"ONLY".to_vec()],
                    None,
                ),
                vec![b"alpha beta".to_vec()]
            );
            assert_eq!(split_read_assignments(&[], &[], None), Vec::<Vec<u8>>::new());
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha beta".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    Some(Vec::new()),
                ),
                vec![b"alpha beta".to_vec(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b" \t ".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    None,
                ),
                vec![Vec::new(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"left,right".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    Some(b",".to_vec()),
                ),
                vec![b"left".to_vec(), b"right".to_vec()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec(), b"THREE".to_vec()],
                    None,
                ),
                vec![b"alpha".to_vec(), Vec::new(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha,   ".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec(), b"THREE".to_vec()],
                    Some(b", ".to_vec()),
                ),
                vec![b"alpha".to_vec(), Vec::new(), Vec::new()]
            );

            let mut pieces = Vec::new();
            let mut empty = Vec::new();
            push_read_piece(&mut pieces, &mut empty, false);
            assert!(pieces.is_empty());
        });
    }

    #[test]
    fn umask_parsing_helpers() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask(b"077", 0o022), Some(0o077));
            assert_eq!(parse_umask_mask(b"g-w", 0o002), Some(0o022));
            assert_eq!(parse_umask_mask(b"u=rw,go=r", 0o022), Some(0o133));
            assert_eq!(parse_umask_mask(b"a+x", 0o777), Some(0o666));
            assert_eq!(parse_umask_mask(b"u=g", 0o022), Some(0o222));
            assert_eq!(parse_umask_mask(b"u=o", 0o022), Some(0o222));
            assert_eq!(format_umask_symbolic(0o022), b"u=rwx,g=rx,o=rx");
            assert_eq!(parse_symbolic_targets(b"z"), 0);
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
            assert_eq!(format_times_value(125, 100), b"0m1.25s");
        });
    }

    #[test]
    fn canonicalize_logical_path_handles_all_cases() {
        assert_no_syscalls(|| {
            assert_eq!(canonicalize_logical_path(b"/usr/.."), b"/");
            assert_eq!(canonicalize_logical_path(b"/a/b/../c"), b"/a/c");
            assert_eq!(canonicalize_logical_path(b"/a/./b"), b"/a/b");
            assert_eq!(canonicalize_logical_path(b"/"), b"/");
            assert_eq!(canonicalize_logical_path(b"/a/b/../../.."), b"/");
            assert_eq!(canonicalize_logical_path(b"/a//b"), b"/a/b");
        });
    }

    #[test]
    fn needs_quoting_and_shell_quote_if_needed_coverage() {
        assert!(!super::needs_quoting(b"simple"));
        assert!(!super::needs_quoting(b"path/to.file-1+2:3,4"));
        assert!(super::needs_quoting(b"has space"));
        assert!(super::needs_quoting(b""));
        assert!(super::needs_quoting(b"quo'te"));

        let result = super::shell_quote_if_needed(b"hello");
        assert_eq!(result, b"hello");

        let result = super::shell_quote_if_needed(b"has space");
        assert_eq!(result, b"'has space'");
    }

    #[test]
    fn format_limit_covers_both_branches() {
        assert_eq!(format_limit(sys::RLIM_INFINITY), b"unlimited");
        assert_eq!(format_limit(42), b"42");
    }

    #[test]
    fn ulimit_resource_for_option_covers_all_and_unknown() {
        for ch in [b'c', b'd', b'f', b'n', b's', b't', b'v'] {
            assert!(ulimit_resource_for_option(ch).is_some());
        }
        assert!(ulimit_resource_for_option(b'z').is_none());
    }

    #[test]
    fn fc_resolve_operand_covers_positive_negative_and_string() {
        let h: Vec<Box<[u8]>> = vec![b"alpha".to_vec().into(), b"beta".to_vec().into(), b"gamma".to_vec().into()];
        assert_eq!(fc_resolve_operand(&h, b"1"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"3"), Some(2));
        assert_eq!(fc_resolve_operand(&h, b"99"), None);
        assert_eq!(fc_resolve_operand(&h, b"-1"), Some(2));
        assert_eq!(fc_resolve_operand(&h, b"-3"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"al"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"be"), Some(1));
        assert_eq!(fc_resolve_operand(&h, b"zzz"), None);
    }
}
