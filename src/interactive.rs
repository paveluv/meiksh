use std::path::PathBuf;

use crate::arena::StringArena;
use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

pub fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| shell.diagnostic(1, &e))?;
    run_loop(shell)
}

fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
    let mut accumulated = String::new();
    let mut sigchld_installed = false;
    loop {
        if shell.options.notify && !sigchld_installed {
            let _ = sys::install_shell_signal_handler(sys::SIGCHLD);
            sigchld_installed = true;
        } else if !shell.options.notify && sigchld_installed {
            let _ = sys::default_signal_action(sys::SIGCHLD);
            sigchld_installed = false;
        }

        for (id, state) in shell.reap_jobs() {
            use crate::shell::ReapedJobState;
            let msg = match state {
                ReapedJobState::Done(status, cmd) => {
                    if status == 0 {
                        format!("[{id}] Done\t{cmd}\n")
                    } else {
                        format!("[{id}] Done({status})\t{cmd}\n")
                    }
                }
                ReapedJobState::Signaled(sig, cmd) => {
                    format!("[{id}] Terminated ({})\t{cmd}\n", sys::signal_name(sig))
                }
                ReapedJobState::Stopped(sig, cmd) => {
                    format!("[{id}] Stopped ({})\t{cmd}\n", sys::signal_name(sig))
                }
            };
            let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
        }

        shell.run_pending_traps()?;
        if !shell.running {
            break;
        }

        let prompt_str = if accumulated.is_empty() {
            expand_prompt(shell, "PS1", "$ ")
        } else {
            expand_prompt(shell, "PS2", "> ")
        };
        write_prompt(&prompt_str).map_err(|e| shell.diagnostic(1, &e))?;

        let line = match read_line().map_err(|e| shell.diagnostic(1, &e))? {
            Some(line) => line,
            None => {
                if !accumulated.is_empty() {
                    let msg = format!("meiksh: unexpected EOF while looking for matching token\n");
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, msg.as_bytes());
                    accumulated.clear();
                }
                break;
            }
        };
        if accumulated.is_empty() && line.trim().is_empty() {
            continue;
        }
        accumulated.push_str(&line);

        match crate::syntax::parse_with_aliases(&accumulated, &shell.aliases) {
            Ok(_) => {}
            Err(ref e) if shell.input_is_incomplete(e) => {
                continue;
            }
            Err(_) => {}
        }

        let source = std::mem::take(&mut accumulated);
        append_history(shell, source.trim_end())?;
        let trimmed = source.trim();
        if !(trimmed == "fc" || trimmed.starts_with("fc ") || trimmed.starts_with("fc\t")) {
            shell.add_history(trimmed);
        }
        match shell.execute_string(&source) {
            Ok(status) => shell.last_status = status,
            Err(error) => {
                shell.last_status = error.exit_status();
                continue;
            }
        }
        if !shell.running {
            break;
        }
    }

    Ok(shell.last_status)
}

fn write_prompt(prompt_str: &str) -> sys::SysResult<()> {
    loop {
        match sys::write_all_fd(sys::STDERR_FILENO, prompt_str.as_bytes()) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
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

fn expand_prompt(shell: &mut Shell, var: &str, default: &str) -> String {
    let raw = shell.get_var(var).unwrap_or(default).to_string();
    let histnum = shell.history_number();
    let arena = StringArena::new();
    let expanded = expand::expand_parameter_text(shell, &raw, &arena).unwrap_or(&raw);
    expand_prompt_exclamation(expanded, histnum)
}

fn expand_prompt_exclamation(s: &str, histnum: usize) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '!' {
            match chars.next() {
                Some('!') => result.push('!'),
                Some(other) => {
                    result.push_str(&histnum.to_string());
                    result.push(other);
                }
                None => result.push_str(&histnum.to_string()),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_value = shell.get_var("ENV").map(|s| s.to_string());
    let arena = StringArena::new();
    let env_file = env_value
        .map(|value| expand::expand_parameter_text(shell, &value, &arena).map(|s| s.to_string()))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?
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
    )
    .map_err(|e| shell.diagnostic(1, &e))?;
    let mut entry = line.to_string();
    if !entry.ends_with('\n') {
        entry.push('\n');
    }
    sys::write_all_fd(fd, entry.as_bytes()).map_err(|e| shell.diagnostic(1, &e))?;
    sys::close_fd(fd).map_err(|e| shell.diagnostic(1, &e))?;
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
    use crate::shell::{ShellOptions, TrapAction, TrapCondition};
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::sys::test_support::{
        ArgMatcher, TraceEntry, TraceResult, assert_no_syscalls, run_trace, t,
    };

    fn read_line_trace(input: &[u8]) -> Vec<TraceEntry> {
        input
            .iter()
            .map(|&b| {
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(vec![b]),
                )
            })
            .collect()
    }

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
            last_status: 0,
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
            path_cache: std::collections::HashMap::new(),
            history: Vec::new(),
        }
    }

    #[test]
    fn prompt_prefers_ps1() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(expand_prompt(&mut shell, "PS1", "$ "), "$ ");
            shell.env.insert("PS1".into(), "custom> ".into());
            assert_eq!(expand_prompt(&mut shell, "PS1", "$ "), "custom> ");
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
                    TraceResult::Auto,
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
                assert_eq!(shell.get_var("FROM_ENV_FILE"), Some("1"));
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
                assert_eq!(shell.get_var("FROM_EXPANDED_ENV"), Some("1"));
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
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 2: unterminated single quote\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("ENV".into(), "/tmp/bad.sh".into());
                let error = load_env_file(&mut shell).expect_err("invalid env file");
                assert_ne!(error.exit_status(), 0);
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
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] Done\tdone\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // first prompt
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"test$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"test$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
                    TraceResult::Auto,
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
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
        let mut trace = vec![t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(b"$ ".to_vec()),
            ],
            TraceResult::Auto,
        )];
        for b in b"echo 'unterminated\n" {
            trace.push(t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Bytes(vec![*b]),
            ));
        }
        trace.extend_from_slice(&[
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"> ".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(
                        b"meiksh: unexpected EOF while looking for matching token\n".to_vec(),
                    ),
                ],
                TraceResult::Auto,
            ),
        ]);
        run_trace(trace, || {
            let mut shell = test_shell();
            shell
                .env
                .insert("HISTFILE".into(), "/tmp/bad-history.txt".into());
            let status = run_loop(&mut shell).expect("parse handled");
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn append_history_reports_open_error() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/history-dir".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EISDIR),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: Is a directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert("HISTFILE".into(), "/tmp/history-dir".into());
                let error = append_history(&shell, "echo hi\n")
                    .expect_err("directory should not open as file");
                assert_ne!(error.exit_status(), 0);
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
                    TraceResult::Auto,
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
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
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
                    TraceResult::Auto,
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

    #[test]
    fn run_loop_prints_stopped_and_running_reap_notifications() {
        run_trace(
            vec![
                // reap_jobs: job 4010 is stopped
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4010), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                // reap_jobs: check if 4010 was subsequently continued
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4010), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                // reap_jobs: job 4011 is still running
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4011), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                // Stopped notification written to stderr
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] Stopped (SIGTSTP)\tvim\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // prompt
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // read EOF
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"".to_vec()),
                ),
            ],
            || {
                let mut shell = test_shell();
                let handle_stopped = sys::ChildHandle {
                    pid: 4010,
                    stdout_fd: None,
                };
                shell.register_background_job("vim".into(), None, vec![handle_stopped]);
                let handle_running = sys::ChildHandle {
                    pid: 4011,
                    stdout_fd: None,
                };
                shell.register_background_job("sleep 999".into(), None, vec![handle_running]);
                let status = run_loop(&mut shell).expect("run loop");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn run_loop_fires_trap_on_sigint_at_prompt() {
        run_trace(
            vec![
                // set_trap installs signal handler
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // first iteration: no pending traps, prompt
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // read interrupted by SIGINT
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                // read_line writes newline to stderr on EINTR
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // second iteration: run_pending_traps drains SIGINT, runs "TRAPPED=yes" (no syscalls)
                // then prompt again
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // read EOF
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command("TRAPPED=yes".into())),
                    )
                    .expect("trap");
                let status = run_loop(&mut shell).expect("trap at prompt");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("TRAPPED"), Some("yes"));
            },
        );
    }

    #[test]
    fn run_loop_exit_trap_on_sigint_stops_shell() {
        run_trace(
            vec![
                // set_trap installs signal handler
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                // first iteration: prompt
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // read interrupted by SIGINT
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                // read_line writes newline to stderr
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // second iteration: run_pending_traps runs "exit 42", shell.running = false → break
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command("exit 42".into())),
                    )
                    .expect("trap");
                let status = run_loop(&mut shell).expect("exit trap at prompt");
                assert_eq!(status, 42);
                assert!(!shell.running);
            },
        );
    }

    #[test]
    fn run_loop_retries_prompt_write_on_eintr() {
        run_trace(
            vec![
                // prompt write fails with EINTR
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Err(sys::EINTR),
                ),
                // retry succeeds
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                // read EOF
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = run_loop(&mut shell).expect("prompt eintr retry");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn run_loop_propagates_prompt_write_error() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Err(sys::EIO),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: Input/output error\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let result = run_loop(&mut shell);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn run_loop_command_not_found_sets_status_127_and_continues() {
        let mut trace = vec![
            // prompt
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
        // read "gibberish\n"
        trace.extend(read_line_trace(b"gibberish\n"));
        trace.extend([
            // history: open, write, close
            t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/hist".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(10),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(10),
                    ArgMatcher::Bytes(b"gibberish\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            // resolve_command_path: stat "/usr/bin/gibberish" → not found
            t(
                "stat",
                vec![
                    ArgMatcher::Str("/usr/bin/gibberish".into()),
                    ArgMatcher::Any,
                ],
                TraceResult::Err(sys::ENOENT),
            ),
            // "not found" diagnostic written to stderr (no fork!)
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"gibberish: not found\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // shell continues: second prompt
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
            // read EOF
            t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
        ]);

        run_trace(trace, || {
            let mut shell = test_shell();
            shell.env.insert("PATH".into(), "/usr/bin".into());
            shell.env.insert("HISTFILE".into(), "/tmp/hist".into());
            let status = run_loop(&mut shell).expect("command not found handled");
            assert_eq!(
                status, 127,
                "exit status should be 127 for command not found"
            );
        });
    }

    #[test]
    fn expand_prompt_exclamation_covers_all_branches() {
        assert_no_syscalls(|| {
            assert_eq!(expand_prompt_exclamation("!!", 42), "!");
            assert_eq!(expand_prompt_exclamation("!x", 42), "42x");
            assert_eq!(expand_prompt_exclamation("!", 42), "42");
            assert_eq!(expand_prompt_exclamation("no bang", 42), "no bang");
        });
    }

    #[test]
    fn run_loop_syntax_error_prints_error_and_continues() {
        let mut trace = Vec::new();

        let prompt = t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(b"$ ".to_vec()),
            ],
            TraceResult::Int(2),
        );
        trace.push(prompt.clone());
        trace.extend(read_line_trace(b"$(\n"));

        trace.push(t(
            "open",
            vec![
                ArgMatcher::Str("/tmp/hist".into()),
                ArgMatcher::Any,
                ArgMatcher::Any,
            ],
            TraceResult::Fd(10),
        ));
        trace.push(t(
            "write",
            vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"$(\n".to_vec())],
            TraceResult::Int(3),
        ));
        trace.push(t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)));

        let err_msg = b"meiksh: line 2: unterminated command substitution\n";
        trace.push(t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(err_msg.to_vec()),
            ],
            TraceResult::Int(err_msg.len() as i64),
        ));

        trace.push(prompt.clone());
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
            TraceResult::Bytes(vec![]),
        ));

        run_trace(trace, || {
            let mut shell = test_shell();
            shell.env.insert("HISTFILE".into(), "/tmp/hist".into());
            let _ = run_loop(&mut shell);
        });
    }

    #[test]
    fn run_loop_sigchld_install_and_remove() {
        let mut trace = vec![
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGCHLD as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
        trace.extend(read_line_trace(b"set +b\n"));
        trace.extend(vec![
            t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/hist".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(10),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"set +b\n".to_vec())],
                TraceResult::Auto,
            ),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGCHLD as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Bytes(vec![]),
            ),
        ]);
        run_trace(trace, || {
            let mut shell = test_shell();
            shell.options.notify = true;
            shell.env.insert("HISTFILE".into(), "/tmp/hist".into());
            let _ = run_loop(&mut shell);
        });
    }

    #[test]
    fn run_loop_signaled_and_done_nonzero_notifications() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(6001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::SignaledSig(15),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(6002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(7),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[1] Terminated (SIGTERM)\tkilled\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"[2] Done(7)\tfailed\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"$ ".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(vec![]),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    "killed".into(),
                    None,
                    vec![sys::ChildHandle {
                        pid: 6001,
                        stdout_fd: None,
                    }],
                );
                shell.register_background_job(
                    "failed".into(),
                    None,
                    vec![sys::ChildHandle {
                        pid: 6002,
                        stdout_fd: None,
                    }],
                );
                let _ = run_loop(&mut shell);
            },
        );
    }
}
