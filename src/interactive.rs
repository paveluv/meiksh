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
        match sys::read_fd(sys::STDIN_FILENO, &mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(line) }),
            Ok(_) => {
                line.push(byte[0] as char);
                if byte[0] == b'\n' {
                    return Ok(Some(line));
                }
            }
            Err(e) if e.is_eintr() => {
                let _ = sys::write_all_fd(sys::STDERR_FILENO, b"\n");
                return Ok(Some(String::new()));
            }
            Err(e) => return Err(e),
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
        .or_else(|| {
            shell
                .get_var("HOME")
                .map(|home| PathBuf::from(home).join(".sh_history"))
        })
        .unwrap_or_else(|| PathBuf::from(".sh_history"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellOptions;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};

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
            ignored_on_entry: BTreeSet::new(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
            interactive: false,
            errexit_suppressed: false,
        }
    }

    #[test]
    fn prompt_prefers_ps1() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(prompt(&shell), "meiksh$ ");
            shell.env.insert("PS1".into(), "custom> ".into());
            assert_eq!(prompt(&shell), "custom> ");
        });
    }

    #[test]
    fn append_history_writes_to_histfile() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/history.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"echo hi\n".to_vec())],
                    TraceResult::Int(8),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("HISTFILE".into(), "/tmp/history.txt".into());
                append_history(&shell, "echo hi\n").expect("append history");
            },
        );
    }

    #[test]
    fn load_env_file_ignores_relative_path() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.env.insert("ENV".into(), "relative.sh".into());
            load_env_file(&mut shell).expect("relative ignored");
        });
    }

    #[test]
    fn load_env_file_ignores_missing_absolute_path() {
        run_trace(
            vec![t(
                "access",
                vec![
                    ArgMatcher::Str("/tmp/meiksh-missing-env.sh".into()),
                    ArgMatcher::Int(0),
                ],
                TraceResult::Err(sys::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("ENV".into(), "/tmp/meiksh-missing-env.sh".into());
                load_env_file(&mut shell).expect("missing ignored");
            },
        );
    }

    #[test]
    fn load_env_file_sources_existing_absolute_path() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/env.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/env.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"FROM_ENV_FILE=1\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("ENV".into(), "/tmp/env.sh".into());
                load_env_file(&mut shell).expect("source env file");
                assert_eq!(shell.get_var("FROM_ENV_FILE").as_deref(), Some("1"));
            },
        );
    }

    #[test]
    fn load_env_file_expands_parameters() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/home/user/env.sh".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/home/user/env.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"FROM_EXPANDED_ENV=1\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("HOME".into(), "/home/user".into());
                shell.env.insert("ENV".into(), "${HOME}/env.sh".into());
                load_env_file(&mut shell).expect("expanded env file");
                assert_eq!(shell.get_var("FROM_EXPANDED_ENV").as_deref(), Some("1"));
            },
        );
    }

    #[test]
    fn load_env_file_respects_identity_guard() {
        run_trace(vec![], || {
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
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/bad.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/bad.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"echo 'unterminated\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("ENV".into(), "/tmp/bad.sh".into());
                let error = load_env_file(&mut shell).expect_err("invalid env file");
                assert!(!error.message.is_empty());
            },
        );
    }

    #[test]
    fn load_env_file_noop_when_env_variable_unset() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            load_env_file(&mut shell).expect("no env");
        });
    }

    #[test]
    fn run_loop_exits_on_immediate_eof() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("eof run loop");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn run_loop_covers_reaped_jobs_blank_lines_and_exit() {
        run_trace(
            vec![
                // reap_jobs for 4001 (called explicitly before run_loop)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                // reap_jobs for 4002 (called at top of run_loop)
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                // reap notification written to stderr
                t(
                    "write",
                    vec![ArgMatcher::Fd(sys::STDERR_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // first prompt
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"test$ ".to_vec()),
                    ],
                    TraceResult::Int(6),
                ),
                // read blank line
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"\n".to_vec()),
                ),
                // second iteration: reap_jobs returns nothing
                // second prompt (after blank line)
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"test$ ".to_vec()),
                    ],
                    TraceResult::Int(6),
                ),
                // read "exit 5\n" byte by byte
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"e".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"x".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"i".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"t".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b" ".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"5".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"\n".to_vec()),
                ),
                // append to history
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/history.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"exit 5\n".to_vec())],
                    TraceResult::Int(7),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("HISTFILE".into(), "/tmp/history.txt".into());
                shell.env.insert("PS1".into(), "test$ ".into());

                let handle = sys::ChildHandle {
                    pid: 4001,
                    stdout_fd: None,
                };
                shell.register_background_job("done".into(), None, vec![handle]);
                shell.reap_jobs();

                let handle = sys::ChildHandle {
                    pid: 4002,
                    stdout_fd: None,
                };
                shell.register_background_job("done".into(), None, vec![handle]);

                let status = run_loop(&mut shell).expect("run loop");

                assert_eq!(status, 5);
                assert_eq!(shell.last_status, 5);
                assert!(!shell.running);
            },
        );
    }

    #[test]
    fn run_loop_exits_cleanly_on_eof() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("eof run loop");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn run_loop_recovers_from_parse_error() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                // read "echo 'unterminated\n" byte by byte
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"e".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"c".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"h".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"o".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b" ".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"'".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"u".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"t".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"e".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"r".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"m".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"i".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"a".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"t".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"e".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"d".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"\n".to_vec()),
                ),
                // open history file
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/bad-history.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(19),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                // parse error written to stderr
                t(
                    "write",
                    vec![ArgMatcher::Fd(sys::STDERR_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // prompt again
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                // EOF
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("HISTFILE".into(), "/tmp/bad-history.txt".into());
                let status = run_loop(&mut shell).expect("parse handled");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn append_history_reports_open_error() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/history-dir".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Err(sys::EISDIR),
            )],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("HISTFILE".into(), "/tmp/history-dir".into());
                let error = append_history(&shell, "echo hi\n")
                    .expect_err("directory should not open as file");
                assert!(!error.message.is_empty());
            },
        );
    }

    #[test]
    fn read_line_propagates_non_eintr_error() {
        run_trace(
            vec![t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Err(sys::EBADF),
            )],
            || {
                let err = read_line().expect_err("should propagate EBADF");
                assert!(!err.is_eintr());
            },
        );
    }

    #[test]
    fn read_line_returns_empty_on_eintr() {
        run_trace(
            vec![
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Err(sys::EINTR),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\n".to_vec()),
                    ],
                    TraceResult::Int(1),
                ),
            ],
            || {
                let result = read_line().expect("should not fail on EINTR");
                assert_eq!(result, Some(String::new()));
            },
        );
    }

    #[test]
    fn run_loop_handles_sigint_by_redisplaying_prompt() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Err(sys::EINTR),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\n".to_vec()),
                    ],
                    TraceResult::Int(1),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDOUT_FILENO),
                        ArgMatcher::Bytes(b"meiksh$ ".to_vec()),
                    ],
                    TraceResult::Int(8),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("sigint handled");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn append_history_uses_default_path_when_histfile_unset() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/home/user/.sh_history".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(10),
                        ArgMatcher::Bytes(b"echo default\n".to_vec()),
                    ],
                    TraceResult::Int(13),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("HOME".into(), "/home/user".into());
                append_history(&shell, "echo default\n").expect("default history");
            },
        );
    }
}
