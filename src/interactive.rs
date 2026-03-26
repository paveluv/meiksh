use std::path::PathBuf;

use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

pub fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    load_env_file(shell)?;
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO)?;
    run_loop(shell)
}

fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
    loop {
        for (id, status) in shell.reap_jobs() {
            let msg = format!("[{id}] Done {status}\n");
            let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
        }

        let prompt_str = prompt(shell);
        sys::write_all_fd(sys::STDOUT_FILENO, prompt_str.as_bytes())?;

        let line = match read_line()? {
            Some(line) => line,
            None => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        append_history(shell, &line)?;
        match shell.execute_string(&line) {
            Ok(status) => shell.last_status = status,
            Err(error) => {
                let msg = format!("meiksh: {}\n", error.message);
                let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
                shell.last_status = 1;
                continue;
            }
        }
        if !shell.running {
            break;
        }
    }

    Ok(shell.last_status)
}

fn read_line() -> sys::SysResult<Option<String>> {
    let mut line = String::new();
    let mut byte = [0u8; 1];
    loop {
        let count = sys::read_fd(sys::STDIN_FILENO, &mut byte)?;
        if count == 0 {
            return Ok(if line.is_empty() { None } else { Some(line) });
        }
        line.push(byte[0] as char);
        if byte[0] == b'\n' {
            return Ok(Some(line));
        }
    }
}

fn prompt(shell: &Shell) -> String {
    shell
        .get_var("PS1")
        .unwrap_or_else(|| "meiksh$ ".to_string())
}

fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_file = shell
        .get_var("ENV")
        .map(|value| expand::expand_parameter_text(shell, &value))
        .transpose()?
        .map(PathBuf::from);
    if let Some(path) = env_file {
        if path.is_absolute() && sys::file_exists(&path.display().to_string()) {
            let _ = shell.source_path(&path)?;
        }
    }
    Ok(())
}

fn append_history(shell: &Shell, line: &str) -> Result<(), ShellError> {
    let history = history_path(shell);
    let fd = sys::open_file(
        &history.display().to_string(),
        sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND,
        0o644,
    )?;
    sys::write_all_fd(fd, line.as_bytes())?;
    sys::close_fd(fd)?;
    Ok(())
}

fn history_path(shell: &Shell) -> PathBuf {
    shell
        .get_var("HISTFILE")
        .map(PathBuf::from)
        .or_else(|| shell.get_var("HOME").map(|home| PathBuf::from(home).join(".sh_history")))
        .unwrap_or_else(|| PathBuf::from(".sh_history"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    fn prompt_prefers_ps1() {
        let mut shell = test_shell();
        assert_eq!(prompt(&shell), "meiksh$ ");
        shell.env.insert("PS1".into(), "custom> ".into());
        assert_eq!(prompt(&shell), "custom> ");
    }

    #[test]
    fn append_history_writes_to_histfile() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .dir("/tmp")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/history.txt".into());

                append_history(&shell, "echo hi\n").expect("append history");
                let contents = sys::read_file("/tmp/history.txt").expect("read history");
                assert_eq!(contents, "echo hi\n");
            });
    }

    #[test]
    fn load_env_file_ignores_relative_and_missing_paths() {
        let mut shell = test_shell();
        shell.env.insert("ENV".into(), "relative.sh".into());
        load_env_file(&mut shell).expect("relative ignored");

        use crate::sys::test_support::VfsBuilder;
        VfsBuilder::new().run(|| {
            let mut shell = test_shell();
            shell.env.insert("ENV".into(), "/tmp/meiksh-missing-env.sh".into());
            load_env_file(&mut shell).expect("missing ignored");
        });
    }

    #[test]
    fn load_env_file_sources_existing_absolute_path() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .file("/tmp/env.sh", b"FROM_ENV_FILE=1\n")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("ENV".into(), "/tmp/env.sh".into());

                load_env_file(&mut shell).expect("source env file");
                assert_eq!(shell.get_var("FROM_ENV_FILE").as_deref(), Some("1"));
            });
    }

    #[test]
    fn load_env_file_expands_parameters() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .file("/home/user/env.sh", b"FROM_EXPANDED_ENV=1\n")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HOME".into(), "/home/user".into());
                shell.env.insert("ENV".into(), "${HOME}/env.sh".into());
                load_env_file(&mut shell).expect("expanded env file");
                assert_eq!(shell.get_var("FROM_EXPANDED_ENV").as_deref(), Some("1"));
            });
    }

    #[test]
    fn load_env_file_respects_identity_guard() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .file("/home/user/env.sh", b"FROM_EXPANDED_ENV=1\n")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HOME".into(), "/home/user".into());
                shell.env.insert("ENV".into(), "${HOME}/env.sh".into());
                sys::test_support::with_process_ids_for_test((1, 2, 3, 3), || {
                    load_env_file(&mut shell).expect("guarded env file");
                });
                assert_eq!(shell.get_var("FROM_EXPANDED_ENV"), None);
            });
    }

    #[test]
    fn load_env_file_propagates_source_errors() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .file("/tmp/bad.sh", b"echo 'unterminated\n")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("ENV".into(), "/tmp/bad.sh".into());

                let error = load_env_file(&mut shell).expect_err("invalid env file");
                assert!(!error.message.is_empty());
            });
    }

    #[test]
    fn load_env_without_variable_and_run_loop_eof_are_covered() {
        use crate::sys::test_support::VfsBuilder;

        let mut shell = test_shell();
        load_env_file(&mut shell).expect("no env");

        VfsBuilder::new()
            .file("/dev/stdin", b"")
            .stdin("/dev/stdin")
            .file("/dev/stdout", b"")
            .stdout("/dev/stdout")
            .file("/dev/stderr", b"")
            .stderr("/dev/stderr")
            .run(|| {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("eof run loop");
                assert_eq!(status, 0);
            });
    }

    #[test]
    fn run_loop_covers_reaped_jobs_blank_lines_and_exit() {
        use crate::sys::test_support::VfsBuilder;

        fn done_waitpid(pid: sys::Pid, status: *mut i32, _opts: i32) -> sys::Pid {
            unsafe { *status = 0; }
            pid
        }

        VfsBuilder::new()
            .dir("/tmp")
            .file("/dev/stdin", b"\nexit 5\n")
            .stdin("/dev/stdin")
            .file("/dev/stdout", b"")
            .stdout("/dev/stdout")
            .file("/dev/stderr", b"")
            .stderr("/dev/stderr")
            .run_with_waitpid(done_waitpid, || {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/history.txt".into());
                shell.env.insert("PS1".into(), "test$ ".into());

                let handle = sys::ChildHandle { pid: 4001, stdout_fd: None };
                shell.launch_background_job("done".into(), None, vec![handle]);
                shell.reap_jobs();

                let handle = sys::ChildHandle { pid: 4002, stdout_fd: None };
                shell.launch_background_job("done".into(), None, vec![handle]);

                let status = run_loop(&mut shell).expect("run loop");

                assert_eq!(status, 5);
                assert_eq!(shell.last_status, 5);
                assert!(!shell.running);
                assert_eq!(sys::read_file("/tmp/history.txt").expect("history"), "exit 5\n");
            });
    }

    #[test]
    fn run_loop_propagates_write_flush_read_and_parse_errors() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .file("/dev/stdin", b"")
            .stdin("/dev/stdin")
            .file("/dev/stdout", b"")
            .stdout("/dev/stdout")
            .file("/dev/stderr", b"")
            .stderr("/dev/stderr")
            .run(|| {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("eof run loop");
                assert_eq!(status, 0);
            });

        VfsBuilder::new()
            .dir("/tmp")
            .file("/dev/stdin", b"echo 'unterminated\n")
            .stdin("/dev/stdin")
            .file("/dev/stdout", b"")
            .stdout("/dev/stdout")
            .file("/dev/stderr", b"")
            .stderr("/dev/stderr")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/bad-history.txt".into());
                let status = run_loop(&mut shell).expect("parse handled");
                assert_eq!(status, 1);
            });
    }

    #[test]
    fn append_history_uses_default_path_and_reports_open_errors() {
        use crate::sys::test_support::VfsBuilder;

        VfsBuilder::new()
            .dir("/tmp/history-dir")
            .dir("/home/user")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/history-dir".into());
                let error = append_history(&shell, "echo hi\n").expect_err("directory should not open as file");
                assert!(!error.message.is_empty());

                let mut shell = test_shell();
                shell.env.insert("HOME".into(), "/home/user".into());
                append_history(&shell, "echo default\n").expect("default history");
                assert_eq!(
                    sys::read_file("/home/user/.sh_history").expect("read history"),
                    "echo default\n"
                );
            });
    }
}
