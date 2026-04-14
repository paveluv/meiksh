use crate::arena::ByteArena;
use crate::bstr::{self, BStrExt, ByteWriter};
use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

fn remove_file_bytes(path: &[u8]) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let _ = std::fs::remove_file(OsStr::from_bytes(path));
}

pub fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| shell.diagnostic_syserr(1, &e))?;
    run_loop(shell)
}

fn run_loop(shell: &mut Shell) -> Result<i32, ShellError> {
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

fn write_prompt(prompt_str: &[u8]) -> sys::SysResult<()> {
    loop {
        match sys::write_all_fd(sys::STDERR_FILENO, prompt_str) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
}

fn read_line() -> sys::SysResult<Option<Vec<u8>>> {
    let mut line = Vec::<u8>::new();
    let mut byte = [0u8; 1];
    loop {
        match sys::read_fd(sys::STDIN_FILENO, &mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(line) }),
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    return Ok(Some(line));
                }
            }
            Err(e) if e.is_eintr() => {
                let _ = sys::write_all_fd(sys::STDERR_FILENO, b"\n");
                return Ok(Some(Vec::new()));
            }
            Err(e) => return Err(e),
        }
    }
}

fn expand_prompt(shell: &mut Shell, var: &[u8], default: &[u8]) -> Vec<u8> {
    let raw = shell.get_var(var).unwrap_or(default).to_vec();
    let histnum = shell.history_number();
    let arena = ByteArena::new();
    let expanded = expand::expand_parameter_text(shell, &raw, &arena).unwrap_or(&raw);
    expand_prompt_exclamation(expanded, histnum)
}

fn expand_prompt_exclamation(s: &[u8], histnum: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'!' {
            i += 1;
            if i < s.len() && s[i] == b'!' {
                result.push(b'!');
                i += 1;
            } else if i < s.len() {
                bstr::push_u64(&mut result, histnum as u64);
                result.push(s[i]);
                i += 1;
            } else {
                bstr::push_u64(&mut result, histnum as u64);
            }
        } else {
            result.push(s[i]);
            i += 1;
        }
    }
    result
}

pub(crate) fn check_mail(shell: &mut Shell) {
    let has_mail = shell.get_var(b"MAIL").is_some();
    let has_mailpath = shell.get_var(b"MAILPATH").is_some();
    if !has_mail && !has_mailpath {
        return;
    }

    let check_interval: u64 = shell
        .get_var(b"MAILCHECK")
        .and_then(|v| bstr::parse_i64(v).map(|n| n as u64))
        .unwrap_or(600);
    let now = sys::monotonic_clock_ns() / 1_000_000_000;
    if shell.mail_last_check != 0 && now.saturating_sub(shell.mail_last_check) < check_interval {
        return;
    }
    shell.mail_last_check = now;

    let entries: Vec<(Vec<u8>, Option<Vec<u8>>)> =
        if let Some(mp) = shell.get_var(b"MAILPATH").map(|s| s.to_vec()) {
            let mut result = Vec::new();
            for entry in mp.split(|&b| b == b':') {
                match entry.iter().position(|&b| b == b'%') {
                    Some(pos) => {
                        result.push((entry[..pos].to_vec(), Some(entry[pos + 1..].to_vec())));
                    }
                    None => {
                        result.push((entry.to_vec(), None));
                    }
                }
            }
            result
        } else {
            let m = shell.get_var(b"MAIL").unwrap().to_vec();
            vec![(m, None)]
        };

    for (path, custom_msg) in entries {
        if path.is_empty() {
            continue;
        }
        let size = sys::stat_path(&path).map(|st| st.size).unwrap_or(0);
        let prev = shell.mail_sizes.get(path.as_slice()).copied().unwrap_or(0);
        if size > prev {
            let msg = custom_msg.unwrap_or_else(|| b"you have mail".to_vec());
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            let _ = sys::write_all_fd(sys::STDERR_FILENO, b"\n");
        }
        shell.mail_sizes.insert(path.into(), size);
    }
}

pub(crate) fn command_is_fc(line: &[u8]) -> bool {
    let mut rest = line;
    loop {
        while !rest.is_empty() && rest[0].is_ascii_whitespace() {
            rest = &rest[1..];
        }
        if rest.is_empty() {
            return false;
        }
        if let Some(eq_pos) = rest.iter().position(|&b| b == b'=') {
            let before_eq = &rest[..eq_pos];
            if !before_eq.is_empty()
                && !before_eq.iter().any(|b| b.is_ascii_whitespace())
                && before_eq
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                let after_eq = &rest[eq_pos + 1..];
                let skip = if !after_eq.is_empty() && after_eq[0] == b'\'' {
                    after_eq[1..].iter().position(|&b| b == b'\'').map(|i| i + 2)
                } else if !after_eq.is_empty() && after_eq[0] == b'"' {
                    after_eq[1..].iter().position(|&b| b == b'"').map(|i| i + 2)
                } else {
                    after_eq.iter().position(|b| b.is_ascii_whitespace())
                };
                match skip {
                    Some(n) => {
                        rest = &after_eq[n..];
                        continue;
                    }
                    None => return false,
                }
            }
        }
        return rest == b"fc"
            || (rest.len() > 3 && &rest[..3] == b"fc " )
            || (rest.len() > 3 && &rest[..3] == b"fc\t");
    }
}

pub fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_value = shell.get_var(b"ENV").map(|s| s.to_vec());
    let arena = ByteArena::new();
    let env_file = env_value
        .map(|value| expand::expand_parameter_text(shell, &value, &arena).map(|s| s.to_vec()))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?;
    if let Some(path) = env_file {
        let is_absolute = !path.is_empty() && path[0] == b'/';
        if is_absolute && sys::file_exists(&path) {
            let _ = shell.source_path(&path)?;
        }
    }
    Ok(())
}

fn append_history(shell: &Shell, line: &[u8]) -> Result<(), ShellError> {
    let history = history_path(shell);
    let fd = match sys::open_file(
        &history,
        sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND,
        0o644,
    ) {
        Ok(fd) => fd,
        Err(_) => return Ok(()),
    };
    let mut entry = line.to_vec();
    if entry.is_empty() || entry[entry.len() - 1] != b'\n' {
        entry.push(b'\n');
    }
    let _ = sys::write_all_fd(fd, &entry);
    sys::close_fd(fd).map_err(|e| shell.diagnostic_syserr(1, &e))?;
    Ok(())
}

fn history_path(shell: &Shell) -> Vec<u8> {
    shell
        .get_var(b"HISTFILE")
        .map(|s| s.to_vec())
        .or_else(|| {
            shell.get_var(b"HOME").map(|home| {
                let mut path = home.to_vec();
                path.extend_from_slice(b"/.sh_history");
                path
            })
        })
        .unwrap_or_else(|| b".sh_history".to_vec())
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
                assert_eq!(
                    shell.get_var(b"FROM_EXPANDED_ENV"),
                    Some(b"1".as_ref())
                );
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
                shell.register_background_job(
                    b"sleep 999"[..].into(),
                    None,
                    vec![handle_running],
                );
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
        let mut trace = vec![
            t(
                "write",
                vec![
                    ArgMatcher::Fd(sys::STDERR_FILENO),
                    ArgMatcher::Bytes(b"$ ".to_vec()),
                ],
                TraceResult::Auto,
            ),
        ];
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
                assert_eq!(
                    get_return(&actions),
                    Some(Some(b"abc\n".to_vec()))
                );
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
                let history: Vec<Box<[u8]>> =
                    vec![b"cmd1"[..].into(), b"cmd2"[..].into()];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"oldest"[..].into(), b"newest"[..].into()];
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
                assert_eq!(
                    get_return(&actions),
                    Some(Some(b"cmd\n".to_vec()))
                );
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
                let history: Vec<Box<[u8]>> =
                    vec![b"h1"[..].into(), b"h2"[..].into()];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"alpha"[..].into(), b"beta"[..].into()];
                let mut state = ViState::new(0x7f, history.len());
                state.process_byte(0x1b, &history);
                feed_bytes(&mut state, b"/alp\n", &history);
                assert_eq!(state.line, b"alpha");
            });
        }

        #[test]
        fn vi_command_search_forward() {
            assert_no_syscalls(|| {
                let history: Vec<Box<[u8]>> =
                    vec![b"alpha"[..].into(), b"beta"[..].into()];
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
                    let _ = shell.set_var(
                        b"MAILPATH",
                        b"/tmp/box1%New mail!:/tmp/box2".to_vec(),
                    );
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
                let history: Vec<Box<[u8]>> = vec![
                    b"one"[..].into(),
                    b"two"[..].into(),
                    b"three"[..].into(),
                ];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"alpha"[..].into(), b"beta"[..].into()];
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
                assert!(state.line.windows(b"aaa.txt".len()).any(|w| w == b"aaa.txt"));
                assert!(state.line.windows(b"bbb.txt".len()).any(|w| w == b"bbb.txt"));
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
                    vec![ArgMatcher::Str(expected.as_bytes().to_vec()), ArgMatcher::Any],
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
                    assert!(state.line.windows(b"unique_file.txt".len()).any(|w| w == b"unique_file.txt"));
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
                    vec![ArgMatcher::Str(expected.as_bytes().to_vec()), ArgMatcher::Any],
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
                assert!(state.line.windows(b"file1.txt".len()).any(|w| w == b"file1.txt"));
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
                let history: Vec<Box<[u8]>> =
                    vec![b"oldest"[..].into(), b"newest"[..].into()];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"aaa"[..].into(), b"bbb"[..].into()];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"alpha"[..].into(), b"beta"[..].into()];
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
                let history: Vec<Box<[u8]>> =
                    vec![b"alpha"[..].into(), b"beta"[..].into()];
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'a'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'Q'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'a'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'b'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'b'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'h'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[1D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'x'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'x'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\n'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Err(libc::EINVAL)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\n'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Err(libc::ENOTTY)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![])),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'a'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'b'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'b'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'b'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())], TraceResult::Auto),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"ab\x1b[2D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Err(libc::EIO)),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'a'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'2'])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'l'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'i'])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'a'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(vec![b'a'])], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x1b[D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'f'])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'z'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\x07".to_vec())], TraceResult::Auto),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())], TraceResult::Auto),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"a\x1b[1D".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'v'])),
                t("getpid", vec![], TraceResult::Int(42)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Int(10)),
                t("write", vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())], TraceResult::Auto),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Err(libc::ENOENT)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'v'])),
                t("getpid", vec![], TraceResult::Int(42)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Int(10)),
                t("write", vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())], TraceResult::Auto),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Int(11)),
                t("read", vec![ArgMatcher::Fd(11), ArgMatcher::Any], TraceResult::Bytes(b"\n".to_vec())),
                t("read", vec![ArgMatcher::Fd(11), ArgMatcher::Any], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\x1b[K".to_vec())], TraceResult::Auto),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'\r'])),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
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
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("tcgetattr", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![0x1b])),
                t("read", vec![ArgMatcher::Fd(0), ArgMatcher::Any], TraceResult::Bytes(vec![b'v'])),
                t("getpid", vec![], TraceResult::Int(42)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Int(10)),
                t("write", vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"\n".to_vec())], TraceResult::Auto),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
                t("open", vec![ArgMatcher::Any, ArgMatcher::Any, ArgMatcher::Any], TraceResult::Int(11)),
                t("read", vec![ArgMatcher::Fd(11), ArgMatcher::Any], TraceResult::Bytes(b"edited\n".to_vec())),
                t("read", vec![ArgMatcher::Fd(11), ArgMatcher::Any], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
                t("write", vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"\r\n".to_vec())], TraceResult::Auto),
                t("tcsetattr", vec![ArgMatcher::Fd(0), ArgMatcher::Int(1)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"EDITOR", b":".to_vec());
                let result = vi::read_line(&mut shell, b"").unwrap();
                assert_eq!(result, Some(b"edited\n".to_vec()));
            },
        );
    }
}


pub(crate) mod vi {
    use crate::bstr::{self, ByteWriter};
    use crate::shell::Shell;
    use crate::sys;

    struct RawMode {
        saved: libc::termios,
    }

    impl RawMode {
        fn enter() -> sys::SysResult<Self> {
            let saved = sys::get_terminal_attrs(sys::STDIN_FILENO)?;
            let mut raw = saved;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            sys::set_terminal_attrs(sys::STDIN_FILENO, &raw)?;
            Ok(Self { saved })
        }
    }

    impl Drop for RawMode {
        fn drop(&mut self) {
            let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, &self.saved);
        }
    }

    fn read_byte() -> sys::SysResult<Option<u8>> {
        let mut buf = [0u8; 1];
        match sys::read_fd(sys::STDIN_FILENO, &mut buf) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(buf[0])),
            Err(e) => Err(e),
        }
    }

    fn write_bytes(data: &[u8]) {
        let _ = sys::write_all_fd(sys::STDOUT_FILENO, data);
    }

    fn bell() {
        write_bytes(b"\x07");
    }

    fn redraw(line: &[u8], cursor: usize, prompt: &[u8]) {
        write_bytes(b"\r\x1b[K");
        let _ = sys::write_all_fd(sys::STDERR_FILENO, prompt);
        let mut buf = Vec::with_capacity(line.len() + 20);
        buf.extend_from_slice(line);
        let cursor_back = line.len().saturating_sub(cursor);
        if cursor_back > 0 {
            buf.extend_from_slice(b"\x1b[");
            bstr::push_u64(&mut buf, cursor_back as u64);
            buf.push(b'D');
        }
        write_bytes(&buf);
    }

    pub(crate) fn is_word_char(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'_'
    }

    pub(crate) fn word_forward(line: &[u8], pos: usize) -> usize {
        let mut p = pos;
        let len = line.len();
        if p >= len {
            return p;
        }
        if is_word_char(line[p]) {
            while p < len && is_word_char(line[p]) {
                p += 1;
            }
        } else if !line[p].is_ascii_whitespace() {
            while p < len && !is_word_char(line[p]) && !line[p].is_ascii_whitespace() {
                p += 1;
            }
        }
        while p < len && line[p].is_ascii_whitespace() {
            p += 1;
        }
        p
    }

    pub(crate) fn word_backward(line: &[u8], pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let mut p = pos;
        while p > 0 && line[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
        if p == 0 {
            return 0;
        }
        if is_word_char(line[p - 1]) {
            while p > 0 && is_word_char(line[p - 1]) {
                p -= 1;
            }
        } else {
            while p > 0 && !is_word_char(line[p - 1]) && !line[p - 1].is_ascii_whitespace() {
                p -= 1;
            }
        }
        p
    }

    pub(crate) fn bigword_forward(line: &[u8], pos: usize) -> usize {
        let mut p = pos;
        let len = line.len();
        while p < len && !line[p].is_ascii_whitespace() {
            p += 1;
        }
        while p < len && line[p].is_ascii_whitespace() {
            p += 1;
        }
        p
    }

    pub(crate) fn bigword_backward(line: &[u8], pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let mut p = pos;
        while p > 0 && line[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
        while p > 0 && !line[p - 1].is_ascii_whitespace() {
            p -= 1;
        }
        p
    }

    pub(crate) fn word_end(line: &[u8], pos: usize) -> usize {
        let len = line.len();
        if pos + 1 >= len {
            return pos;
        }
        let mut p = pos + 1;
        while p < len && line[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= len {
            return len.saturating_sub(1);
        }
        if is_word_char(line[p]) {
            while p + 1 < len && is_word_char(line[p + 1]) {
                p += 1;
            }
        } else {
            while p + 1 < len && !is_word_char(line[p + 1]) && !line[p + 1].is_ascii_whitespace() {
                p += 1;
            }
        }
        p
    }

    pub(crate) fn bigword_end(line: &[u8], pos: usize) -> usize {
        let len = line.len();
        if pos + 1 >= len {
            return pos;
        }
        let mut p = pos + 1;
        while p < len && line[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= len {
            return len.saturating_sub(1);
        }
        while p + 1 < len && !line[p + 1].is_ascii_whitespace() {
            p += 1;
        }
        p
    }

    #[derive(Clone, Debug, PartialEq)]
    pub(crate) enum ViAction {
        Redraw,
        Bell,
        Return(Option<Vec<u8>>),
        ReadByte,
        WriteBytes(Vec<u8>),
        RunEditor { editor: Vec<u8>, tmp_path: Vec<u8> },
        NeedSearchByte,
        NeedFindTarget,
        NeedReplaceChar,
        NeedMotion,
        NeedReplaceModeInput,
        NeedLiteralChar,
        SetInsertMode(bool),
    }

    #[derive(Clone, Debug, PartialEq)]
    pub(crate) enum PendingInput {
        None,
        CountDigits,
        FindTarget { cmd: u8, count: usize },
        ReplaceChar { count: usize },
        ReplaceMode,
        Motion { op: u8, count: usize },
        LiteralChar,
        SearchInput { direction: u8 },
    }

    pub(crate) struct ViState {
        pub line: Vec<u8>,
        pub cursor: usize,
        pub insert_mode: bool,
        pub yank_buf: Vec<u8>,
        pub last_cmd: Option<(u8, usize, Option<u8>)>,
        pub last_find: Option<(u8, u8)>,
        pub hist_index: Option<usize>,
        pub edit_line: Vec<u8>,
        pub search_buf: Vec<u8>,
        pub count_buf: Option<(usize, u8)>,
        pub pending: PendingInput,
        erase_char: u8,
        hist_len: usize,
    }

    impl ViState {
        pub(crate) fn new(erase_char: u8, hist_len: usize) -> Self {
            Self {
                line: Vec::new(),
                cursor: 0,
                insert_mode: true,
                yank_buf: Vec::new(),
                last_cmd: None,
                last_find: None,
                hist_index: None,
                edit_line: Vec::new(),
                search_buf: Vec::new(),
                count_buf: None,
                pending: PendingInput::None,
                erase_char,
                hist_len,
            }
        }

        pub(crate) fn process_byte(&mut self, byte: u8, history: &[Box<[u8]>]) -> Vec<ViAction> {
            let mut actions = Vec::new();

            match &self.pending {
                PendingInput::CountDigits => {
                    if byte.is_ascii_digit() {
                        if let Some((ref mut count, _)) = self.count_buf {
                            *count = count
                                .saturating_mul(10)
                                .saturating_add((byte - b'0') as usize);
                        }
                        return vec![ViAction::ReadByte];
                    }
                    let (count, first_byte) = self.count_buf.take().unwrap();
                    self.pending = PendingInput::None;
                    return self.process_command(byte, count, first_byte, history);
                }
                PendingInput::FindTarget { cmd, count } => {
                    let cmd = *cmd;
                    let count = *count;
                    self.pending = PendingInput::None;
                    self.last_find = Some((cmd, byte));
                    for _ in 0..count {
                        if let Some(pos) = do_find(&self.line, self.cursor, cmd, byte) {
                            self.cursor = pos;
                        } else {
                            actions.push(ViAction::Bell);
                            break;
                        }
                    }
                    actions.push(ViAction::Redraw);
                    return actions;
                }
                PendingInput::ReplaceChar { count } => {
                    let count = *count;
                    self.pending = PendingInput::None;
                    self.last_cmd = Some((b'r', count, Some(byte)));
                    for _ in 0..count {
                        if self.cursor < self.line.len() {
                            self.line[self.cursor] = byte;
                            if self.cursor + 1 < self.line.len() {
                                self.cursor += 1;
                            }
                        }
                    }
                    if count > 1 && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    actions.push(ViAction::Redraw);
                    return actions;
                }
                PendingInput::ReplaceMode => match byte {
                    0x1b => {
                        self.pending = PendingInput::None;
                        if self.cursor > 0 && self.cursor >= self.line.len() {
                            self.cursor = self.line.len().saturating_sub(1);
                        }
                        actions.push(ViAction::Redraw);
                        return actions;
                    }
                    b'\r' | b'\n' => {
                        self.pending = PendingInput::None;
                        let mut s = self.line.clone();
                        s.push(b'\n');
                        return vec![
                            ViAction::WriteBytes(b"\r\n".to_vec()),
                            ViAction::Return(Some(s)),
                        ];
                    }
                    b => {
                        if self.cursor < self.line.len() {
                            self.line[self.cursor] = b;
                        } else {
                            self.line.push(b);
                        }
                        self.cursor += 1;
                        actions.push(ViAction::Redraw);
                        return actions;
                    }
                },
                PendingInput::Motion { op, count } => {
                    let op = *op;
                    let count = *count;
                    self.pending = PendingInput::None;
                    return self.process_motion(op, byte, count, &mut actions);
                }
                PendingInput::LiteralChar => {
                    self.pending = PendingInput::None;
                    self.line.insert(self.cursor, byte);
                    self.cursor += 1;
                    actions.push(ViAction::Redraw);
                    return actions;
                }
                PendingInput::SearchInput { direction } => {
                    let direction = *direction;
                    match byte {
                        b'\r' | b'\n' => {
                            self.pending = PendingInput::None;
                            if !self.search_buf.is_empty() {
                                self.do_search(direction, history, &mut actions);
                            }
                            actions.push(ViAction::Redraw);
                            return actions;
                        }
                        0x7f | 0x08 => {
                            if !self.search_buf.is_empty() {
                                self.search_buf.pop();
                                actions.push(ViAction::WriteBytes(b"\x08 \x08".to_vec()));
                            }
                            return actions;
                        }
                        b => {
                            self.search_buf.push(b);
                            actions.push(ViAction::WriteBytes(vec![b]));
                            return actions;
                        }
                    }
                }
                PendingInput::None => {}
            }

            if self.insert_mode {
                match byte {
                    0x1b => {
                        self.insert_mode = false;
                        if self.cursor > 0 && self.cursor >= self.line.len() {
                            self.cursor = self.line.len().saturating_sub(1);
                            actions.push(ViAction::WriteBytes(b"\x1b[D".to_vec()));
                        }
                    }
                    b'\n' | b'\r' => {
                        let mut s = self.line.clone();
                        s.push(b'\n');
                        actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                        actions.push(ViAction::Return(Some(s)));
                    }
                    0x16 => {
                        self.pending = PendingInput::LiteralChar;
                        actions.push(ViAction::NeedLiteralChar);
                    }
                    0x17 => {
                        if self.cursor > 0 {
                            let start = word_backward(&self.line, self.cursor);
                            self.line.drain(start..self.cursor);
                            self.cursor = start;
                            actions.push(ViAction::Redraw);
                        }
                    }
                    0x03 => {
                        actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                        actions.push(ViAction::Return(Some(Vec::new())));
                    }
                    0x04 => {
                        if self.line.is_empty() {
                            actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                            actions.push(ViAction::Return(None));
                        }
                    }
                    b if b == self.erase_char || b == 0x7f || b == 0x08 => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.line.remove(self.cursor);
                            actions.push(ViAction::Redraw);
                        }
                    }
                    _ => {
                        self.line.insert(self.cursor, byte);
                        self.cursor += 1;
                        if self.cursor == self.line.len() {
                            actions.push(ViAction::WriteBytes(vec![byte]));
                        } else {
                            actions.push(ViAction::Redraw);
                        }
                    }
                }
                return actions;
            }

            if byte.is_ascii_digit() && byte != b'0' {
                let count = (byte - b'0') as usize;
                self.count_buf = Some((count, byte));
                self.pending = PendingInput::CountDigits;
                return vec![ViAction::ReadByte];
            }

            self.process_command(byte, 1, byte, history)
        }

        fn process_command(
            &mut self,
            ch: u8,
            count: usize,
            first_byte: u8,
            history: &[Box<[u8]>],
        ) -> Vec<ViAction> {
            let mut actions = Vec::new();

            match ch {
                b'i' => {
                    self.insert_mode = true;
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'a' => {
                    self.insert_mode = true;
                    if !self.line.is_empty() {
                        self.cursor = (self.cursor + 1).min(self.line.len());
                        actions.push(ViAction::Redraw);
                    }
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'A' => {
                    self.insert_mode = true;
                    self.cursor = self.line.len();
                    actions.push(ViAction::Redraw);
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'I' => {
                    self.insert_mode = true;
                    self.cursor = 0;
                    actions.push(ViAction::Redraw);
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'h' => {
                    let n = count.min(self.cursor);
                    self.cursor -= n;
                    if n > 0 {
                        let esc = ByteWriter::new()
                            .bytes(b"\x1b[")
                            .usize_val(n)
                            .byte(b'D')
                            .finish();
                        actions.push(ViAction::WriteBytes(esc));
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                b'l' | b' ' => {
                    let max = self.line.len().saturating_sub(1);
                    let n = count.min(max.saturating_sub(self.cursor));
                    self.cursor += n;
                    if n > 0 {
                        let esc = ByteWriter::new()
                            .bytes(b"\x1b[")
                            .usize_val(n)
                            .byte(b'C')
                            .finish();
                        actions.push(ViAction::WriteBytes(esc));
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                b'0' if first_byte == b'0' => {
                    self.cursor = 0;
                    actions.push(ViAction::Redraw);
                }
                b'$' => {
                    if !self.line.is_empty() {
                        self.cursor = self.line.len() - 1;
                    }
                    actions.push(ViAction::Redraw);
                }
                b'^' => {
                    self.cursor = self
                        .line
                        .iter()
                        .position(|c| !c.is_ascii_whitespace())
                        .unwrap_or(0);
                    actions.push(ViAction::Redraw);
                }
                b'w' => {
                    for _ in 0..count {
                        let next = word_forward(&self.line, self.cursor);
                        if next == self.cursor {
                            actions.push(ViAction::Bell);
                            break;
                        }
                        self.cursor = next.min(self.line.len().saturating_sub(1));
                    }
                    actions.push(ViAction::Redraw);
                }
                b'W' => {
                    for _ in 0..count {
                        let next = bigword_forward(&self.line, self.cursor);
                        if next == self.cursor {
                            actions.push(ViAction::Bell);
                            break;
                        }
                        self.cursor = next.min(self.line.len().saturating_sub(1));
                    }
                    actions.push(ViAction::Redraw);
                }
                b'b' => {
                    for _ in 0..count {
                        let prev = word_backward(&self.line, self.cursor);
                        if prev == self.cursor {
                            actions.push(ViAction::Bell);
                            break;
                        }
                        self.cursor = prev;
                    }
                    actions.push(ViAction::Redraw);
                }
                b'B' => {
                    for _ in 0..count {
                        let prev = bigword_backward(&self.line, self.cursor);
                        if prev == self.cursor {
                            actions.push(ViAction::Bell);
                            break;
                        }
                        self.cursor = prev;
                    }
                    actions.push(ViAction::Redraw);
                }
                b'e' => {
                    for _ in 0..count {
                        self.cursor = word_end(&self.line, self.cursor);
                    }
                    actions.push(ViAction::Redraw);
                }
                b'E' => {
                    for _ in 0..count {
                        self.cursor = bigword_end(&self.line, self.cursor);
                    }
                    actions.push(ViAction::Redraw);
                }
                b'|' => {
                    let col = count
                        .saturating_sub(1)
                        .min(self.line.len().saturating_sub(1));
                    self.cursor = col;
                    actions.push(ViAction::Redraw);
                }
                b'f' | b'F' | b't' | b'T' => {
                    self.pending = PendingInput::FindTarget { cmd: ch, count };
                    actions.push(ViAction::NeedFindTarget);
                }
                b';' => {
                    if let Some((cmd, target)) = self.last_find {
                        for _ in 0..count {
                            if let Some(pos) = do_find(&self.line, self.cursor, cmd, target) {
                                self.cursor = pos;
                            } else {
                                actions.push(ViAction::Bell);
                                break;
                            }
                        }
                        actions.push(ViAction::Redraw);
                    }
                }
                b',' => {
                    if let Some((cmd, target)) = self.last_find {
                        let rev = match cmd {
                            b'f' => b'F',
                            b'F' => b'f',
                            b't' => b'T',
                            b'T' => b't',
                            _ => cmd,
                        };
                        for _ in 0..count {
                            if let Some(pos) = do_find(&self.line, self.cursor, rev, target) {
                                self.cursor = pos;
                            } else {
                                actions.push(ViAction::Bell);
                                break;
                            }
                        }
                        actions.push(ViAction::Redraw);
                    }
                }
                b'x' => {
                    self.last_cmd = Some((b'x', count, None));
                    for _ in 0..count {
                        if self.cursor < self.line.len() {
                            self.yank_buf = vec![self.line.remove(self.cursor)];
                        } else {
                            break;
                        }
                        if self.cursor >= self.line.len() && self.cursor > 0 {
                            self.cursor -= 1;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'X' => {
                    self.last_cmd = Some((b'X', count, None));
                    for _ in 0..count {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.yank_buf = vec![self.line.remove(self.cursor)];
                        } else {
                            actions.push(ViAction::Bell);
                            break;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'r' => {
                    self.pending = PendingInput::ReplaceChar { count };
                    actions.push(ViAction::NeedReplaceChar);
                }
                b'R' => {
                    self.pending = PendingInput::ReplaceMode;
                    actions.push(ViAction::NeedReplaceModeInput);
                }
                b'~' => {
                    for _ in 0..count {
                        if self.cursor < self.line.len() {
                            let c = self.line[self.cursor];
                            if c.is_ascii_lowercase() {
                                self.line[self.cursor] = c.to_ascii_uppercase();
                            } else if c.is_ascii_uppercase() {
                                self.line[self.cursor] = c.to_ascii_lowercase();
                            }
                            if self.cursor + 1 < self.line.len() {
                                self.cursor += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'd' => {
                    self.pending = PendingInput::Motion { op: b'd', count };
                    actions.push(ViAction::NeedMotion);
                }
                b'D' => {
                    if self.cursor < self.line.len() {
                        self.yank_buf = self.line[self.cursor..].to_vec();
                        self.line.truncate(self.cursor);
                        if self.cursor > 0 {
                            self.cursor -= 1;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'c' => {
                    self.pending = PendingInput::Motion { op: b'c', count };
                    actions.push(ViAction::NeedMotion);
                }
                b'C' => {
                    if self.cursor < self.line.len() {
                        self.yank_buf = self.line[self.cursor..].to_vec();
                        self.line.truncate(self.cursor);
                    }
                    self.insert_mode = true;
                    actions.push(ViAction::Redraw);
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'S' => {
                    self.yank_buf = self.line.clone();
                    self.line.clear();
                    self.cursor = 0;
                    self.insert_mode = true;
                    actions.push(ViAction::Redraw);
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'y' => {
                    self.pending = PendingInput::Motion { op: b'y', count };
                    actions.push(ViAction::NeedMotion);
                }
                b'Y' => {
                    if self.cursor < self.line.len() {
                        self.yank_buf = self.line[self.cursor..].to_vec();
                    }
                }
                b'p' => {
                    if !self.yank_buf.is_empty() {
                        let pos = (self.cursor + 1).min(self.line.len());
                        for b in self.yank_buf.clone().iter().rev() {
                            self.line.insert(pos, *b);
                        }
                        self.cursor = pos + self.yank_buf.len() - 1;
                        actions.push(ViAction::Redraw);
                    }
                }
                b'P' => {
                    if !self.yank_buf.is_empty() {
                        let yb = self.yank_buf.clone();
                        for (i, b) in yb.iter().enumerate() {
                            self.line.insert(self.cursor + i, *b);
                        }
                        self.cursor += self.yank_buf.len().saturating_sub(1);
                        actions.push(ViAction::Redraw);
                    }
                }
                b'u' => {
                    let saved = self.line.clone();
                    let saved_cursor = self.cursor;
                    self.line.clear();
                    self.line.extend_from_slice(&self.edit_line);
                    self.edit_line = saved;
                    self.cursor = saved_cursor.min(self.line.len().saturating_sub(1));
                    actions.push(ViAction::Redraw);
                }
                b'U' => {
                    if let Some(idx) = self.hist_index {
                        if idx < self.hist_len {
                            self.line = history[idx].to_vec();
                        }
                    } else {
                        self.line.clear();
                    }
                    self.cursor = self.cursor.min(self.line.len().saturating_sub(1));
                    if self.line.is_empty() {
                        self.cursor = 0;
                    }
                    actions.push(ViAction::Redraw);
                }
                b'.' => {
                    if let Some((cmd, prev_count, arg)) = self.last_cmd {
                        let c = if first_byte.is_ascii_digit() && first_byte != b'0' {
                            count
                        } else {
                            prev_count
                        };
                        replay_cmd(
                            &mut self.line,
                            &mut self.cursor,
                            &mut self.yank_buf,
                            cmd,
                            c,
                            arg,
                        );
                        actions.push(ViAction::Redraw);
                    }
                }
                b'k' | b'-' => {
                    let hist_len = self.hist_len;
                    let target = match self.hist_index {
                        None => {
                            if hist_len > 0 {
                                self.edit_line = self.line.clone();
                                Some(hist_len - 1)
                            } else {
                                None
                            }
                        }
                        Some(idx) => {
                            if idx > 0 {
                                Some(idx - 1)
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(idx) = target {
                        self.hist_index = Some(idx);
                        self.line = history[idx].to_vec();
                        self.cursor = self.line.len().saturating_sub(1);
                        if self.line.is_empty() {
                            self.cursor = 0;
                        }
                        actions.push(ViAction::Redraw);
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                b'j' | b'+' => {
                    let hist_len = self.hist_len;
                    if let Some(idx) = self.hist_index {
                        if idx + 1 < hist_len {
                            self.hist_index = Some(idx + 1);
                            self.line = history[idx + 1].to_vec();
                        } else {
                            self.hist_index = None;
                            self.line = self.edit_line.clone();
                        }
                        self.cursor = self.line.len().saturating_sub(1);
                        if self.line.is_empty() {
                            self.cursor = 0;
                        }
                        actions.push(ViAction::Redraw);
                    } else {
                        actions.push(ViAction::Bell);
                    }
                }
                b'G' => {
                    let hist_len = self.hist_len;
                    if first_byte.is_ascii_digit() && first_byte != b'0' {
                        let target =
                            count.saturating_sub(1).min(hist_len.saturating_sub(1));
                        if target < hist_len {
                            if self.hist_index.is_none() {
                                self.edit_line = self.line.clone();
                            }
                            self.hist_index = Some(target);
                            self.line = history[target].to_vec();
                        }
                    } else if hist_len > 0 {
                        if self.hist_index.is_none() {
                            self.edit_line = self.line.clone();
                        }
                        self.hist_index = Some(0);
                        self.line = history[0].to_vec();
                    }
                    self.cursor = self.line.len().saturating_sub(1);
                    if self.line.is_empty() {
                        self.cursor = 0;
                    }
                    actions.push(ViAction::Redraw);
                }
                b'/' => {
                    actions.push(ViAction::WriteBytes(b"/".to_vec()));
                    self.search_buf.clear();
                    self.pending = PendingInput::SearchInput { direction: b'/' };
                    actions.push(ViAction::NeedSearchByte);
                }
                b'?' => {
                    actions.push(ViAction::WriteBytes(b"?".to_vec()));
                    self.search_buf.clear();
                    self.pending = PendingInput::SearchInput { direction: b'?' };
                    actions.push(ViAction::NeedSearchByte);
                }
                b'n' => {
                    if !self.search_buf.is_empty() {
                        self.do_search(b'/', history, &mut actions);
                        actions.push(ViAction::Redraw);
                    }
                }
                b'N' => {
                    if !self.search_buf.is_empty() {
                        self.do_search(b'?', history, &mut actions);
                        actions.push(ViAction::Redraw);
                    }
                }
                b'#' => {
                    self.line.insert(0, b'#');
                    let mut s = self.line.clone();
                    s.push(b'\n');
                    actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                    actions.push(ViAction::Return(Some(s)));
                }
                b'v' => {
                    let mut tmp = b"/tmp/meiksh_vi_edit_".to_vec();
                    bstr::push_u64(&mut tmp, sys::current_pid() as u64);
                    if let Ok(fd) = sys::open_file(
                        &tmp,
                        sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC,
                        0o600,
                    ) {
                        let _ = sys::write_all_fd(fd, &self.line);
                        let _ = sys::write_all_fd(fd, b"\n");
                        let _ = sys::close_fd(fd);
                    }
                    actions.push(ViAction::RunEditor {
                        editor: Vec::new(),
                        tmp_path: tmp,
                    });
                }
                b'*' => {
                    let word_start = {
                        let mut p = self.cursor;
                        while p > 0 && !self.line[p - 1].is_ascii_whitespace() {
                            p -= 1;
                        }
                        p
                    };
                    let word_end_pos = {
                        let mut p = self.cursor;
                        while p < self.line.len() && !self.line[p].is_ascii_whitespace() {
                            p += 1;
                        }
                        p
                    };
                    let raw = &self.line[word_start..word_end_pos];
                    let pattern = if raw.contains(&b'*')
                        || raw.contains(&b'?')
                        || raw.contains(&b'[')
                    {
                        raw.to_vec()
                    } else {
                        let mut p = raw.to_vec();
                        p.push(b'*');
                        p
                    };
                    if let Ok(expanded) = glob_expand(&pattern) {
                        let mut replacement = Vec::new();
                        for (i, entry) in expanded.iter().enumerate() {
                            if i > 0 {
                                replacement.push(b' ');
                            }
                            replacement.extend_from_slice(entry);
                        }
                        self.line.drain(word_start..word_end_pos);
                        for (i, b) in replacement.iter().enumerate() {
                            self.line.insert(word_start + i, *b);
                        }
                        self.cursor = word_start + replacement.len();
                        if self.cursor > 0 {
                            self.cursor -= 1;
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'\\' => {
                    let word_start = {
                        let mut p = self.cursor;
                        while p > 0 && !self.line[p - 1].is_ascii_whitespace() {
                            p -= 1;
                        }
                        p
                    };
                    let word_end_pos = {
                        let mut p = self.cursor;
                        while p < self.line.len() && !self.line[p].is_ascii_whitespace() {
                            p += 1;
                        }
                        p
                    };
                    let prefix = self.line[word_start..word_end_pos].to_vec();
                    let mut glob_pat = prefix.clone();
                    glob_pat.push(b'*');
                    if let Ok(matches) = glob_expand(&glob_pat) {
                        if matches.len() == 1 {
                            let replacement = &matches[0];
                            let is_dir = sys::stat_path(replacement)
                                .map(|s| s.is_dir())
                                .unwrap_or(false);
                            let mut rep = replacement.clone();
                            if is_dir {
                                rep.push(b'/');
                            }
                            self.line.drain(word_start..word_end_pos);
                            for (i, b) in rep.iter().enumerate() {
                                self.line.insert(word_start + i, *b);
                            }
                            self.cursor = word_start + rep.len();
                            if self.cursor > 0 && !is_dir {
                                self.cursor -= 1;
                            }
                        } else {
                            actions.push(ViAction::Bell);
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                0x03 => {
                    actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                    actions.push(ViAction::Return(Some(Vec::new())));
                }
                b'\r' | b'\n' => {
                    let mut s = self.line.clone();
                    s.push(b'\n');
                    actions.push(ViAction::WriteBytes(b"\r\n".to_vec()));
                    actions.push(ViAction::Return(Some(s)));
                }
                _ => {
                    actions.push(ViAction::Bell);
                }
            }
            actions
        }

        fn process_motion(
            &mut self,
            op: u8,
            motion: u8,
            count: usize,
            actions: &mut Vec<ViAction>,
        ) -> Vec<ViAction> {
            match op {
                b'd' => {
                    if motion == b'd' {
                        self.yank_buf = self.line.clone();
                        self.line.clear();
                        self.cursor = 0;
                        self.last_cmd = Some((b'd', count, Some(b'd')));
                    } else {
                        let (start, end) =
                            resolve_motion(&self.line, self.cursor, motion, count);
                        if start != end {
                            self.yank_buf = self.line[start..end].to_vec();
                            self.line.drain(start..end);
                            self.cursor = start.min(self.line.len().saturating_sub(1));
                            self.last_cmd = Some((b'd', count, Some(motion)));
                        } else {
                            actions.push(ViAction::Bell);
                        }
                    }
                    actions.push(ViAction::Redraw);
                }
                b'c' => {
                    if motion == b'c' {
                        self.yank_buf = self.line.clone();
                        self.line.clear();
                        self.cursor = 0;
                        self.last_cmd = Some((b'c', count, Some(b'c')));
                    } else {
                        let (start, end) =
                            resolve_motion(&self.line, self.cursor, motion, count);
                        if start != end {
                            self.yank_buf = self.line[start..end].to_vec();
                            self.line.drain(start..end);
                            self.cursor = start;
                            self.last_cmd = Some((b'c', count, Some(motion)));
                        }
                    }
                    self.insert_mode = true;
                    actions.push(ViAction::Redraw);
                    actions.push(ViAction::SetInsertMode(true));
                }
                b'y' => {
                    if motion == b'y' {
                        self.yank_buf = self.line.clone();
                    } else {
                        let (start, end) =
                            resolve_motion(&self.line, self.cursor, motion, count);
                        if start != end {
                            self.yank_buf = self.line[start..end].to_vec();
                        }
                    }
                }
                _ => {}
            }
            std::mem::take(actions)
        }

        pub(crate) fn do_search(
            &mut self,
            direction: u8,
            history: &[Box<[u8]>],
            actions: &mut Vec<ViAction>,
        ) {
            let pat = &self.search_buf;
            let hist_len = self.hist_len;
            match direction {
                b'/' => {
                    let start = self
                        .hist_index
                        .map(|i| i.wrapping_sub(1))
                        .unwrap_or(hist_len.wrapping_sub(1));
                    let mut found = false;
                    let mut idx = start;
                    for _ in 0..hist_len {
                        if idx >= hist_len {
                            break;
                        }
                        if history[idx]
                            .windows(pat.len())
                            .any(|w| w == pat.as_slice())
                        {
                            self.hist_index = Some(idx);
                            self.line = history[idx].to_vec();
                            self.cursor = self.line.len().saturating_sub(1);
                            found = true;
                            break;
                        }
                        idx = idx.wrapping_sub(1);
                    }
                    if !found {
                        actions.push(ViAction::Bell);
                    }
                }
                b'?' => {
                    let start = self
                        .hist_index
                        .map(|i| (i + 1).min(hist_len))
                        .unwrap_or(0);
                    let mut found = false;
                    for idx in start..hist_len {
                        if history[idx]
                            .windows(pat.len())
                            .any(|w| w == pat.as_slice())
                        {
                            self.hist_index = Some(idx);
                            self.line = history[idx].to_vec();
                            self.cursor = self.line.len().saturating_sub(1);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        actions.push(ViAction::Bell);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn read_line(shell: &mut Shell, prompt: &[u8]) -> sys::SysResult<Option<Vec<u8>>> {
        let _raw = match RawMode::enter() {
            Ok(r) => r,
            Err(_) => return super::read_line(),
        };

        let erase_char = {
            if let Ok(attrs) = sys::get_terminal_attrs(sys::STDIN_FILENO) {
                attrs.c_cc[libc::VERASE]
            } else {
                0x7f
            }
        };

        let hist_len = shell.history.len();
        let mut state = ViState::new(erase_char, hist_len);

        loop {
            let byte = match read_byte()? {
                Some(b) => b,
                None => {
                    if state.line.is_empty() && state.cursor == 0 {
                        write_bytes(b"\r\n");
                        return Ok(None);
                    }
                    continue;
                }
            };

            let actions = state.process_byte(byte, &shell.history);
            for action in actions {
                match action {
                    ViAction::Redraw => {
                        redraw(&state.line, state.cursor, prompt);
                    }
                    ViAction::Bell => {
                        bell();
                    }
                    ViAction::Return(result) => {
                        return Ok(result);
                    }
                    ViAction::ReadByte => {}
                    ViAction::WriteBytes(data) => {
                        write_bytes(&data);
                    }
                    ViAction::RunEditor { tmp_path, .. } => {
                        let editor = shell
                            .get_var(b"VISUAL")
                            .or_else(|| shell.get_var(b"EDITOR"))
                            .unwrap_or(b"vi")
                            .to_vec();
                        let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, &_raw.saved);
                        write_bytes(b"\r\n");
                        let mut edit_cmd = editor;
                        edit_cmd.push(b' ');
                        edit_cmd.extend_from_slice(&tmp_path);
                        let _ = shell.execute_string(&edit_cmd);
                        let mut raw_restored = _raw.saved;
                        raw_restored.c_lflag &=
                            !(libc::ICANON | libc::ECHO | libc::ISIG);
                        raw_restored.c_cc[libc::VMIN] = 1;
                        raw_restored.c_cc[libc::VTIME] = 0;
                        let _ =
                            sys::set_terminal_attrs(sys::STDIN_FILENO, &raw_restored);
                        if let Ok(content) = sys::read_file(&tmp_path) {
                            let mut end = content.len();
                            while end > 0
                                && (content[end - 1] == b' '
                                    || content[end - 1] == b'\t'
                                    || content[end - 1] == b'\n'
                                    || content[end - 1] == b'\r')
                            {
                                end -= 1;
                            }
                            let trimmed = &content[..end];
                            if !trimmed.is_empty() {
                                super::remove_file_bytes(&tmp_path);
                                write_bytes(b"\r\n");
                                let mut s = trimmed.to_vec();
                                s.push(b'\n');
                                return Ok(Some(s));
                            }
                        }
                        super::remove_file_bytes(&tmp_path);
                        redraw(&state.line, state.cursor, prompt);
                    }
                    ViAction::NeedSearchByte
                    | ViAction::NeedFindTarget
                    | ViAction::NeedReplaceChar
                    | ViAction::NeedMotion
                    | ViAction::NeedReplaceModeInput
                    | ViAction::NeedLiteralChar => {}
                    ViAction::SetInsertMode(_) => {}
                }
            }
        }
    }

    pub(crate) fn do_find(
        line: &[u8],
        cursor: usize,
        cmd: u8,
        target: u8,
    ) -> Option<usize> {
        match cmd {
            b'f' => {
                for i in (cursor + 1)..line.len() {
                    if line[i] == target {
                        return Some(i);
                    }
                }
                None
            }
            b'F' => {
                for i in (0..cursor).rev() {
                    if line[i] == target {
                        return Some(i);
                    }
                }
                None
            }
            b't' => {
                for i in (cursor + 1)..line.len() {
                    if line[i] == target {
                        return if i > 0 { Some(i - 1) } else { None };
                    }
                }
                None
            }
            b'T' => {
                for i in (0..cursor).rev() {
                    if line[i] == target {
                        return Some(i + 1);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn resolve_motion(
        line: &[u8],
        cursor: usize,
        motion: u8,
        count: usize,
    ) -> (usize, usize) {
        let target = match motion {
            b'w' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = word_forward(line, p);
                }
                p
            }
            b'W' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = bigword_forward(line, p);
                }
                p
            }
            b'b' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = word_backward(line, p);
                }
                p
            }
            b'B' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = bigword_backward(line, p);
                }
                p
            }
            b'e' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = word_end(line, p);
                }
                p + 1
            }
            b'E' => {
                let mut p = cursor;
                for _ in 0..count {
                    p = bigword_end(line, p);
                }
                p + 1
            }
            b'h' => return (cursor.saturating_sub(count), cursor),
            b'l' | b' ' => {
                let end = (cursor + count).min(line.len());
                return (cursor, end);
            }
            b'0' => return (0, cursor),
            b'$' => return (cursor, line.len()),
            _ => return (cursor, cursor),
        };
        if target < cursor {
            (target, cursor)
        } else {
            (cursor, target.min(line.len()))
        }
    }

    pub(crate) fn replay_cmd(
        line: &mut Vec<u8>,
        cursor: &mut usize,
        yank_buf: &mut Vec<u8>,
        cmd: u8,
        count: usize,
        arg: Option<u8>,
    ) {
        match cmd {
            b'x' => {
                for _ in 0..count {
                    if *cursor < line.len() {
                        *yank_buf = vec![line.remove(*cursor)];
                    }
                    if *cursor >= line.len() && *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            b'X' => {
                for _ in 0..count {
                    if *cursor > 0 {
                        *cursor -= 1;
                        *yank_buf = vec![line.remove(*cursor)];
                    }
                }
            }
            b'r' => {
                if let Some(replacement) = arg {
                    for _ in 0..count {
                        if *cursor < line.len() {
                            line[*cursor] = replacement;
                            if *cursor + 1 < line.len() {
                                *cursor += 1;
                            }
                        }
                    }
                    if count > 1 && *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            b'd' => {
                if let Some(motion) = arg {
                    if motion == b'd' {
                        *yank_buf = line.clone();
                        line.clear();
                        *cursor = 0;
                    } else {
                        let (start, end) = resolve_motion(line, *cursor, motion, count);
                        if start != end {
                            *yank_buf = line[start..end].to_vec();
                            line.drain(start..end);
                            *cursor = start.min(line.len().saturating_sub(1));
                        }
                    }
                }
            }
            b'c' => {
                if let Some(motion) = arg {
                    if motion == b'c' {
                        *yank_buf = line.clone();
                        line.clear();
                        *cursor = 0;
                    } else {
                        let (start, end) = resolve_motion(line, *cursor, motion, count);
                        if start != end {
                            *yank_buf = line[start..end].to_vec();
                            line.drain(start..end);
                            *cursor = start;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn glob_expand(pattern: &[u8]) -> Result<Vec<Vec<u8>>, ()> {
        let c_pattern = std::ffi::CString::new(pattern.to_vec()).map_err(|_| ())?;
        let mut glob_buf: libc::glob_t = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            libc::glob(
                c_pattern.as_ptr(),
                libc::GLOB_TILDE | libc::GLOB_MARK,
                None,
                &mut glob_buf,
            )
        };
        if ret != 0 {
            unsafe { libc::globfree(&mut glob_buf) };
            return Err(());
        }
        let mut results = Vec::new();
        for i in 0..glob_buf.gl_pathc {
            let path = unsafe { std::ffi::CStr::from_ptr(*glob_buf.gl_pathv.add(i)) };
            results.push(path.to_bytes().to_vec());
        }
        unsafe { libc::globfree(&mut glob_buf) };
        Ok(results)
    }
}
