use std::rc::Rc;

use crate::arena::ByteArena;
use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand::word;
use crate::shell::error::{ShellError, VarError};
use crate::shell::state::{FlowSignal, PendingControl, Shell};
use crate::syntax::ast::{RedirectionKind, SimpleCommand};
use crate::sys;

use super::and_or::ProcessGroupPlan;
use super::command::execute_command;
use super::pipeline::wait_for_external_child;
use super::process::{
    ExpandedRedirection, ExpandedSimpleCommand, PreparedProcess, ProcessRedirection,
    exec_prepared_in_current_process, join_boxed_bytes, resolve_command_path, spawn_prepared,
};
use super::redirection::{
    apply_shell_redirection, apply_shell_redirections, default_fd_for_redirection,
};

pub(super) fn var_error_bytes(e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(name)
            .bytes(b": readonly variable")
            .finish(),
    }
}

pub(super) struct SavedVar {
    pub(super) name: Box<[u8]>,
    pub(super) value: Option<Vec<u8>>,
    pub(super) was_exported: bool,
}

pub(super) fn save_vars(shell: &Shell, assignments: &[(Vec<u8>, Vec<u8>)]) -> Vec<SavedVar> {
    assignments
        .iter()
        .map(|(name, _)| SavedVar {
            name: name.clone().into(),
            value: shell.get_var(name).map(|s| s.to_vec()),
            was_exported: shell.exported.contains(name),
        })
        .collect()
}

pub(super) fn apply_prefix_assignments(
    shell: &mut Shell,
    assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<(), ShellError> {
    for (name, value) in assignments {
        shell.set_var(name, value.clone()).map_err(|e| {
            let msg = var_error_bytes(&e);
            shell.diagnostic(1, &msg)
        })?;
    }
    Ok(())
}

pub(super) fn restore_vars(shell: &mut Shell, saved: Vec<SavedVar>) {
    for entry in saved {
        let name: Vec<u8> = entry.name.into();
        match entry.value {
            Some(v) => {
                shell.env.insert(name.clone(), v);
            }
            None => {
                shell.env.remove(&name);
            }
        }
        if entry.was_exported {
            shell.exported.insert(name);
        } else {
            shell.exported.remove(&name);
        }
    }
}

pub(super) enum BuiltinResult {
    Status(i32),
    UtilityError(i32),
}

pub(super) fn run_builtin_flow(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinResult, ShellError> {
    match shell.run_builtin(argv, assignments)? {
        FlowSignal::Continue(status) => Ok(BuiltinResult::Status(status)),
        FlowSignal::UtilityError(status) => Ok(BuiltinResult::UtilityError(status)),
        FlowSignal::Exit(status) => {
            shell.running = false;
            Ok(BuiltinResult::Status(status))
        }
    }
}

pub(super) fn write_xtrace(shell: &mut Shell, expanded: &ExpandedSimpleCommand<'_>) {
    if !shell.options.xtrace {
        return;
    }
    let arena = ByteArena::new();
    let ps4_raw = shell.get_var(b"PS4").unwrap_or(b"+ ").to_vec();
    let prefix = word::expand_parameter_text(shell, &ps4_raw, &arena).unwrap_or(b"+ ");
    let mut line = prefix.to_vec();
    for (name, value) in &expanded.assignments {
        line.extend_from_slice(name);
        line.push(b'=');
        line.extend_from_slice(value);
        line.push(b' ');
    }
    for (i, word) in expanded.argv.iter().enumerate() {
        if i > 0 {
            line.push(b' ');
        }
        line.extend_from_slice(word);
    }
    line.push(b'\n');
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &line);
}

pub(super) fn has_command_substitution(simple: &SimpleCommand) -> bool {
    simple.assignments.iter().any(|a| {
        let raw: &[u8] = &a.value.raw;
        raw.windows(2).any(|w| w == b"$(") || raw.contains(&b'`')
    }) || simple.words.iter().any(|w| {
        let raw: &[u8] = &w.raw;
        raw.windows(2).any(|w| w == b"$(") || raw.contains(&b'`')
    })
}

pub(super) fn execute_simple(
    shell: &mut Shell,
    simple: &SimpleCommand,
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    let arena = ByteArena::new();
    let expanded = expand_simple(shell, simple, &arena)?;

    if let Some(first_word) = simple.words.first() {
        shell.lineno = first_word.line;
    }

    if !expanded.argv.is_empty() || !expanded.assignments.is_empty() {
        write_xtrace(shell, &expanded);
    }

    let owned_argv: Vec<Vec<u8>> = expanded.argv.iter().map(|s| s.to_vec()).collect();
    let owned_assignments: Vec<(Vec<u8>, Vec<u8>)> = expanded
        .assignments
        .iter()
        .map(|&(n, v)| (n.to_vec(), v.to_vec()))
        .collect();

    if expanded.argv.is_empty() {
        let cmd_sub_status = if has_command_substitution(simple) {
            shell.last_status
        } else {
            0
        };
        let guard = match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
        {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        for (name, value) in &owned_assignments {
            shell.set_var(name, value.clone()).map_err(|e| {
                let msg = var_error_bytes(&e);
                shell.diagnostic(1, &msg)
            })?;
        }
        drop(guard);
        return Ok(cmd_sub_status);
    }

    let is_special_builtin = builtin::is_special_builtin(&owned_argv[0]);
    let is_exec_no_cmd = is_special_builtin
        && owned_argv[0] == b"exec"
        && !owned_argv.iter().skip(1).any(|a| a == b"--");

    if is_exec_no_cmd {
        for redir in &expanded.redirections {
            shell.lineno = redir.line;
            apply_shell_redirection(redir, shell.options.noclobber)
                .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        }
        return match run_builtin_flow(shell, &owned_argv, &owned_assignments) {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    } else if is_special_builtin {
        let _guard = apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
            .map_err(|e| shell.diagnostic_syserr(1, &e))?;
        let result = run_builtin_flow(shell, &owned_argv, &owned_assignments);
        drop(_guard);
        return match result {
            Ok(BuiltinResult::UtilityError(status)) if !shell.interactive => {
                Err(ShellError::Status(status))
            }
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Err(error),
        };
    }

    if let Some(function) = shell.functions.get(&owned_argv[0]).map(Rc::clone) {
        let guard = match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
        {
            Ok(g) => g,
            Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
        };
        let saved_vars = save_vars(shell, &owned_assignments);
        if let Err(e) = apply_prefix_assignments(shell, &owned_assignments) {
            restore_vars(shell, saved_vars);
            drop(guard);
            return Err(e);
        }
        let saved = std::mem::replace(&mut shell.positional, owned_argv[1..].to_vec());
        shell.function_depth += 1;
        let status = execute_command(shell, &function);
        shell.function_depth = shell.function_depth.saturating_sub(1);
        shell.positional = saved;
        restore_vars(shell, saved_vars);
        drop(guard);
        match status {
            Ok(status) => match shell.pending_control {
                Some(PendingControl::Return(return_status)) => {
                    shell.pending_control = None;
                    Ok(return_status)
                }
                _ => Ok(status),
            },
            Err(error) => Err(error),
        }
    } else if builtin::is_builtin(&owned_argv[0]) {
        let saved_vars = save_vars(shell, &owned_assignments);
        let assign_result = apply_prefix_assignments(shell, &owned_assignments);
        let result = match assign_result {
            Ok(()) => {
                let r =
                    match apply_shell_redirections(&expanded.redirections, shell.options.noclobber)
                    {
                        Ok(guard) => {
                            let r = run_builtin_flow(shell, &owned_argv, &[]);
                            drop(guard);
                            r
                        }
                        Err(e) => Err(shell.diagnostic_syserr(1, &e)),
                    };
                restore_vars(shell, saved_vars);
                r
            }
            Err(error) => {
                restore_vars(shell, saved_vars);
                Err(error)
            }
        };
        match result {
            Ok(BuiltinResult::Status(status) | BuiltinResult::UtilityError(status)) => Ok(status),
            Err(error) => Ok(error.exit_status()),
        }
    } else {
        for (name, _value) in &owned_assignments {
            if shell.readonly.contains(name) {
                let mut msg = name.clone();
                msg.extend_from_slice(b": readonly variable");
                return Err(shell.diagnostic(1, &msg));
            }
        }
        let command_name = owned_argv[0].clone();
        let prepared = build_process_from_expanded(shell, expanded, owned_argv, owned_assignments)
            .expect("argv is non-empty");
        if !prepared.path_verified && !prepared.exec_path.contains(&b'/') {
            let _guard = apply_shell_redirections(&prepared.redirections, prepared.noclobber).ok();
            let msg = ByteWriter::new()
                .bytes(&command_name)
                .bytes(b": not found\n")
                .finish();
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
            return Ok(127);
        }
        let desc = join_boxed_bytes(&prepared.argv, b' ');
        if allow_exec_in_place {
            exec_prepared_in_current_process(shell, &prepared, ProcessGroupPlan::None)
        } else if shell.in_subshell {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::None) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let status = wait_for_external_child(shell, &handle, None, Some(&desc))?;
            Ok(status)
        } else {
            let handle = match spawn_prepared(shell, &prepared, ProcessGroupPlan::NewGroup) {
                Ok(h) => h,
                Err(error) => return Ok(error.exit_status()),
            };
            let pgid = handle.pid;
            let _ = sys::tty::set_process_group(pgid, pgid);
            let status = wait_for_external_child(shell, &handle, Some(pgid), Some(&desc))?;
            Ok(status)
        }
    }
}

pub(super) fn is_declaration_utility(name: &[u8]) -> bool {
    name == b"export" || name == b"readonly"
}

pub(super) fn find_declaration_context(words: &[crate::syntax::ast::Word]) -> bool {
    let mut i = 0;
    while i < words.len() {
        let raw: &[u8] = &words[i].raw;
        if raw == b"command" {
            i += 1;
            while i < words.len() && words[i].raw.starts_with(b"-") {
                i += 1;
            }
            continue;
        }
        return is_declaration_utility(raw);
    }
    false
}

pub(super) fn expand_simple<'a>(
    shell: &mut Shell,
    simple: &SimpleCommand,
    arena: &'a ByteArena,
) -> Result<ExpandedSimpleCommand<'a>, ShellError> {
    let mut assignments = Vec::new();
    for assignment in &simple.assignments {
        let value = word::expand_assignment_value(shell, &assignment.value, arena)
            .map_err(|e| shell.expand_to_err(e))?;
        assignments.push((arena.intern_bytes(&assignment.name), value));
    }

    let declaration_ctx = find_declaration_context(&simple.words);
    let argv = if declaration_ctx {
        expand_words_declaration(shell, &simple.words, arena)?
    } else {
        word::expand_words(shell, &simple.words, arena).map_err(|e| shell.expand_to_err(e))?
    };
    let mut redirections = Vec::new();
    for redirection in &simple.redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                word::expand_here_document(shell, &here_doc.body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_bytes(&here_doc.body)
            };
            (arena.intern_bytes(&here_doc.delimiter), Some(body))
        } else {
            let target = word::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        redirections.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }

    Ok(ExpandedSimpleCommand {
        assignments,
        argv,
        redirections,
    })
}

pub(super) fn parse_i32_bytes(s: &[u8]) -> Option<i32> {
    crate::bstr::parse_i64(s).and_then(|v| i32::try_from(v).ok())
}

pub(super) fn expand_words_declaration<'a>(
    shell: &mut Shell,
    words: &[crate::syntax::ast::Word],
    arena: &'a ByteArena,
) -> Result<Vec<&'a [u8]>, ShellError> {
    let mut result = Vec::new();
    let mut found_cmd = false;
    for word in words {
        if !found_cmd {
            result
                .extend(word::expand_word(shell, word, arena).map_err(|e| shell.expand_to_err(e))?);
            if result
                .last()
                .is_some_and(|s: &&[u8]| !s.is_empty() && *s != b"command")
            {
                found_cmd = true;
            }
        } else if word::word_is_assignment(&word.raw) {
            result.push(
                word::expand_word_as_declaration_assignment(shell, word, arena)
                    .map_err(|e| shell.expand_to_err(e))?,
            );
        } else {
            result
                .extend(word::expand_word(shell, word, arena).map_err(|e| shell.expand_to_err(e))?);
        }
    }
    Ok(result)
}

pub(super) fn expand_redirections<'a>(
    shell: &mut Shell,
    redirections: &[crate::syntax::ast::Redirection],
    arena: &'a ByteArena,
) -> Result<Vec<ExpandedRedirection<'a>>, ShellError> {
    let mut expanded_vec = Vec::new();
    for redirection in redirections {
        let fd = redirection
            .fd
            .unwrap_or_else(|| default_fd_for_redirection(redirection.kind));
        let (target, here_doc_body) = if redirection.kind == RedirectionKind::HereDoc {
            let here_doc = redirection
                .here_doc
                .as_ref()
                .ok_or_else(|| shell.diagnostic(2, b"missing here-document body" as &[u8]))?;
            let body = if here_doc.expand {
                word::expand_here_document(shell, &here_doc.body, here_doc.body_line, arena)
                    .map_err(|e| shell.expand_to_err(e))?
            } else {
                arena.intern_bytes(&here_doc.body)
            };
            (arena.intern_bytes(&here_doc.delimiter), Some(body))
        } else {
            let target = word::expand_redirect_word(shell, &redirection.target, arena)
                .map_err(|e| shell.expand_to_err(e))?;
            if matches!(
                redirection.kind,
                RedirectionKind::DupInput | RedirectionKind::DupOutput
            ) && target != b"-"
                && parse_i32_bytes(target).is_none()
            {
                return Err(shell.diagnostic(
                    1,
                    b"redirection target must be a file descriptor or '-'" as &[u8],
                ));
            }
            (target, None)
        };
        expanded_vec.push(ExpandedRedirection {
            fd,
            kind: redirection.kind,
            target,
            here_doc_body,
            line: redirection.target.line,
        });
    }
    Ok(expanded_vec)
}

pub(super) fn build_process_from_expanded(
    shell: &Shell,
    expanded: ExpandedSimpleCommand<'_>,
    owned_argv: Vec<Vec<u8>>,
    owned_assignments: Vec<(Vec<u8>, Vec<u8>)>,
) -> Result<PreparedProcess, ShellError> {
    let program = expanded
        .argv
        .first()
        .ok_or_else(|| shell.diagnostic(1, b"empty command" as &[u8]))?;
    let prefix_path = expanded
        .assignments
        .iter()
        .find(|&&(name, _)| name == b"PATH")
        .map(|&(_, value)| value);
    let resolved = resolve_command_path(shell, program, prefix_path);
    let path_verified = resolved.is_some();
    let exec_path: Vec<u8> = resolved.unwrap_or_else(|| program.to_vec());
    let mut child_env = shell.env_for_child();
    child_env.extend(owned_assignments);
    let redirections = expanded
        .redirections
        .into_iter()
        .map(|r| ProcessRedirection {
            fd: r.fd,
            kind: r.kind,
            target: r.target.to_vec().into(),
            here_doc_body: r.here_doc_body.map(|s| s.to_vec().into()),
        })
        .collect();
    Ok(PreparedProcess {
        exec_path: exec_path.into(),
        argv: owned_argv
            .into_iter()
            .map(Vec::into_boxed_slice)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        child_env: child_env
            .into_iter()
            .map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        redirections,
        noclobber: shell.options.noclobber,
        path_verified,
    })
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::program::execute_program;
    use crate::exec::test_support::{parse_test, test_shell};
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn save_restore_vars_restores_previous_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            shell.exported.insert(b"FOO".to_vec());

            let assignments = vec![
                (b"FOO".to_vec(), b"temp".to_vec()),
                (b"BAR".to_vec(), b"new".to_vec()),
            ];
            let saved = save_vars(&shell, &assignments);

            shell.set_var(b"FOO", b"temp".to_vec()).unwrap();
            shell.set_var(b"BAR", b"new".to_vec()).unwrap();
            assert_eq!(shell.get_var(b"FOO"), Some(b"temp" as &[u8]));
            assert_eq!(shell.get_var(b"BAR"), Some(b"new" as &[u8]));

            restore_vars(&mut shell, saved);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
            assert!(shell.exported.contains(&b"FOO".to_vec()));
            assert_eq!(shell.get_var(b"BAR"), None);
            assert!(!shell.exported.contains(&b"BAR".to_vec()));
        });
    }

    #[test]
    fn non_special_builtin_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("FOO=temp true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn special_builtin_prefix_assignments_are_permanent() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=permanent :").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"permanent" as &[u8]));
        });
    }

    #[test]
    fn function_prefix_assignments_are_temporary() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"FOO".to_vec(), b"original".to_vec());
            let program = parse_test("myfn() { :; }; FOO=temp myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"original" as &[u8]));
        });
    }

    #[test]
    fn non_special_builtin_exit_with_temp_assignments() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("FOO=bar exit 0").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(!shell.running);
        });
    }

    #[test]
    fn assignment_expansion_does_not_field_split() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"IFS".to_vec(), b" ".to_vec());
            shell.env.insert(b"X".to_vec(), b"a b c".to_vec());
            let program = parse_test("Y=$X").expect("parse");
            let _status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(shell.get_var(b"Y"), Some(b"a b c" as &[u8]));
        });
    }

    #[test]
    fn xtrace_writes_trace_to_stderr() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ echo hello\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],

                    argv: vec![b"echo", b"hello"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn xtrace_skipped_when_disabled() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.xtrace = false;
            let expanded = ExpandedSimpleCommand {
                assignments: vec![],

                argv: vec![b"echo"],
                redirections: vec![],
            };
            write_xtrace(&mut shell, &expanded);
        });
    }

    #[test]
    fn xtrace_includes_assignments() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ FOO=bar cmd\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(b"FOO", b"bar")],
                    argv: vec![b"cmd"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn has_command_substitution_detects_backtick_in_words() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                words: vec![Word {
                    raw: b"echo `date`".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd));

            let cmd_no_sub = SimpleCommand {
                words: vec![Word {
                    raw: b"plain".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(!has_command_substitution(&cmd_no_sub));
        });
    }

    #[test]
    fn declaration_builtin_expands_assignments_and_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let status = shell
                .execute_string(b"command export FOO=bar BAZ")
                .expect("export");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FOO"), Some(b"bar" as &[u8]));
        });
    }

    #[test]
    fn var_error_bytes_formats_readonly() {
        assert_no_syscalls(|| {
            let err = VarError::Readonly(b"HOME".to_vec().into());
            assert_eq!(var_error_bytes(&err), b"HOME: readonly variable");
        });
    }

    #[test]
    fn parse_i32_bytes_various_inputs() {
        assert_no_syscalls(|| {
            assert_eq!(parse_i32_bytes(b"0"), Some(0));
            assert_eq!(parse_i32_bytes(b"42"), Some(42));
            assert_eq!(parse_i32_bytes(b"-1"), Some(-1));
            assert_eq!(parse_i32_bytes(b"2147483647"), Some(i32::MAX));
            assert_eq!(parse_i32_bytes(b"-2147483648"), Some(i32::MIN));
            assert_eq!(parse_i32_bytes(b"2147483648"), None);
            assert_eq!(parse_i32_bytes(b""), None);
            assert_eq!(parse_i32_bytes(b"abc"), None);
            assert_eq!(parse_i32_bytes(b"-"), None);
        });
    }

    #[test]
    fn is_declaration_utility_matches_export_and_readonly() {
        assert_no_syscalls(|| {
            assert!(is_declaration_utility(b"export"));
            assert!(is_declaration_utility(b"readonly"));
            assert!(!is_declaration_utility(b"echo"));
            assert!(!is_declaration_utility(b"command"));
            assert!(!is_declaration_utility(b""));
        });
    }

    #[test]
    fn find_declaration_context_with_command_prefix() {
        assert_no_syscalls(|| {
            let words = |strs: &[&[u8]]| -> Vec<Word> {
                strs.iter()
                    .map(|s| Word {
                        raw: s.to_vec().into(),
                        line: 0,
                    })
                    .collect()
            };

            assert!(find_declaration_context(&words(&[b"export"])));
            assert!(find_declaration_context(&words(&[b"readonly"])));
            assert!(find_declaration_context(&words(&[b"command", b"export"])));
            assert!(find_declaration_context(&words(&[
                b"command", b"-v", b"export"
            ])));
            assert!(!find_declaration_context(&words(&[b"echo"])));
            assert!(!find_declaration_context(&words(&[b"command"])));
            assert!(!find_declaration_context(&words(&[])));
        });
    }

    #[test]
    fn has_command_substitution_dollar_paren_in_assignments() {
        assert_no_syscalls(|| {
            let cmd = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"$(date)".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(has_command_substitution(&cmd));

            let cmd_backtick_assign = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"`date`".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(has_command_substitution(&cmd_backtick_assign));

            let cmd_dollar_paren_word = SimpleCommand {
                words: vec![Word {
                    raw: b"echo $(date)".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            assert!(has_command_substitution(&cmd_dollar_paren_word));

            let cmd_none = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"plain".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![].into_boxed_slice(),
            };
            assert!(!has_command_substitution(&cmd_none));
        });
    }

    #[test]
    fn readonly_var_blocks_external_cmd_prefix_assignment() {
        run_trace(
            trace_entries![write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: line 1: X: readonly variable\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                shell.readonly.insert(b"X".to_vec());
                let err = shell
                    .execute_string(b"X=val /nonexistent/cmd")
                    .expect_err("readonly prefix");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn write_xtrace_with_custom_ps4() {
        run_trace(
            trace_entries![write(fd(2), bytes(b">> echo hi\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                shell.env.insert(b"PS4".to_vec(), b">> ".to_vec());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![],
                    argv: vec![b"echo", b"hi"],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn write_xtrace_empty_argv_with_assignments_only() {
        run_trace(
            trace_entries![write(fd(2), bytes(b"+ A=1 B=2 \n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.options.xtrace = true;
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(b"A" as &[u8], b"1" as &[u8]), (b"B", b"2")],
                    argv: vec![],
                    redirections: vec![],
                };
                write_xtrace(&mut shell, &expanded);
            },
        );
    }

    #[test]
    fn apply_prefix_assignments_readonly_error() {
        run_trace(
            trace_entries![write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: RO: readonly variable\n")) -> auto],
            || {
                let mut shell = test_shell();
                shell.readonly.insert(b"RO".to_vec());
                let assignments = vec![(b"RO".to_vec(), b"newval".to_vec())];
                let err = apply_prefix_assignments(&mut shell, &assignments)
                    .expect_err("readonly should fail");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn empty_command_redirection_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10),
                open(_, _, _) -> err(libc::EACCES),
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("> /forbidden").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn exec_no_cmd_redirection_error() {
        run_trace(
            trace_entries![
                open(_, _, _) -> err(libc::EACCES),
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("exec > /forbidden").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn exec_no_cmd_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 exec").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn function_redirection_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10),
                open(_, _, _) -> err(libc::EACCES),
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let fn_prog = parse_test("myfn() { true; }").unwrap();
                execute_program(&mut shell, &fn_prog).unwrap();
                let prog = parse_test("myfn > /forbidden").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn builtin_prefix_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 echo hi").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn external_command_prefix_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let prog = parse_test("x=2 /bin/true").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn external_command_spawn_error_subshell() {
        run_trace(
            trace_entries![
                fork() -> pid(123), child: [
                    fork() -> err(libc::ENOMEM),
                    write(fd(2), bytes(b"/bin/fail: Cannot allocate memory\n")) -> auto,

                ],
                waitpid(123, _) -> status(1),
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("( /bin/fail )").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn external_command_spawn_error_main() {
        run_trace(
            trace_entries![
                fork() -> err(libc::ENOMEM),
                write(fd(2), bytes(b"/bin/fail: Cannot allocate memory\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prog = parse_test("/bin/fail").unwrap();
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn assignment_only_command_without_words() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("X=1 Y=2").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"X"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"Y"), Some(b"2" as &[u8]));
        });
    }

    #[test]
    fn test_execute_expanded_readonly() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: RO: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"RO");
                let prog = parse_test("RO=val /bin/echo").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }
    #[test]
    fn prefix_assignment_readonly_external_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: line 1: y: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"y");
                // Put y=2 in a position where the assignment loop evaluates the second item, hitting the end of the block. Wait, the loop condition returns early. To hit the loop increment/continue, we just need a successful assignment followed by a failing one, or just a successful one.
                let prog = parse_test("x=1 y=2 /bin/true").unwrap();
                let err = execute_program(&mut shell, &prog).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }
}
