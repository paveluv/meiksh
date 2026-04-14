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
        shell_name: b"meiksh"[..].into(),
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
        mail_last_check: 0,
        mail_sizes: std::collections::HashMap::new(),
    }
}

#[test]
fn prompt_prefers_ps1() {
    assert_no_syscalls(|| {
        let mut shell = test_shell();
        assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"$ ");
        shell.env.insert(b"PS1".to_vec(), b"custom> ".to_vec());
        assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"custom> ");
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
                .insert(b"HISTFILE".to_vec(), b"/tmp/history.txt".to_vec());
            append_history(&shell, b"echo hi\n").expect("append history");
        },
    );
}

#[test]
fn load_env_file_ignores_relative_path() {
    run_trace(vec![], || {
        let mut shell = test_shell();
        shell.env.insert(b"ENV".to_vec(), b"relative.sh".to_vec());
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
                .insert(b"ENV".to_vec(), b"/tmp/meiksh-missing-env.sh".to_vec());
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
            shell.env.insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
            load_env_file(&mut shell).expect("source env file");
            assert_eq!(shell.get_var(b"FROM_ENV_FILE"), Some(b"1".as_ref()));
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
            shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
            shell
                .env
                .insert(b"ENV".to_vec(), b"${HOME}/env.sh".to_vec());
            load_env_file(&mut shell).expect("expanded env file");
            assert_eq!(shell.get_var(b"FROM_EXPANDED_ENV"), Some(b"1".as_ref()));
        },
    );
}

#[test]
fn load_env_file_respects_identity_guard() {
    run_trace(vec![], || {
        let mut shell = test_shell();
        shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
        shell
            .env
            .insert(b"ENV".to_vec(), b"${HOME}/env.sh".to_vec());
        sys::test_support::with_process_ids_for_test((1, 2, 3, 3), || {
            load_env_file(&mut shell).expect("guarded env file");
        });
        assert_eq!(shell.get_var(b"FROM_EXPANDED_ENV"), None);
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
            shell.env.insert(b"ENV".to_vec(), b"/tmp/bad.sh".to_vec());
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
            t(
                "waitpid",
                vec![ArgMatcher::Int(4001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0),
            ),
            t(
                "waitpid",
                vec![ArgMatcher::Int(4002), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"[1] Done\tdone\n".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"test$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Bytes(b"\n".to_vec()),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"test$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
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
                .insert(b"HISTFILE".to_vec(), b"/tmp/history.txt".to_vec());
            shell.env.insert(b"PS1".to_vec(), b"test$ ".to_vec());

            let handle = sys::ChildHandle {
                pid: 4001,
                stdout_fd: None,
            };
            shell.register_background_job(b"done"[..].into(), None, vec![handle]);
            shell.reap_jobs();

            let handle = sys::ChildHandle {
                pid: 4002,
                stdout_fd: None,
            };
            shell.register_background_job(b"done"[..].into(), None, vec![handle]);

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
            .insert(b"HISTFILE".to_vec(), b"/tmp/bad-history.txt".to_vec());
        let status = run_loop(&mut shell).expect("parse handled");
        assert_eq!(status, 0);
    });
}

#[test]
fn append_history_silently_ignores_open_error() {
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
                .insert(b"HISTFILE".to_vec(), b"/tmp/history-dir".to_vec());
            append_history(&shell, b"echo hi\n").expect("should silently succeed");
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
            assert_eq!(result, Some(Vec::new()));
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
            shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
            append_history(&shell, b"echo default\n").expect("default history");
        },
    );
}

#[test]
fn run_loop_prints_stopped_and_running_reap_notifications() {
    run_trace(
        vec![
            t(
                "waitpid",
                vec![ArgMatcher::Int(4010), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::StoppedSig(sys::SIGTSTP),
            ),
            t(
                "waitpid",
                vec![ArgMatcher::Int(4010), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Pid(0),
            ),
            t(
                "waitpid",
                vec![ArgMatcher::Int(4011), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Pid(0),
            ),
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"[1] Stopped (SIGTSTP)\tvim\n".to_vec()),
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
                TraceResult::Bytes(b"".to_vec()),
            ),
        ],
        || {
            let mut shell = test_shell();
            let handle_stopped = sys::ChildHandle {
                pid: 4010,
                stdout_fd: None,
            };
            shell.register_background_job(b"vim"[..].into(), None, vec![handle_stopped]);
            let handle_running = sys::ChildHandle {
                pid: 4011,
                stdout_fd: None,
            };
            shell.register_background_job(b"sleep 999"[..].into(), None, vec![handle_running]);
            let status = run_loop(&mut shell).expect("run loop");
            assert_eq!(status, 0);
        },
    );
}

#[test]
fn run_loop_fires_trap_on_sigint_at_prompt() {
    run_trace(
        vec![
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
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
                TraceResult::Interrupt(sys::SIGINT),
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
            shell
                .set_trap(
                    TrapCondition::Signal(sys::SIGINT),
                    Some(TrapAction::Command(b"TRAPPED=yes"[..].into())),
                )
                .expect("trap");
            let status = run_loop(&mut shell).expect("trap at prompt");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"TRAPPED"), Some(b"yes".as_ref()));
        },
    );
}

#[test]
fn run_loop_exit_trap_on_sigint_stops_shell() {
    run_trace(
        vec![
            t(
                "signal",
                vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
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
                TraceResult::Interrupt(sys::SIGINT),
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
            let mut shell = test_shell();
            shell
                .set_trap(
                    TrapCondition::Signal(sys::SIGINT),
                    Some(TrapAction::Command(b"exit 42"[..].into())),
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
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Err(sys::EINTR),
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
    let mut trace = vec![t(
        "write",
        vec![
            ArgMatcher::Fd(sys::STDERR_FILENO),
            ArgMatcher::Bytes(b"$ ".to_vec()),
        ],
        TraceResult::Auto,
    )];
    trace.extend(read_line_trace(b"gibberish\n"));
    trace.extend([
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
        t(
            "stat",
            vec![
                ArgMatcher::Str("/usr/bin/gibberish".into()),
                ArgMatcher::Any,
            ],
            TraceResult::Err(sys::ENOENT),
        ),
        t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(b"gibberish: not found\n".to_vec()),
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
    ]);

    run_trace(trace, || {
        let mut shell = test_shell();
        shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
        shell
            .env
            .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
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
        assert_eq!(expand_prompt_exclamation(b"!!", 42), b"!");
        assert_eq!(expand_prompt_exclamation(b"!x", 42), b"42x");
        assert_eq!(expand_prompt_exclamation(b"!", 42), b"42");
        assert_eq!(expand_prompt_exclamation(b"no bang", 42), b"no bang");
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
        shell
            .env
            .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
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
        shell
            .env
            .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
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
                b"killed"[..].into(),
                None,
                vec![sys::ChildHandle {
                    pid: 6001,
                    stdout_fd: None,
                }],
            );
            shell.register_background_job(
                b"failed"[..].into(),
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

#[test]
fn run_loop_vi_mode_exits_on_eof() {
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
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            shell.options.vi_mode = true;
            let status = run_loop(&mut shell).expect("vi mode eof");
            assert_eq!(status, 0);
        },
    );
}

mod vi_tests {
    use super::super::vi::*;
    use super::super::{check_mail, command_is_fc};
    use crate::sys::test_support::assert_no_syscalls;

    fn has_return(actions: &[ViAction]) -> bool {
        actions.iter().any(|a| matches!(a, ViAction::Return(_)))
    }

    fn get_return(actions: &[ViAction]) -> Option<Option<Vec<u8>>> {
        actions.iter().find_map(|a| match a {
            ViAction::Return(s) => Some(s.clone()),
            _ => None,
        })
    }

    fn has_bell(actions: &[ViAction]) -> bool {
        actions.iter().any(|a| matches!(a, ViAction::Bell))
    }

    fn feed_bytes(state: &mut ViState, bytes: &[u8], history: &[Box<[u8]>]) -> Vec<ViAction> {
        let mut all = Vec::new();
        for &b in bytes {
            all.extend(state.process_byte(b, history));
        }
        all
    }

    #[test]
    fn word_forward_covers_all_branches() {
        assert_no_syscalls(|| {
            assert_eq!(word_forward(b"hello world", 0), 6);
            assert_eq!(word_forward(b"hello world", 5), 6);
            assert_eq!(word_forward(b"hello", 5), 5);
            assert_eq!(word_forward(b"a.b cd", 1), 2);
            assert_eq!(word_forward(b"   a", 0), 3);
        });
    }

    #[test]
    fn word_backward_covers_all_branches() {
        assert_no_syscalls(|| {
            assert_eq!(word_backward(b"hello world", 6), 0);
            assert_eq!(word_backward(b"hello world", 11), 6);
            assert_eq!(word_backward(b"hello", 0), 0);
            assert_eq!(word_backward(b"a.b cd", 3), 2);
            assert_eq!(word_backward(b"  ab", 4), 2);
        });
    }

    #[test]
    fn bigword_forward_and_backward() {
        assert_no_syscalls(|| {
            assert_eq!(bigword_forward(b"a.b c.d", 0), 4);
            assert_eq!(bigword_forward(b"abc", 0), 3);
            assert_eq!(bigword_backward(b"a.b c.d", 4), 0);
            assert_eq!(bigword_backward(b"a.b c.d", 0), 0);
            assert_eq!(bigword_backward(b"ab   cd", 7), 5);
        });
    }

    #[test]
    fn word_end_and_bigword_end() {
        assert_no_syscalls(|| {
            assert_eq!(word_end(b"ab cd", 0), 1);
            assert_eq!(word_end(b"ab cd", 2), 4);
            assert_eq!(word_end(b"a", 0), 0);
            assert_eq!(word_end(b"a  b", 0), 3);
            assert_eq!(bigword_end(b"a.b c.d", 0), 2);
            assert_eq!(bigword_end(b"a", 0), 0);
            assert_eq!(bigword_end(b"a  b", 0), 3);
            assert_eq!(bigword_end(b"ab", 1), 1);
        });
    }

    #[test]
    fn is_word_char_tests() {
        assert_no_syscalls(|| {
            assert!(is_word_char(b'a'));
            assert!(is_word_char(b'Z'));
            assert!(is_word_char(b'0'));
            assert!(is_word_char(b'_'));
            assert!(!is_word_char(b'.'));
            assert!(!is_word_char(b' '));
        });
    }

    #[test]
    fn do_find_all_directions() {
        assert_no_syscalls(|| {
            let line = b"abcba";
            assert_eq!(do_find(line, 0, b'f', b'c'), Some(2));
            assert_eq!(do_find(line, 0, b'f', b'z'), None);
            assert_eq!(do_find(line, 4, b'F', b'c'), Some(2));
            assert_eq!(do_find(line, 0, b'F', b'c'), None);
            assert_eq!(do_find(line, 0, b't', b'c'), Some(1));
            assert_eq!(do_find(line, 0, b't', b'z'), None);
            assert_eq!(do_find(line, 4, b'T', b'c'), Some(3));
            assert_eq!(do_find(line, 0, b'T', b'c'), None);
            assert_eq!(do_find(line, 0, b'z', b'a'), None);
        });
    }

    #[test]
    fn resolve_motion_covers_all_motions() {
        assert_no_syscalls(|| {
            let line = b"hello world";
            assert_eq!(resolve_motion(line, 0, b'w', 1), (0, 6));
            assert_eq!(resolve_motion(line, 6, b'b', 1), (0, 6));
            assert_eq!(resolve_motion(line, 0, b'W', 1), (0, 6));
            assert_eq!(resolve_motion(line, 6, b'B', 1), (0, 6));
            assert_eq!(resolve_motion(line, 0, b'e', 1), (0, 5));
            assert_eq!(resolve_motion(line, 0, b'E', 1), (0, 5));
            assert_eq!(resolve_motion(line, 5, b'h', 3), (2, 5));
            assert_eq!(resolve_motion(line, 2, b'l', 3), (2, 5));
            assert_eq!(resolve_motion(line, 5, b'0', 1), (0, 5));
            assert_eq!(resolve_motion(line, 0, b'$', 1), (0, 11));
            assert_eq!(resolve_motion(line, 0, b'z', 1), (0, 0));
        });
    }

    #[test]
    fn replay_cmd_x_and_X() {
        assert_no_syscalls(|| {
            let mut line = b"abcde".to_vec();
            let mut cursor = 2;
            let mut yank = Vec::new();
            replay_cmd(&mut line, &mut cursor, &mut yank, b'x', 2, None);
            assert_eq!(line, b"abe");
            assert_eq!(cursor, 2);

            let mut line = b"abcde".to_vec();
            cursor = 3;
            replay_cmd(&mut line, &mut cursor, &mut yank, b'X', 2, None);
            assert_eq!(line, b"ade");
            assert_eq!(cursor, 1);
        });
    }

    #[test]
    fn replay_cmd_r() {
        assert_no_syscalls(|| {
            let mut line = b"abcde".to_vec();
            let mut cursor = 1;
            let mut yank = Vec::new();
            replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 3, Some(b'Z'));
            assert_eq!(line, b"aZZZe");
            assert_eq!(cursor, 3);
        });
    }

    #[test]
    fn replay_cmd_d_dd_and_motion() {
        assert_no_syscalls(|| {
            let mut line = b"hello world".to_vec();
            let mut cursor = 0;
            let mut yank = Vec::new();
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
            assert_eq!(line, b"world");
            assert_eq!(yank, b"hello ");

            let mut line = b"hello".to_vec();
            cursor = 0;
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'd'));
            assert!(line.is_empty());
            assert_eq!(cursor, 0);
        });
    }

    #[test]
    fn replay_cmd_c_cc_and_motion() {
        assert_no_syscalls(|| {
            let mut line = b"hello world".to_vec();
            let mut cursor = 0;
            let mut yank = Vec::new();
            replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'w'));
            assert_eq!(line, b"world");
            assert_eq!(cursor, 0);

            let mut line = b"hello".to_vec();
            cursor = 0;
            replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'c'));
            assert!(line.is_empty());
        });
    }

    #[test]
    fn replay_cmd_unknown_is_noop() {
        assert_no_syscalls(|| {
            let mut line = b"ab".to_vec();
            let mut cursor = 0;
            let mut yank = Vec::new();
            replay_cmd(&mut line, &mut cursor, &mut yank, b'z', 1, None);
            assert_eq!(line, b"ab");
        });
    }

    #[test]
    fn glob_expand_with_real_files() {
        let dir = std::env::temp_dir().join("meiksh_glob_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("aaa_1"), "").unwrap();
        std::fs::write(dir.join("aaa_2"), "").unwrap();
        let pat = format!("{}/aaa_*", dir.display());
        let result = glob_expand(pat.as_bytes());
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn glob_expand_no_match_returns_err() {
        let result = glob_expand(b"/nonexistent_path_xyz/no_*_match");
        assert!(result.is_err());
    }

    #[test]
    fn command_is_fc_tests() {
        assert_no_syscalls(|| {
            assert!(command_is_fc(b"fc"));
            assert!(command_is_fc(b"fc -l"));
            assert!(command_is_fc(b"fc\t-l"));
            assert!(command_is_fc(b"FCEDIT=true fc -e true"));
            assert!(command_is_fc(b"A=1 B=2 fc"));
            assert!(command_is_fc(b"X='val' fc -s"));
            assert!(command_is_fc(b"X=\"val\" fc -s"));
            assert!(!command_is_fc(b"echo fc"));
            assert!(!command_is_fc(b""));
            assert!(!command_is_fc(b"echo hello"));
            assert!(!command_is_fc(b"FCEDIT=true"));
        });
    }

    #[test]
    fn vi_insert_mode_enter_returns_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            let actions = feed_bytes(&mut state, b"abc\n", &history);
            assert!(has_return(&actions));
            assert_eq!(get_return(&actions), Some(Some(b"abc\n".to_vec())));
        });
    }

    #[test]
    fn vi_insert_mode_eof_returns_none() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            let actions = state.process_byte(0x04, &history);
            assert!(has_return(&actions));
            assert_eq!(get_return(&actions), Some(None));
        });
    }

    #[test]
    fn vi_insert_mode_backspace_erases() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            assert_eq!(state.line, b"abc");
            state.process_byte(0x7f, &history);
            assert_eq!(state.line, b"ab");
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_insert_mode_ctrl_c_returns_empty() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            let actions = state.process_byte(0x03, &history);
            assert_eq!(get_return(&actions), Some(Some(Vec::new())));
        });
    }

    #[test]
    fn vi_insert_mode_ctrl_w_deletes_word() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x17, &history);
            assert_eq!(state.line, b"hello ");
        });
    }

    #[test]
    fn vi_insert_mode_ctrl_v_inserts_literal() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.process_byte(0x16, &history);
            state.process_byte(0x03, &history);
            assert_eq!(state.line, vec![0x03]);
        });
    }

    #[test]
    fn vi_esc_to_command_mode() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            assert!(!state.insert_mode);
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_command_h_l_motion() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcde", &history);
            state.process_byte(0x1b, &history);
            assert_eq!(state.cursor, 4);
            state.process_byte(b'h', &history);
            assert_eq!(state.cursor, 3);
            state.process_byte(b'h', &history);
            assert_eq!(state.cursor, 2);
            state.process_byte(b'l', &history);
            assert_eq!(state.cursor, 3);
        });
    }

    #[test]
    fn vi_command_0_dollar() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcde", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            assert_eq!(state.cursor, 0);
            state.process_byte(b'$', &history);
            assert_eq!(state.cursor, 4);
        });
    }

    #[test]
    fn vi_command_caret() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"  hello", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'^', &history);
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_command_w_b_motion() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"echo hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'w', &history);
            assert_eq!(state.cursor, 5);
            state.process_byte(b'w', &history);
            assert_eq!(state.cursor, 11);
            state.process_byte(b'b', &history);
            assert_eq!(state.cursor, 5);
        });
    }

    #[test]
    fn vi_command_W_B_motion() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a.b c.d", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'W', &history);
            assert_eq!(state.cursor, 4);
            state.process_byte(b'B', &history);
            assert_eq!(state.cursor, 0);
        });
    }

    #[test]
    fn vi_command_e_E_motion() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ab cd", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'e', &history);
            assert_eq!(state.cursor, 1);

            let mut state = ViState::new(0x7f, 0);
            feed_bytes(&mut state, b"a-b cd", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'E', &history);
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_command_pipe() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcde", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"3|", &history);
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_command_find_f_F_t_T() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcba", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"fc", &history);
            assert_eq!(state.cursor, 2);
            feed_bytes(&mut state, b"Fb", &history);
            assert_eq!(state.cursor, 1);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"tc", &history);
            assert_eq!(state.cursor, 1);
            state.process_byte(b'$', &history);
            feed_bytes(&mut state, b"Tb", &history);
            assert_eq!(state.cursor, 4);
        });
    }

    #[test]
    fn vi_command_semicolon_comma_repeat_find() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ababa", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"fa", &history);
            assert_eq!(state.cursor, 2);
            state.process_byte(b';', &history);
            assert_eq!(state.cursor, 4);
            state.process_byte(b',', &history);
            assert_eq!(state.cursor, 2);
        });
    }

    #[test]
    fn vi_command_x_delete() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'x', &history);
            assert_eq!(state.line, b"bc");
        });
    }

    #[test]
    fn vi_command_X_delete_before() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'X', &history);
            assert_eq!(state.line, b"ac");
            assert_eq!(state.cursor, 1);
        });
    }

    #[test]
    fn vi_command_r_replace() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"rH", &history);
            assert_eq!(state.line, b"Hello");
        });
    }

    #[test]
    fn vi_command_r_with_count() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcd", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"3rZ", &history);
            assert_eq!(state.line, b"ZZZd");
        });
    }

    #[test]
    fn vi_command_R_replace_mode() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcdef", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"RXY\x1b", &history);
            assert_eq!(state.line, b"XYcdef");
        });
    }

    #[test]
    fn vi_command_R_enter_returns() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ab", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'R', &history);
            let actions = state.process_byte(b'\n', &history);
            assert!(has_return(&actions));
        });
    }

    #[test]
    fn vi_command_tilde_toggle_case() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"aB", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'~', &history);
            assert_eq!(state.line, b"AB");
            state.process_byte(b'~', &history);
            assert_eq!(state.line, b"Ab");
        });
    }

    #[test]
    fn vi_command_d_with_motion() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"dw", &history);
            assert_eq!(state.line, b"world");
            assert_eq!(state.yank_buf, b"hello ");
        });
    }

    #[test]
    fn vi_command_dd_clears_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"dd", &history);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_command_D_delete_to_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'w', &history);
            state.process_byte(b'D', &history);
            assert_eq!(state.line, b"hello ");
        });
    }

    #[test]
    fn vi_command_c_change() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"cw", &history);
            assert_eq!(state.line, b"world");
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_command_cc_clears_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"cc", &history);
            assert!(state.line.is_empty());
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_command_C_change_to_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'w', &history);
            state.process_byte(b'C', &history);
            assert_eq!(state.line, b"hello ");
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_command_S_substitute_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'S', &history);
            assert!(state.line.is_empty());
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_command_y_yank() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"yw", &history);
            assert_eq!(state.yank_buf, b"hello ");
            assert_eq!(state.cursor, 0);
        });
    }

    #[test]
    fn vi_command_yy_yanks_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"yy", &history);
            assert_eq!(state.yank_buf, b"hello");
        });
    }

    #[test]
    fn vi_command_Y_yank_to_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello world", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'w', &history);
            state.process_byte(b'Y', &history);
            assert_eq!(state.yank_buf, b"world");
        });
    }

    #[test]
    fn vi_command_p_P_put() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'x', &history);
            assert_eq!(state.yank_buf, vec![b'c']);
            state.process_byte(b'p', &history);
            assert_eq!(state.line, b"abc");

            let mut state = ViState::new(0x7f, 0);
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'x', &history);
            state.process_byte(b'P', &history);
            assert_eq!(state.line, b"acb");
        });
    }

    #[test]
    fn vi_command_u_undo() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            state.edit_line = state.line.clone();
            state.process_byte(b'x', &history);
            assert_eq!(state.line, b"hell");
            state.process_byte(b'u', &history);
            assert_eq!(state.line, b"hello");
        });
    }

    #[test]
    fn vi_command_dot_repeat() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcde", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'x', &history);
            assert_eq!(state.line, b"bcde");
            state.process_byte(b'.', &history);
            assert_eq!(state.line, b"cde");
        });
    }

    #[test]
    fn vi_command_a_A_i_I_enter_insert() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ab", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'a', &history);
            assert!(state.insert_mode);
            assert_eq!(state.cursor, 1);

            state.process_byte(0x1b, &history);
            state.process_byte(b'A', &history);
            assert!(state.insert_mode);
            assert_eq!(state.cursor, 2);

            state.process_byte(0x1b, &history);
            state.process_byte(b'I', &history);
            assert!(state.insert_mode);
            assert_eq!(state.cursor, 0);

            state.process_byte(0x1b, &history);
            state.process_byte(b'i', &history);
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_command_history_k_j() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"cmd1"[..].into(), b"cmd2"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"current", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            assert_eq!(state.line, b"cmd2");
            assert_eq!(state.hist_index, Some(1));
            state.process_byte(b'k', &history);
            assert_eq!(state.line, b"cmd1");
            assert_eq!(state.hist_index, Some(0));
            let actions = state.process_byte(b'k', &history);
            assert!(has_bell(&actions));
            state.process_byte(b'j', &history);
            assert_eq!(state.line, b"cmd2");
            state.process_byte(b'j', &history);
            assert_eq!(state.line, b"current");
            assert_eq!(state.hist_index, None);
        });
    }

    #[test]
    fn vi_command_G_goes_to_oldest() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"oldest"[..].into(), b"newest"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'G', &history);
            assert_eq!(state.line, b"oldest");
        });
    }

    #[test]
    fn vi_command_hash_comments_out() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"echo hello", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'#', &history);
            assert!(has_return(&actions));
            let ret = get_return(&actions).unwrap().unwrap();
            assert!(ret.starts_with(b"#echo hello"));
        });
    }

    #[test]
    fn vi_command_sigint_in_command_mode() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"partial", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(0x03, &history);
            assert_eq!(get_return(&actions), Some(Some(Vec::new())));
        });
    }

    #[test]
    fn vi_command_enter_in_command_mode() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"cmd", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'\n', &history);
            assert!(has_return(&actions));
            assert_eq!(get_return(&actions), Some(Some(b"cmd\n".to_vec())));
        });
    }

    #[test]
    fn vi_command_U_undoes_all() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"baseline"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            assert_eq!(state.line, b"baseline");
            state.process_byte(b'$', &history);
            state.process_byte(b'x', &history);
            assert_eq!(state.line, b"baselin");
            state.process_byte(b'U', &history);
            assert_eq!(state.line, b"baseline");
        });
    }

    #[test]
    fn vi_command_minus_navigates_history() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"hist1"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            state.process_byte(b'-', &history);
            assert_eq!(state.line, b"hist1");
        });
    }

    #[test]
    fn vi_command_plus_navigates_forward() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"h1"[..].into(), b"h2"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            state.process_byte(b'+', &history);
            assert_eq!(state.hist_index, None);
        });
    }

    #[test]
    fn vi_command_search_backward() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"/alp\n", &history);
            assert_eq!(state.line, b"alpha");
        });
    }

    #[test]
    fn vi_command_search_forward() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            state.process_byte(b'k', &history);
            feed_bytes(&mut state, b"?beta\n", &history);
            assert_eq!(state.line, b"beta");
        });
    }

    #[test]
    fn vi_command_n_N_repeat_search() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"alpha1"[..].into(),
                b"beta"[..].into(),
                b"alpha2"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"/alpha\n", &history);
            assert_eq!(state.line, b"alpha2");
            state.process_byte(b'n', &history);
            assert_eq!(state.line, b"alpha1");
            state.process_byte(b'N', &history);
            assert_eq!(state.line, b"alpha2");
        });
    }

    #[test]
    fn vi_command_search_not_found_bells() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            let actions = feed_bytes(&mut state, b"/zzz\n", &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_command_search_backspace() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"/alphx\x7fa\n", &history);
            assert_eq!(state.line, b"alpha");
        });
    }

    #[test]
    fn vi_command_d_with_invalid_motion_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"hello", &history);
            state.process_byte(0x1b, &history);
            let actions = feed_bytes(&mut state, b"dz", &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_command_unknown_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'Z', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_command_count_prefix() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcde", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"3x", &history);
            assert_eq!(state.line, b"de");
        });
    }

    #[test]
    fn vi_command_numbered_G() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> =
                vec![b"h0"[..].into(), b"h1"[..].into(), b"h2"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"2G", &history);
            assert_eq!(state.line, b"h1");
        });
    }

    #[test]
    fn vi_command_v_returns_run_editor_action() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t("getpid", vec![], TraceResult::Int(42)),
                t(
                    "open",
                    vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Int(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = b"hello".to_vec();
                state.cursor = 4;
                state.insert_mode = false;
                let actions = state.process_byte(b'v', &history);
                assert!(
                    actions
                        .iter()
                        .any(|a| matches!(a, ViAction::RunEditor { .. }))
                );
            },
        );
    }

    #[test]
    fn vi_h_at_beginning_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'h', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_l_at_end_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'l', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_w_at_end_no_move() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let before = state.cursor;
            let _actions = state.process_byte(b'w', &history);
            assert_eq!(state.cursor, before);
        });
    }

    #[test]
    fn vi_b_at_start_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            let actions = state.process_byte(b'b', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_W_at_end_no_move() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let before = state.cursor;
            let _actions = state.process_byte(b'W', &history);
            assert_eq!(state.cursor, before);
        });
    }

    #[test]
    fn vi_B_at_start_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            let actions = state.process_byte(b'B', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_find_not_found_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            let actions = feed_bytes(&mut state, b"fz", &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_X_at_start_bells() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'X', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_j_with_no_history_bells() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![];
            let mut state = ViState::new(0x7f, 0);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'j', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_k_with_no_history_bells() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![];
            let mut state = ViState::new(0x7f, 0);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'k', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_insert_not_at_end_redraws() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ac", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'l', &history);
            state.process_byte(b'i', &history);
            let actions = state.process_byte(b'b', &history);
            assert_eq!(state.line, b"abc");
            assert!(actions.iter().any(|a| matches!(a, ViAction::Redraw)));
        });
    }

    #[test]
    fn vi_tilde_count_overflow() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"aB", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"9~", &history);
            assert_eq!(state.line, b"Ab");
        });
    }

    #[test]
    fn check_mail_noop_when_no_mail_set() {
        assert_no_syscalls(|| {
            let mut shell = super::test_shell();
            check_mail(&mut shell);
        });
    }

    #[test]
    fn check_mail_detects_new_mail() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/tmp/test_mail".into()), ArgMatcher::Any],
                    TraceResult::StatFileSize(42),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(2),
                        ArgMatcher::Bytes(b"you have mail".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(2), ArgMatcher::Bytes(b"\n".to_vec())],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = super::test_shell();
                let _ = shell.set_var(b"MAIL", b"/tmp/test_mail".to_vec());
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_with_mailpath_and_custom_message() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/tmp/box1".into()), ArgMatcher::Any],
                    TraceResult::StatFileSize(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(2), ArgMatcher::Bytes(b"New mail!".to_vec())],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(2), ArgMatcher::Bytes(b"\n".to_vec())],
                    TraceResult::Auto,
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/tmp/box2".into()), ArgMatcher::Any],
                    TraceResult::StatFileSize(0),
                ),
            ],
            || {
                let mut shell = super::test_shell();
                let _ = shell.set_var(b"MAILPATH", b"/tmp/box1%New mail!:/tmp/box2".to_vec());
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_skips_empty_path() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/tmp/box".into()), ArgMatcher::Any],
                    TraceResult::StatFileSize(0),
                ),
            ],
            || {
                let mut shell = super::test_shell();
                let _ = shell.set_var(b"MAILPATH", b":/tmp/box".to_vec());
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_respects_interval() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(1_000_000_000),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/tmp/mbox".into()), ArgMatcher::Any],
                    TraceResult::StatFileSize(0),
                ),
                t(
                    "monotonic_clock_ns",
                    vec![],
                    TraceResult::Int(2_000_000_000),
                ),
            ],
            || {
                let mut shell = super::test_shell();
                let _ = shell.set_var(b"MAIL", b"/tmp/mbox".to_vec());
                check_mail(&mut shell);
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn word_backward_skips_punctuation_class() {
        assert_no_syscalls(|| {
            assert_eq!(word_backward(b"abc...", 5), 3);
            assert_eq!(word_backward(b"   ", 2), 0);
        });
    }

    #[test]
    fn word_end_punctuation_class() {
        assert_no_syscalls(|| {
            assert_eq!(word_end(b"abc...xyz", 0), 2);
            assert_eq!(word_end(b"abc...xyz", 3), 5);
            assert_eq!(word_end(b"a  ", 0), 2);
        });
    }

    #[test]
    fn bigword_end_at_end() {
        assert_no_syscalls(|| {
            assert_eq!(bigword_end(b"abc", 2), 2);
            assert_eq!(bigword_end(b"a  ", 0), 2);
        });
    }

    #[test]
    fn vi_count_digits_continuation() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> =
                vec![b"one"[..].into(), b"two"[..].into(), b"three"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"text", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'1', &history);
            state.process_byte(b'2', &history);
            state.process_byte(b'G', &history);
            assert!(!state.line.is_empty());
        });
    }

    #[test]
    fn vi_replace_with_count() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcdef", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'3', &history);
            state.process_byte(b'r', &history);
            state.process_byte(b'z', &history);
            assert_eq!(&state.line[..3], b"zzz");
        });
    }

    #[test]
    fn vi_replace_mode_esc_adjusts_cursor() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'R', &history);
            state.process_byte(b'z', &history);
            state.process_byte(b'z', &history);
            state.process_byte(b'z', &history);
            state.process_byte(0x1b, &history);
            assert_eq!(state.line, b"abzzz");
            assert_eq!(state.cursor, 4);
        });
    }

    #[test]
    fn vi_replace_mode_past_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"ab", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'R', &history);
            state.process_byte(b'x', &history);
            state.process_byte(b'y', &history);
            state.process_byte(b'z', &history);
            assert_eq!(state.line, b"axyz");
        });
    }

    #[test]
    fn vi_count_zero_normalization() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'i', &history);
            assert!(state.insert_mode);
        });
    }

    #[test]
    fn vi_semicolon_bell_on_not_found() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcdef", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'f', &history);
            state.process_byte(b'c', &history);
            assert_eq!(state.cursor, 2);
            let actions = state.process_byte(b';', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_comma_reverses_find_direction() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcba", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b'f', &history);
            state.process_byte(b'b', &history);
            assert_eq!(state.cursor, 1);
            state.process_byte(b';', &history);
            assert_eq!(state.cursor, 3);
            state.process_byte(b',', &history);
            assert_eq!(state.cursor, 1);
        });
    }

    #[test]
    fn vi_comma_bell_when_not_found() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcdef", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'$', &history);
            state.process_byte(b'F', &history);
            state.process_byte(b'c', &history);
            assert_eq!(state.cursor, 2);
            let actions = state.process_byte(b',', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_D_on_empty_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.process_byte(0x1b, &history);
            let _actions = state.process_byte(b'D', &history);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_p_empty_yank_buf() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'p', &history);
            assert_eq!(state.line, b"abc");
            assert!(!actions.iter().any(|a| matches!(a, ViAction::Redraw)));
        });
    }

    #[test]
    fn vi_P_empty_yank_buf() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b'P', &history);
            assert_eq!(state.line, b"abc");
            assert!(!actions.iter().any(|a| matches!(a, ViAction::Redraw)));
        });
    }

    #[test]
    fn vi_U_without_history_clears_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"some text", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'U', &history);
            assert!(state.line.is_empty());
            assert_eq!(state.cursor, 0);
        });
    }

    #[test]
    fn vi_dot_with_explicit_count() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcdef", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'x', &history);
            state.process_byte(b'2', &history);
            state.process_byte(b'.', &history);
            assert_eq!(state.line.len(), 3);
        });
    }

    #[test]
    fn vi_dot_no_last_cmd() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let _actions = state.process_byte(b'.', &history);
            assert_eq!(state.line, b"abc");
        });
    }

    #[test]
    fn vi_k_with_empty_history_line() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b""[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            assert_eq!(state.cursor, 0);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_G_with_explicit_count() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"first"[..].into(),
                b"second"[..].into(),
                b"third"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'2', &history);
            state.process_byte(b'G', &history);
            assert_eq!(state.line, b"second");
        });
    }

    #[test]
    fn vi_G_default_goes_to_oldest() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"first"[..].into(),
                b"second"[..].into(),
                b"third"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'G', &history);
            assert_eq!(state.line, b"first");
            assert_eq!(state.cursor, 4);
        });
    }

    #[test]
    fn vi_G_with_empty_history_line() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b""[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'G', &history);
            assert!(state.line.is_empty());
            assert_eq!(state.cursor, 0);
        });
    }

    #[test]
    fn vi_search_forward_break_and_edit_line() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'/', &history);
            for &b in b"alpha" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert_eq!(state.line, b"alpha");
        });
    }

    #[test]
    fn vi_search_backward_not_found_bells() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"cur", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            assert_eq!(state.line, b"alpha");
            state.process_byte(b'?', &history);
            for &b in b"nothere" {
                state.process_byte(b, &history);
            }
            let actions = state.process_byte(b'\r', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_w_truly_stuck_no_movement() {
        assert_no_syscalls(|| {
            let next = word_forward(b"a", 0);
            assert_eq!(next, 1);
            let clamped = next.min(1usize.saturating_sub(1));
            assert_eq!(clamped, 0);
        });
    }

    #[test]
    fn replay_cmd_x_cursor_adjusts() {
        assert_no_syscalls(|| {
            let mut line = b"ab".to_vec();
            let mut cursor = 1usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'x', 2, None);
            assert_eq!(line, b"");
            assert_eq!(cursor, 0);
        });
    }

    #[test]
    fn replay_cmd_r_with_count() {
        assert_no_syscalls(|| {
            let mut line = b"abcdef".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 3, Some(b'z'));
            assert_eq!(&line[..3], b"zzz");
        });
    }

    #[test]
    fn replay_cmd_d_and_c_with_motion() {
        assert_no_syscalls(|| {
            let mut line = b"hello world".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
            assert_eq!(line, b"world");

            let mut line2 = b"hello world".to_vec();
            let mut cursor2 = 0usize;
            let mut yank2 = vec![];
            replay_cmd(&mut line2, &mut cursor2, &mut yank2, b'c', 1, Some(b'w'));
            assert_eq!(line2, b"world");
        });
    }

    #[test]
    fn vi_star_glob_expand() {
        assert_no_syscalls(|| {
            let dir = std::env::temp_dir().join("meiksh_vi_star_test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("aaa.txt"), b"").unwrap();
            std::fs::write(dir.join("bbb.txt"), b"").unwrap();

            let pattern = format!("{}/", dir.display());
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.line = pattern.as_bytes().to_vec();
            state.cursor = state.line.len().saturating_sub(1);
            state.insert_mode = false;
            state.process_byte(b'*', &history);
            assert!(
                state
                    .line
                    .windows(b"aaa.txt".len())
                    .any(|w| w == b"aaa.txt")
            );
            assert!(
                state
                    .line
                    .windows(b"bbb.txt".len())
                    .any(|w| w == b"bbb.txt")
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn vi_backslash_unique_completion() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        let dir = std::env::temp_dir().join("meiksh_vi_bslash_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("unique_file.txt"), b"").unwrap();

        let expected = format!("{}/unique_file.txt", dir.display());
        run_trace(
            vec![t(
                "stat",
                vec![
                    ArgMatcher::Str(expected.as_bytes().to_vec()),
                    ArgMatcher::Any,
                ],
                TraceResult::StatFile(0o644),
            )],
            || {
                let prefix = format!("{}/unique_fi", dir.display());
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = prefix.as_bytes().to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'\\', &history);
                assert!(
                    state
                        .line
                        .windows(b"unique_file.txt".len())
                        .any(|w| w == b"unique_file.txt")
                );
            },
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vi_backslash_dir_appends_slash() {
        use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
        let dir = std::env::temp_dir().join("meiksh_vi_bslash_dir_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("subdir_only")).unwrap();

        let expected = format!("{}/subdir_only/", dir.display());
        run_trace(
            vec![t(
                "stat",
                vec![
                    ArgMatcher::Str(expected.as_bytes().to_vec()),
                    ArgMatcher::Any,
                ],
                TraceResult::StatDir,
            )],
            || {
                let prefix = format!("{}/subdir_on", dir.display());
                let mut state = ViState::new(0x7f, 0);
                let history: Vec<Box<[u8]>> = vec![];
                state.line = prefix.as_bytes().to_vec();
                state.cursor = state.line.len().saturating_sub(1);
                state.insert_mode = false;
                state.process_byte(b'\\', &history);
                assert_eq!(state.line.last(), Some(&b'/'));
            },
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vi_backslash_ambiguous_bells() {
        assert_no_syscalls(|| {
            let dir = std::env::temp_dir().join("meiksh_vi_bslash_amb_test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("ab1.txt"), b"").unwrap();
            std::fs::write(dir.join("ab2.txt"), b"").unwrap();

            let prefix = format!("{}/ab", dir.display());
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.line = prefix.as_bytes().to_vec();
            state.cursor = state.line.len().saturating_sub(1);
            state.insert_mode = false;
            let actions = state.process_byte(b'\\', &history);
            assert!(has_bell(&actions));
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn glob_expand_error_returns_err() {
        assert_no_syscalls(|| {
            assert!(glob_expand(b"\0invalid").is_err());
        });
    }

    #[test]
    fn vi_r_replace_at_end_of_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'3', &history);
            state.process_byte(b'r', &history);
            state.process_byte(b'z', &history);
            assert_eq!(state.line, b"z");
        });
    }

    #[test]
    fn vi_w_empty_line_truly_stuck() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.insert_mode = false;
            let actions = state.process_byte(b'w', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_W_empty_line_truly_stuck() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.insert_mode = false;
            let actions = state.process_byte(b'W', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_comma_with_t_and_T_directions() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcba", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            state.process_byte(b't', &history);
            state.process_byte(b'b', &history);
            let saved = state.cursor;
            state.process_byte(b',', &history);
            let _ = saved;

            let mut state2 = ViState::new(0x7f, 0);
            feed_bytes(&mut state2, b"abcba", &history);
            state2.process_byte(0x1b, &history);
            state2.process_byte(b'$', &history);
            state2.process_byte(b'T', &history);
            state2.process_byte(b'b', &history);
            state2.process_byte(b',', &history);
        });
    }

    #[test]
    fn vi_tilde_at_end_break() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'~', &history);
            assert_eq!(state.line, b"A");
        });
    }

    #[test]
    fn vi_D_on_empty() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.insert_mode = false;
            state.process_byte(b'D', &history);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_p_P_empty_yank_no_redraw() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            let a1 = state.process_byte(b'p', &history);
            assert!(!a1.iter().any(|a| matches!(a, ViAction::Redraw)));
            let a2 = state.process_byte(b'P', &history);
            assert!(!a2.iter().any(|a| matches!(a, ViAction::Redraw)));
        });
    }

    #[test]
    fn vi_star_with_explicit_glob_chars() {
        assert_no_syscalls(|| {
            let dir = std::env::temp_dir().join("meiksh_vi_star_glob_test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("file1.txt"), b"").unwrap();

            let pattern = format!("{}/*.txt", dir.display());
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.line = pattern.as_bytes().to_vec();
            state.cursor = state.line.len().saturating_sub(1);
            state.insert_mode = false;
            state.process_byte(b'*', &history);
            assert!(
                state
                    .line
                    .windows(b"file1.txt".len())
                    .any(|w| w == b"file1.txt")
            );
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn vi_search_forward_idx_break() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'/', &history);
            for &b in b"aaa" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert_eq!(state.line, b"aaa");
        });
    }

    #[test]
    fn vi_search_backward_not_found() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            state.process_byte(b'?', &history);
            for &b in b"zzz" {
                state.process_byte(b, &history);
            }
            let actions = state.process_byte(b'\r', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn replay_cmd_r_past_end() {
        assert_no_syscalls(|| {
            let mut line = b"a".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 5, Some(b'z'));
            assert_eq!(line, b"z");
        });
    }

    #[test]
    fn replay_cmd_d_with_dd() {
        assert_no_syscalls(|| {
            let mut line = b"hello".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'd'));
            assert!(line.is_empty());
            assert_eq!(cursor, 0);
        });
    }

    #[test]
    fn replay_cmd_c_with_cc() {
        assert_no_syscalls(|| {
            let mut line = b"hello".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'c'));
            assert!(line.is_empty());
            assert_eq!(cursor, 0);
        });
    }

    #[test]
    fn vi_semicolon_with_last_find_on_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"a", &history);
            state.process_byte(0x1b, &history);
            state.last_find = Some((b'f', b'z'));
            let actions = state.process_byte(b';', &history);
            assert!(has_bell(&actions));
            state.last_find = Some((b'f', b'z'));
            let _actions = state.process_byte(b';', &history);
        });
    }

    #[test]
    fn vi_semicolon_no_last_find() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b';', &history);
            assert!(!has_bell(&actions));
        });
    }

    #[test]
    fn vi_comma_reverse_find() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abcabc", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'0', &history);
            feed_bytes(&mut state, b"fb", &history);
            assert_eq!(state.cursor, 1);
            state.process_byte(b';', &history);
            assert_eq!(state.cursor, 4);
            state.process_byte(b',', &history);
            assert_eq!(state.cursor, 1);
        });
    }

    #[test]
    fn vi_comma_no_last_find() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            let actions = state.process_byte(b',', &history);
            assert!(!has_bell(&actions));
        });
    }

    #[test]
    fn vi_comma_with_invalid_last_find_cmd() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.last_find = Some((b'z', b'a'));
            let actions = state.process_byte(b',', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_replace_char_past_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"rZ", &history);
            assert_eq!(state.line, b"");
        });
    }

    #[test]
    fn vi_tilde_on_empty_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.process_byte(0x1b, &history);
            state.process_byte(b'~', &history);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_x_past_end() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            state.process_byte(0x1b, &history);
            state.process_byte(b'x', &history);
            assert!(state.line.is_empty());
        });
    }

    #[test]
    fn vi_G_with_count() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"first"[..].into(),
                b"second"[..].into(),
                b"third"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"current", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"2G", &history);
            assert_eq!(state.line, b"second");
            assert!(state.hist_index.is_some());
        });
    }

    #[test]
    #[allow(non_snake_case)]
    fn vi_G_with_count_no_history() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"text", &history);
            state.process_byte(0x1b, &history);
            feed_bytes(&mut state, b"2G", &history);
            assert_eq!(state.line, b"text");
            assert!(state.hist_index.is_none());
        });
    }

    #[test]
    fn vi_G_without_count_no_history() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"text", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'G', &history);
            assert_eq!(state.line, b"text");
        });
    }

    #[test]
    fn vi_G_without_count_with_history() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"oldest"[..].into(), b"newest"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"text", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'G', &history);
            assert_eq!(state.line, b"oldest");
        });
    }

    #[test]
    fn vi_search_forward_finds_match() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"echo hello"[..].into(),
                b"ls -la"[..].into(),
                b"echo world"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'/', &history);
            for &b in b"echo" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert!(state.hist_index.is_some());
            let idx = state.hist_index.unwrap();
            assert!(history[idx].windows(4).any(|w| w == b"echo"));
        });
    }

    #[test]
    fn vi_search_forward_not_found() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into(), b"bbb"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'/', &history);
            for &b in b"zzz" {
                state.process_byte(b, &history);
            }
            let actions = state.process_byte(b'\r', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_search_forward_idx_wraps() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            state.process_byte(b'/', &history);
            for &b in b"alpha" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert_eq!(state.hist_index, Some(0));
        });
    }

    #[test]
    fn vi_search_backward_finds_match() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"echo hello"[..].into(),
                b"ls -la"[..].into(),
                b"echo world"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'?', &history);
            for &b in b"echo" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert!(state.hist_index.is_some());
            let idx = state.hist_index.unwrap();
            assert!(history[idx].windows(4).any(|w| w == b"echo"));
        });
    }

    #[test]
    fn vi_search_default_direction_noop() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"aaa"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            let mut actions = Vec::new();
            state.do_search(b'x', &history, &mut actions);
            assert!(actions.is_empty());
        });
    }

    #[test]
    fn replay_cmd_r_no_arg() {
        assert_no_syscalls(|| {
            let mut line = b"abc".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 1, None);
            assert_eq!(line, b"abc");
        });
    }

    #[test]
    fn replay_cmd_r_cursor_past_end() {
        assert_no_syscalls(|| {
            let mut line = vec![];
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'r', 1, Some(b'z'));
            assert!(line.is_empty());
        });
    }

    #[test]
    fn replay_cmd_d_no_arg() {
        assert_no_syscalls(|| {
            let mut line = b"hello".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, None);
            assert_eq!(line, b"hello");
        });
    }

    #[test]
    fn replay_cmd_c_no_arg() {
        assert_no_syscalls(|| {
            let mut line = b"hello".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, None);
            assert_eq!(line, b"hello");
        });
    }

    #[test]
    fn replay_cmd_d_with_motion() {
        assert_no_syscalls(|| {
            let mut line = b"hello world".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'd', 1, Some(b'w'));
            assert_eq!(line, b"world");
            assert_eq!(yank, b"hello ");
        });
    }

    #[test]
    fn replay_cmd_c_with_motion() {
        assert_no_syscalls(|| {
            let mut line = b"hello world".to_vec();
            let mut cursor = 0usize;
            let mut yank = vec![];
            replay_cmd(&mut line, &mut cursor, &mut yank, b'c', 1, Some(b'w'));
            assert_eq!(line, b"world");
            assert_eq!(yank, b"hello ");
        });
    }

    #[test]
    fn glob_expand_null_byte_returns_err() {
        assert_no_syscalls(|| {
            let result = glob_expand(b"foo\0bar");
            assert!(result.is_err());
        });
    }

    #[test]
    fn vi_process_motion_unknown_op_noop() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"abc", &history);
            state.process_byte(0x1b, &history);
            state.pending = PendingInput::Motion { op: b'z', count: 1 };
            let actions = state.process_byte(b'w', &history);
            assert_eq!(state.line, b"abc");
            assert!(actions.is_empty() || !has_bell(&actions));
        });
    }

    #[test]
    fn glob_expand_nomatch_returns_err() {
        assert_no_syscalls(|| {
            let result = glob_expand(b"/nonexistent_dir_xyz_42/*.qqq");
            assert!(result.is_err());
        });
    }

    #[test]
    fn vi_star_glob_nomatch_leaves_line() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            let word = b"/no_such_dir_xyzzy/";
            state.line = word.to_vec();
            state.cursor = state.line.len().saturating_sub(1);
            state.insert_mode = false;
            state.process_byte(b'*', &history);
            assert_eq!(state.line, word);
        });
    }

    #[test]
    fn vi_search_forward_from_oldest_wraps() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![b"alpha"[..].into(), b"beta"[..].into()];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"x", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'k', &history);
            state.process_byte(b'k', &history);
            assert_eq!(state.hist_index, Some(0));
            state.process_byte(b'/', &history);
            for &b in b"zzz" {
                state.process_byte(b, &history);
            }
            let actions = state.process_byte(b'\r', &history);
            assert!(has_bell(&actions));
        });
    }

    #[test]
    fn vi_search_forward_edit_line_save() {
        assert_no_syscalls(|| {
            let history: Vec<Box<[u8]>> = vec![
                b"found"[..].into(),
                b"skip"[..].into(),
                b"also_skip"[..].into(),
            ];
            let mut state = ViState::new(0x7f, history.len());
            feed_bytes(&mut state, b"original", &history);
            state.process_byte(0x1b, &history);
            state.process_byte(b'/', &history);
            for &b in b"found" {
                state.process_byte(b, &history);
            }
            state.process_byte(b'\r', &history);
            assert_eq!(state.hist_index, Some(0));
            assert_eq!(state.line, b"found");
        });
    }

    #[test]
    fn vi_backslash_glob_nomatch_no_change() {
        assert_no_syscalls(|| {
            let mut state = ViState::new(0x7f, 0);
            let history: Vec<Box<[u8]>> = vec![];
            let word = b"/no_such_dir_xyzzy/nomatch";
            state.line = word.to_vec();
            state.cursor = state.line.len().saturating_sub(1);
            state.insert_mode = false;
            state.process_byte(b'\\', &history);
            assert_eq!(state.line, word);
        });
    }
}

#[test]
fn vi_read_line_returns_line_on_enter() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'h']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'h'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\n']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"h\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_eof_returns_none() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, None);
        },
    );
}

#[test]
fn vi_read_line_bell_and_redraw() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'a']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'Q']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"a\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_redraw_on_motion() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'a']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'b']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'b'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'h']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[1D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"ab\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_eof_with_nonempty_continues() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'x']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'x'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![]),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\n']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"x\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_erase_char_fallback() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "tcgetattr",
                vec![ArgMatcher::Fd(0)],
                TraceResult::Err(libc::EINVAL),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\n']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_tcgetattr_error_falls_back() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t(
                "tcgetattr",
                vec![ArgMatcher::Fd(0)],
                TraceResult::Err(libc::ENOTTY),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![]),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, None);
        },
    );
}

#[test]
fn vi_read_line_redraw_covers_full_redraw() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'a']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'b']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'b'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'b']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"ab\x1b[2D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"ab\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_read_error_propagates() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Err(libc::EIO),
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"");
            assert!(result.is_err());
        },
    );
}

#[test]
fn vi_read_line_count_digit_triggers_readbyte() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'a']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'2']),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'l']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"a\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_insert_mode_change() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'i']),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_find_triggers_need_find_target() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'a']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'f']),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'z']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"a\x1b[1D".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"a\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_v_command_empty_file_redraws() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'v']),
            ),
            t("getpid", vec![], TraceResult::Int(42)),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(10),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())],
                TraceResult::Auto,
            ),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let _ = shell.set_var(b"EDITOR", b":".to_vec());
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_v_command_whitespace_only_redraws() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'v']),
            ),
            t("getpid", vec![], TraceResult::Int(42)),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(10),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())],
                TraceResult::Auto,
            ),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(11),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(11), ArgMatcher::Any],
                TraceResult::Bytes(b"\n".to_vec()),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(11), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'\r']),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let _ = shell.set_var(b"EDITOR", b":".to_vec());
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"\n".to_vec()));
        },
    );
}

#[test]
fn vi_read_line_v_command_runs_editor() {
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    run_trace(
        vec![
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![0x1b]),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(0), ArgMatcher::Any],
                TraceResult::Bytes(vec![b'v']),
            ),
            t("getpid", vec![], TraceResult::Int(42)),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(10),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())],
                TraceResult::Auto,
            ),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
            t(
                "open",
                vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Int(11),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(11), ArgMatcher::Any],
                TraceResult::Bytes(b"edited\n".to_vec()),
            ),
            t(
                "read",
                vec![ArgMatcher::Fd(11), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())],
                TraceResult::Auto,
            ),
            t(
                "tcsetattr",
                vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)],
                TraceResult::Int(0),
            ),
        ],
        || {
            let mut shell = test_shell();
            let _ = shell.set_var(b"EDITOR", b":".to_vec());
            let result = vi::read_line(&mut shell, b"").unwrap();
            assert_eq!(result, Some(b"edited\n".to_vec()));
        },
    );
}
