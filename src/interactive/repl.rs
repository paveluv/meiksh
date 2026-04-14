use super::{
    append_history, check_mail, command_is_fc, expand_prompt, read_line, vi, write_prompt,
};
use crate::bstr::{BStrExt, ByteWriter};
use crate::shell::{Shell, ShellError};
use crate::sys;

pub(super) fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
    let mut accumulated = Vec::<u8>::new();
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
                        ByteWriter::new()
                            .byte(b'[')
                            .usize_val(id)
                            .bytes(b"] Done\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish()
                    } else {
                        ByteWriter::new()
                            .byte(b'[')
                            .usize_val(id)
                            .bytes(b"] Done(")
                            .i32_val(status)
                            .bytes(b")\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish()
                    }
                }
                ReapedJobState::Signaled(sig, cmd) => ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Terminated (")
                    .bytes(sys::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
                ReapedJobState::Stopped(sig, cmd) => ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Stopped (")
                    .bytes(sys::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
            };
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        }

        shell.run_pending_traps()?;
        if !shell.running {
            break;
        }

        check_mail(shell);

        let prompt_str = if accumulated.is_empty() {
            expand_prompt(shell, b"PS1", b"$ ")
        } else {
            expand_prompt(shell, b"PS2", b"> ")
        };
        write_prompt(&prompt_str).map_err(|e| shell.diagnostic_syserr(1, &e))?;

        let line = match if shell.options.vi_mode {
            vi::read_line(shell, &prompt_str)
        } else {
            read_line()
        }
        .map_err(|e| shell.diagnostic_syserr(1, &e))?
        {
            Some(line) => line,
            None => {
                if !accumulated.is_empty() {
                    let _ = sys::write_all_fd(
                        sys::STDERR_FILENO,
                        b"meiksh: unexpected EOF while looking for matching token\n",
                    );
                    accumulated.clear();
                }
                break;
            }
        };
        if accumulated.is_empty() && line.trim_ascii_ws().is_empty() {
            continue;
        }
        accumulated.extend_from_slice(&line);

        match crate::syntax::parse_with_aliases(&accumulated, &shell.aliases) {
            Ok(_) => {}
            Err(ref e) if shell.input_is_incomplete(e) => {
                continue;
            }
            Err(_) => {}
        }

        let source = std::mem::take(&mut accumulated);
        let trimmed_end = {
            let mut end = source.len();
            while end > 0
                && (source[end - 1] == b' '
                    || source[end - 1] == b'\t'
                    || source[end - 1] == b'\n'
                    || source[end - 1] == b'\r')
            {
                end -= 1;
            }
            &source[..end]
        };
        append_history(shell, trimmed_end)?;
        let trimmed = source.trim_ascii_ws();
        if !command_is_fc(trimmed) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::*;
    use crate::shell::{TrapAction, TrapCondition};

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
}
