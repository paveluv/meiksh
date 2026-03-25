use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

pub fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    load_env_file(shell)?;
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO)?;
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let stderr = io::stderr();
    run_loop(shell, &mut reader, stdout.lock(), stderr.lock())
}

fn run_loop<R: BufRead, W: Write, E: Write>(
    shell: &mut Shell,
    reader: &mut R,
    mut stdout: W,
    mut stderr: E,
) -> Result<i32, ShellError> {
    let mut line = String::new();

    loop {
        for (id, status) in shell.reap_jobs() {
            writeln!(stderr, "[{id}] Done {status}")?;
        }

        write!(stdout, "{}", prompt(shell))?;
        stdout.flush()?;
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        append_history(shell, &line)?;
        match shell.execute_string(&line) {
            Ok(status) => shell.last_status = status,
            Err(error) => {
                writeln!(stderr, "meiksh: {}", error.message)?;
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
    use std::io::{self, Cursor, Read};

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

    struct FailingWriter {
        writes_before_error: usize,
        fail_flush: bool,
    }

    impl io::Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.writes_before_error == 0 {
                return Err(io::Error::other("write failure"));
            }
            self.writes_before_error -= 1;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::other("flush failure"))
            } else {
                Ok(())
            }
        }
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("read failure"))
        }
    }

    impl io::BufRead for FailingReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::other("read failure"))
        }

        fn consume(&mut self, _amt: usize) {}
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
        let mut shell = test_shell();
        load_env_file(&mut shell).expect("no env");

        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_loop(&mut shell, &mut reader, &mut stdout, &mut stderr).expect("eof run loop");
        assert_eq!(status, 0);
        assert!(String::from_utf8(stdout).expect("stdout").contains("meiksh$ "));
        assert!(String::from_utf8(stderr).expect("stderr").is_empty());
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
            .run_with_waitpid(done_waitpid, || {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/history.txt".into());
                shell.env.insert("PS1".into(), "test$ ".into());

                let handle = sys::ChildHandle { pid: 4001, stdout_fd: None };
                shell.launch_background_job("done".into(), None, vec![handle]);
                shell.reap_jobs();

                let handle = sys::ChildHandle { pid: 4002, stdout_fd: None };
                shell.launch_background_job("done".into(), None, vec![handle]);

                let mut reader = Cursor::new(b"\nexit 5\n".to_vec());
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                let status = run_loop(&mut shell, &mut reader, &mut stdout, &mut stderr).expect("run loop");

                assert_eq!(status, 5);
                assert_eq!(shell.last_status, 5);
                assert!(!shell.running);
                assert!(String::from_utf8(stdout).expect("stdout").contains("test$ "));
                assert!(String::from_utf8(stderr).expect("stderr").contains("Done 0"));
                assert_eq!(sys::read_file("/tmp/history.txt").expect("history"), "exit 5\n");
            });
    }

    #[test]
    fn run_loop_propagates_write_flush_read_and_parse_errors() {
        use crate::sys::test_support::VfsBuilder;

        let mut shell = test_shell();
        let mut eof = Cursor::new(Vec::<u8>::new());
        let mut stderr = Vec::new();

        let error = run_loop(
            &mut shell,
            &mut eof,
            FailingWriter {
                writes_before_error: 0,
                fail_flush: false,
            },
            &mut stderr,
        )
        .expect_err("write failure");
        assert!(!error.message.is_empty());

        let mut shell = test_shell();
        let mut eof = Cursor::new(Vec::<u8>::new());
        let error = run_loop(
            &mut shell,
            &mut eof,
            FailingWriter {
                writes_before_error: 1,
                fail_flush: true,
            },
            Vec::new(),
        )
        .expect_err("flush failure");
        assert!(!error.message.is_empty());

        let mut shell = test_shell();
        let error = run_loop(&mut shell, &mut FailingReader, Vec::new(), Vec::new()).expect_err("read failure");
        assert!(!error.message.is_empty());

        VfsBuilder::new()
            .dir("/tmp")
            .run(|| {
                let mut shell = test_shell();
                shell.env.insert("HISTFILE".into(), "/tmp/bad-history.txt".into());
                let mut reader = Cursor::new(b"echo 'unterminated\n".to_vec());
                let mut stderr = Vec::new();
                let status = run_loop(&mut shell, &mut reader, Vec::new(), &mut stderr).expect("parse handled");
                assert_eq!(status, 1);
                assert!(String::from_utf8(stderr).expect("stderr").contains("unterminated"));
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
