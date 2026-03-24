use std::env;
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
        "alias" => alias(shell, argv),
        "unalias" => unalias(shell, argv)?,
        "return" => return_builtin(shell, argv)?,
        "break" => break_builtin(shell, argv)?,
        "continue" => continue_builtin(shell, argv)?,
        "times" => times(),
        "trap" => trap(shell, argv),
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
            | "readonly"
            | "return"
            | "set"
            | "shift"
            | "times"
            | "trap"
            | "true"
            | "unalias"
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
                "-C" => {
                    shell.options.noclobber = true;
                    index += 1;
                }
                "+C" => {
                    shell.options.noclobber = false;
                    index += 1;
                }
                "--" => {
                    shell.set_positional(argv[index + 1..].to_vec());
                    return BuiltinOutcome::Status(0);
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

fn alias(shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.aliases.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            println!("alias {}='{}'", name, value);
        }
        return BuiltinOutcome::Status(0);
    }
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once('=') {
            shell.aliases.insert(name.to_string(), value.to_string());
        }
    }
    BuiltinOutcome::Status(0)
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
    println!("0m0.000s 0m0.000s");
    println!("0m0.000s 0m0.000s");
    BuiltinOutcome::Status(0)
}

fn trap(_shell: &mut Shell, argv: &[String]) -> BuiltinOutcome {
    if argv.len() == 1 {
        return BuiltinOutcome::Status(0);
    }
    BuiltinOutcome::Status(0)
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
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
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
            current_exe: std::env::current_exe().expect("current exe"),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        }
    }

    #[test]
    fn builtin_registry_knows_core_commands() {
        assert!(is_builtin("cd"));
        assert!(is_builtin("export"));
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

        run(&mut shell, &["unalias".into(), "ll".into()]).expect("unalias");
        assert!(!shell.aliases.contains_key("ll"));

        let error = run(&mut shell, &["unalias".into()]).expect_err("missing alias");
        assert_eq!(error.message, "unalias: name required");
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
        let _guard = cwd_lock().lock().expect("cwd lock");
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

        let outcome = run(&mut shell, &["set".into(), "-C".into(), "--".into(), "epsilon".into()])
            .expect("set -C --");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        assert!(shell.options.noclobber);
        assert_eq!(shell.positional, vec!["epsilon".to_string()]);

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

        let outcome = run(&mut shell, &["command".into(), "sh".into()]).expect("command sh");
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
        let child = std::process::Command::new("sh")
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("sleep".into(), vec![child]);

        let outcome = run(&mut shell, &["bg".into(), format!("%{id}")]).expect("bg");
        assert!(matches!(outcome, BuiltinOutcome::Status(0)));

        let outcome = run(&mut shell, &["wait".into(), format!("%{id}")]).expect("wait");
        assert!(matches!(outcome, BuiltinOutcome::Status(_)));

        let child = std::process::Command::new("sh")
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
        let child_a = std::process::Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        let child_b = std::process::Command::new("sh")
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
}
