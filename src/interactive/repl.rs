use super::history::append_history;
use super::mail::{check_mail, command_is_fc};
use super::prompt::{expand_prompt, read_line, write_prompt};
use super::vi_editing;
use crate::bstr::{BStrExt, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
    let mut accumulated = Vec::<u8>::new();
    let mut sigchld_installed = false;
    loop {
        if shell.options.notify && !sigchld_installed {
            let _ = sys::process::install_shell_signal_handler(sys::constants::SIGCHLD);
            sigchld_installed = true;
        } else if !shell.options.notify && sigchld_installed {
            let _ = sys::process::default_signal_action(sys::constants::SIGCHLD);
            sigchld_installed = false;
        }

        for (id, state) in shell.reap_jobs() {
            use crate::shell::jobs::ReapedJobState;
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
                    .bytes(sys::process::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
                ReapedJobState::Stopped(sig, cmd) => ByteWriter::new()
                    .byte(b'[')
                    .usize_val(id)
                    .bytes(b"] Stopped (")
                    .bytes(sys::process::signal_name(sig))
                    .bytes(b")\t")
                    .bytes(&cmd)
                    .byte(b'\n')
                    .finish(),
            };
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
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
            vi_editing::read_line(shell, &prompt_str)
        } else {
            read_line()
        }
        .map_err(|e| shell.diagnostic_syserr(1, &e))?
        {
            Some(line) => line,
            None => {
                if !accumulated.is_empty() {
                    let _ = sys::fd_io::write_all_fd(
                        sys::constants::STDERR_FILENO,
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

        match crate::syntax::parse_with_aliases(&accumulated, &shell.aliases()) {
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
    use crate::interactive::test_support::{read_line_trace, test_shell};
    use crate::shell::traps::{TrapAction, TrapCondition};
    use crate::sys;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn run_loop_exits_on_immediate_eof() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
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
            trace_entries![
                waitpid(4001, _) -> status(0),
                waitpid(4002, _) -> status(0),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"[1] Done\tdone\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"test$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"\n"),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"test$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"e"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"x"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"i"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"t"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b" "),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"5"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"\n"),
                open("/tmp/history.txt", _, _) -> fd(10),
                write(fd(10), bytes(b"exit 5\n")) -> auto,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/history.txt".to_vec());
                shell.env_mut().insert(b"PS1".to_vec(), b"test$ ".to_vec());

                let handle = sys::types::ChildHandle {
                    pid: 4001,
                    stdout_fd: None,
                };
                shell.register_background_job(b"done"[..].into(), None, vec![handle]);
                shell.reap_jobs();

                let handle = sys::types::ChildHandle {
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                ..read_line_trace(b"echo 'unterminated\n"),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"> ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: unexpected EOF while looking for matching token\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/bad-history.txt".to_vec());
                let status = run_loop(&mut shell).expect("parse handled");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn run_loop_handles_sigint_by_redisplaying_prompt() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> err(sys::constants::EINTR),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
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
            trace_entries![
                waitpid(4010, _) -> stopped_sig(sys::constants::SIGTSTP),
                waitpid(4010, _) -> pid(0),
                waitpid(4011, _) -> pid(0),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"[1] Stopped (SIGTSTP)\tvim\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b""),
            ],
            || {
                let mut shell = test_shell();
                let handle_stopped = sys::types::ChildHandle {
                    pid: 4010,
                    stdout_fd: None,
                };
                shell.register_background_job(b"vim"[..].into(), None, vec![handle_stopped]);
                let handle_running = sys::types::ChildHandle {
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
            trace_entries![
                signal(int(sys::constants::SIGINT), _) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> interrupt(sys::constants::SIGINT),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::command(b"TRAPPED=yes")),
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
            trace_entries![
                signal(int(sys::constants::SIGINT), _) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> interrupt(sys::constants::SIGINT),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::command(b"exit 42")),
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> err(sys::constants::EINTR),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> err(sys::constants::EIO),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: Input/output error\n")) -> auto,
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
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                ..read_line_trace(b"gibberish\n"),
                open(str("/tmp/hist"), _, _) -> fd(10),
                write(fd(10), bytes(b"gibberish\n")) -> auto,
                close(fd(10)) -> 0,
                stat(str("/usr/bin/gibberish"), _) -> err(sys::constants::ENOENT),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"gibberish: not found\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
                let status = run_loop(&mut shell).expect("command not found handled");
                assert_eq!(
                    status, 127,
                    "exit status should be 127 for command not found"
                );
            },
        );
    }

    #[test]
    fn run_loop_syntax_error_prints_error_and_continues() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> int(2),
                ..read_line_trace(b"$(\n"),
                open(str("/tmp/hist"), _, _) -> fd(10),
                write(fd(10), bytes(b"$(\n")) -> int(3),
                close(fd(10)) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: line 2: unterminated command substitution\n")) -> int(50),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> int(2),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b""),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
                let _ = run_loop(&mut shell);
            },
        );
    }

    #[test]
    fn run_loop_sigchld_install_and_remove() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGCHLD), _) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                ..read_line_trace(b"set +b\n"),
                open(str("/tmp/hist"), _, _) -> fd(10),
                write(fd(10), bytes(b"set +b\n")) -> auto,
                close(fd(10)) -> 0,
                signal(int(sys::constants::SIGCHLD), _) -> 0,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b""),
            ],
            || {
                let mut shell = test_shell();
                shell.options.notify = true;
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/hist".to_vec());
                let _ = run_loop(&mut shell);
            },
        );
    }

    #[test]
    fn run_loop_signaled_and_done_nonzero_notifications() {
        run_trace(
            trace_entries![
                waitpid(6001, _) -> signaled_sig(15),
                waitpid(6002, _) -> status(7),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"[1] Terminated (SIGTERM)\tkilled\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"[2] Done(7)\tfailed\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b""),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    b"killed"[..].into(),
                    None,
                    vec![sys::types::ChildHandle {
                        pid: 6001,
                        stdout_fd: None,
                    }],
                );
                shell.register_background_job(
                    b"failed"[..].into(),
                    None,
                    vec![sys::types::ChildHandle {
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"$ ")) -> auto,
                tcgetattr(fd(sys::constants::STDIN_FILENO)) -> 0,
                tcsetattr(fd(sys::constants::STDIN_FILENO), int(1)) -> 0,
                tcgetattr(fd(sys::constants::STDIN_FILENO)) -> 0,
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b""),
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"\r\n")) -> auto,
                tcsetattr(fd(sys::constants::STDIN_FILENO), int(1)) -> 0,
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
