use std::rc::Rc;

use crate::expand::word;
use crate::shell::error::ShellError;
use crate::shell::state::{PendingControl, Shell};
use crate::syntax::ast::{
    CaseCommand, Command, ForCommand, IfCommand, LoopCommand, LoopKind, Program, RedirectionKind,
};
use crate::sys;

use super::process::ExpandedRedirection;
use super::program::execute_list_item;
use super::redirection::{RedirectionRef, apply_shell_redirections};
use super::render::case_pattern_matches;
use super::simple::{execute_simple, expand_redirections, var_error_bytes};

pub(super) fn execute_command(shell: &mut Shell, command: &Command) -> Result<i32, ShellError> {
    execute_command_inner(shell, command, false)
}

pub(super) fn execute_command_in_pipeline_child(
    shell: &mut Shell,
    command: &Command,
) -> Result<i32, ShellError> {
    execute_command_inner(shell, command, true)
}

pub(super) fn execute_command_inner(
    shell: &mut Shell,
    command: &Command,
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    match command {
        Command::Simple(simple) => execute_simple(shell, simple, allow_exec_in_place),
        Command::Subshell(program) => {
            let pid = sys::process::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
            if pid == 0 {
                let mut child_shell = shell.clone();
                child_shell.owns_terminal = false;
                child_shell.in_subshell = true;
                child_shell.restore_signals_for_child();
                let _ = child_shell.reset_traps_for_subshell();
                let status = match execute_nested_program(&mut child_shell, program) {
                    Ok(s) => s,
                    Err(error) => error.exit_status(),
                };
                let status = child_shell.run_exit_trap(status).unwrap_or(status);
                sys::process::exit_process(status as sys::types::RawFd);
            }
            let ws = loop {
                match sys::process::wait_pid(pid, false) {
                    Ok(Some(ws)) => break ws,
                    Ok(None) => continue,
                    Err(e) if e.is_eintr() => continue,
                    Err(e) => return Err(shell.diagnostic_syserr(1, &e)),
                }
            };
            Ok(sys::process::decode_wait_status(ws.status))
        }
        Command::Group(program) => execute_nested_program(shell, program),
        Command::FunctionDef(function) => {
            shell
                .functions_mut()
                .insert(function.name.to_vec(), Rc::clone(&function.body));
            Ok(0)
        }
        Command::If(if_command) => execute_if(shell, if_command),
        Command::Loop(loop_command) => execute_loop(shell, loop_command),
        Command::For(for_command) => execute_for(shell, for_command),
        Command::Case(case_command) => execute_case(shell, case_command),
        Command::Redirected(command, redirections) => {
            execute_redirected(shell, command, redirections, allow_exec_in_place)
        }
    }
}

pub(super) fn execute_redirected(
    shell: &mut Shell,
    command: &Command,
    redirections: &[crate::syntax::ast::Redirection],
    allow_exec_in_place: bool,
) -> Result<i32, ShellError> {
    let expanded = expand_redirections(shell, redirections)?;
    if let Some(first) = expanded.first() {
        shell.lineno = first.line;
    }
    let guard = match apply_shell_redirections(&expanded, shell.options.noclobber) {
        Ok(guard) => guard,
        Err(error) => return Ok(shell.diagnostic_syserr(1, &error).exit_status()),
    };
    let result = execute_command_inner(shell, command, allow_exec_in_place);
    drop(guard);
    result
}

pub(super) fn execute_if(shell: &mut Shell, if_command: &IfCommand) -> Result<i32, ShellError> {
    let saved_suppressed = shell.errexit_suppressed;
    shell.errexit_suppressed = true;
    let cond = execute_nested_program(shell, &if_command.condition)?;
    shell.errexit_suppressed = saved_suppressed;
    if cond == 0 {
        return execute_nested_program(shell, &if_command.then_branch);
    }

    for branch in &if_command.elif_branches {
        shell.errexit_suppressed = true;
        let cond = execute_nested_program(shell, &branch.condition)?;
        shell.errexit_suppressed = saved_suppressed;
        if cond == 0 {
            return execute_nested_program(shell, &branch.body);
        }
    }

    if let Some(else_branch) = &if_command.else_branch {
        execute_nested_program(shell, else_branch)
    } else {
        Ok(0)
    }
}

pub(super) fn execute_loop(
    shell: &mut Shell,
    loop_command: &LoopCommand,
) -> Result<i32, ShellError> {
    shell.loop_depth += 1;
    let result = (|| {
        let mut last_status = 0;
        loop {
            shell.run_pending_traps()?;
            let saved_suppressed = shell.errexit_suppressed;
            shell.errexit_suppressed = true;
            let condition_status = execute_nested_program(shell, &loop_command.condition)?;
            shell.errexit_suppressed = saved_suppressed;
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
            let should_run = match loop_command.kind {
                LoopKind::While => condition_status == 0,
                LoopKind::Until => condition_status != 0,
            };
            if !should_run {
                break;
            }
            last_status = execute_nested_program(shell, &loop_command.body)?;
            if !shell.running {
                break;
            }
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
        }
        Ok(last_status)
    })();
    shell.loop_depth = shell.loop_depth.saturating_sub(1);
    result
}

pub(super) fn execute_for(shell: &mut Shell, for_command: &ForCommand) -> Result<i32, ShellError> {
    let values: Vec<Vec<u8>> = if let Some(items) = &for_command.items {
        let mut values = Vec::with_capacity(items.len());
        word::expand_words_into(shell, items, &mut values).map_err(|e| shell.expand_to_err(e))?;
        values
    } else {
        shell.positional.clone()
    };

    shell.loop_depth += 1;
    let result = (|| {
        let mut last_status = 0;
        for value in values {
            shell.set_var(&for_command.name, &value).map_err(|e| {
                let msg = var_error_bytes(&e);
                shell.diagnostic(1, &msg)
            })?;
            last_status = execute_nested_program(shell, &for_command.body)?;
            if !shell.running {
                break;
            }
            match shell.pending_control {
                Some(PendingControl::Return(_)) => break,
                Some(PendingControl::Break(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Break(levels - 1));
                    } else {
                        shell.pending_control = None;
                    }
                    break;
                }
                Some(PendingControl::Continue(levels)) => {
                    if levels > 1 {
                        shell.pending_control = Some(PendingControl::Continue(levels - 1));
                        break;
                    }
                    shell.pending_control = None;
                    continue;
                }
                None => {}
            }
        }
        Ok(last_status)
    })();
    shell.loop_depth = shell.loop_depth.saturating_sub(1);
    result
}

pub(super) fn execute_case(
    shell: &mut Shell,
    case_command: &CaseCommand,
) -> Result<i32, ShellError> {
    let word =
        word::expand_word_text(shell, &case_command.word).map_err(|e| shell.expand_to_err(e))?;
    let arms = &case_command.arms;
    let mut matched = false;
    for (i, arm) in arms.iter().enumerate() {
        if !matched {
            for pattern in &arm.patterns {
                let pattern = word::expand_word_pattern(shell, pattern)
                    .map_err(|e| shell.expand_to_err(e))?;
                if case_pattern_matches(&word, &pattern) {
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            let status = execute_nested_program(shell, &arm.body)?;
            if !arm.fallthrough {
                return Ok(status);
            }
            if i + 1 >= arms.len() {
                return Ok(status);
            }
        }
    }
    Ok(0)
}

impl RedirectionRef for ExpandedRedirection {
    fn fd(&self) -> i32 {
        self.fd
    }
    fn kind(&self) -> RedirectionKind {
        self.kind
    }
    fn target(&self) -> &[u8] {
        &self.target
    }
    fn here_doc_body(&self) -> Option<&[u8]> {
        self.here_doc_body.as_deref()
    }
}

pub(super) fn execute_nested_program(
    shell: &mut Shell,
    program: &Program,
) -> Result<i32, ShellError> {
    let mut status = 0;
    for item in &program.items {
        shell.run_pending_traps()?;
        status = execute_list_item(shell, item)?;
        shell.last_status = status;
        if !shell.running || shell.has_pending_control() {
            break;
        }
    }
    Ok(status)
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::program::execute_program;
    use crate::exec::test_support::{parse_test, t_stderr, test_shell};
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn execute_if_and_loop_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = parse_test(
                "if false; then VALUE=no; elif true; then VALUE=yes; else VALUE=bad; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let while_program = parse_test(
                "COUNTER=1; while case $COUNTER in 0) false ;; *) true ;; esac; do COUNTER=0; FLAG=done; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &while_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"FLAG"), Some(b"done" as &[u8]));

            let mut shell = test_shell();
            let until_program = parse_test(
                "READY=; until case $READY in yes) true ;; *) false ;; esac; do READY=yes; VALUE=ready; done",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &until_program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"ready" as &[u8]));
        });
    }

    #[test]
    fn execute_for_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("for item in a b c; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"LAST"), Some(b"c" as &[u8]));

            let mut shell = test_shell();
            shell.positional = vec![b"alpha".to_vec(), b"beta".to_vec()];
            let program = parse_test("for item; do LAST=$item; done").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"LAST"), Some(b"beta" as &[u8]));
        });
    }

    #[test]
    fn execute_case_commands() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program =
                parse_test("name=beta; case $name in alpha) VALUE=no ;; b*) VALUE=yes ;; esac")
                    .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let program = parse_test("name=zeta; case $name in alpha|beta) VALUE=hit ;; esac")
                .expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case a in a) A=1 ;& b) B=2 ;; c) C=3 ;; esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"A"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"B"), Some(b"2" as &[u8]));
            assert_eq!(shell.get_var(b"C"), None);

            let mut shell = test_shell();
            let program =
                parse_test("case x in x) V=one ;& y) V=two ;& z) V=three ;& esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), Some(b"three" as &[u8]));
        });
    }

    #[test]
    fn execute_if_covers_then_and_else_branches() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"PATH".to_vec(), b"/usr/bin:/bin".to_vec());
            shell.exported_mut().insert(b"PATH".to_vec());

            let if_program =
                parse_test("if true; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"yes" as &[u8]));

            let mut shell = test_shell();
            let if_program =
                parse_test("if false; then VALUE=yes; else VALUE=no; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"no" as &[u8]));

            let mut shell = test_shell();
            let if_program = parse_test(
                "if false; then VALUE=yes; elif false; then VALUE=maybe; else VALUE=no; fi",
            )
            .expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"no" as &[u8]));
        });
    }

    #[test]
    fn loop_and_function_exit_behavior() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let if_program = parse_test("if false; then VALUE=yes; fi").expect("parse");
            let status = execute_program(&mut shell, &if_program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), None);

            let mut shell = test_shell();
            let for_program = parse_test("for item in a b; do exit 9; done").expect("parse");
            let status = execute_program(&mut shell, &for_program).expect("exec");
            assert_eq!(status, 9);
            assert!(!shell.running);
            assert_eq!(shell.get_var(b"item"), Some(b"a" as &[u8]));

            let mut shell = test_shell();
            let loop_program = parse_test("while true; do exit 7; done").expect("parse");
            let status = execute_program(&mut shell, &loop_program).expect("exec");
            assert_eq!(status, 7);
            assert!(!shell.running);

            let mut shell = test_shell();
            let program = parse_test("greet() { RESULT=$X; }; X=ok greet").expect("parse");
            let status = execute_program(&mut shell, &program).expect("exec");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"RESULT"), Some(b"ok" as &[u8]));
        });
    }

    #[test]
    fn control_flow_propagates_across_functions_and_loops() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: line 1: break: only meaningful in a loop\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let program = parse_test("f() { return 6; VALUE=bad; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 6);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.pending_control, None);

                let mut shell = test_shell();
                let program = parse_test(
                "for outer in x y; do for inner in a b; do continue 2; VALUE=bad; done; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.get_var(b"outer"), Some(b"y" as &[u8]));

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x y; do for inner in a b; do break 2; done; VALUE=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), None);
                assert_eq!(shell.get_var(b"outer"), Some(b"x" as &[u8]));

                let mut shell = test_shell();
                let program =
                    parse_test("f() { while true; do return 4; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 4);
                assert_eq!(shell.pending_control, None);

                let mut shell = test_shell();
                let program = parse_test("g() { break; }; g").expect("parse");
                let error = execute_program(&mut shell, &program).expect_err("function error");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do while break 2; do printf no; done; AFTER=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"AFTER"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do while continue 2; do printf no; done; AFTER=bad; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"AFTER"), None);

                let mut shell = test_shell();
                let program =
                    parse_test("f() { while return 3; do printf no; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 3);

                let mut shell = test_shell();
                let program = parse_test(
                "COUNT=1; while case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; do printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"COUNT"), Some(b"0" as &[u8]));

                let mut shell = test_shell();
                let program = parse_test(
                "COUNT=1; while true; do case $COUNT in 0) break ;; *) COUNT=0; continue ;; esac; printf no; done",
            )
            .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"COUNT"), Some(b"0" as &[u8]));

                let mut shell = test_shell();
                let program =
                    parse_test("f() { for item in a; do return 5; done; }; f").expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 5);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do for inner in y; do break 2; done; DONE=no; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"DONE"), None);

                let mut shell = test_shell();
                let program = parse_test(
                    "for outer in x; do for inner in y; do continue 2; done; DONE=no; done",
                )
                .expect("parse");
                let status = execute_program(&mut shell, &program).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"DONE"), None);

                let mut shell = test_shell();
                shell.loop_depth = 1;
                let loop_command = LoopCommand {
                    kind: LoopKind::While,
                    condition: parse_test("true").expect("parse"),
                    body: parse_test("break 2").expect("parse"),
                };
                let status = execute_loop(&mut shell, &loop_command).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.pending_control, Some(PendingControl::Break(1)));

                let mut shell = test_shell();
                shell.loop_depth = 1;
                let loop_command = LoopCommand {
                    kind: LoopKind::While,
                    condition: parse_test("true").expect("parse"),
                    body: parse_test("continue 2").expect("parse"),
                };
                let status = execute_loop(&mut shell, &loop_command).expect("exec");
                assert_eq!(status, 0);
                assert_eq!(shell.pending_control, Some(PendingControl::Continue(1)));
            },
        );
    }

    #[test]
    fn case_fallthrough_stops_at_end() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("case a in a) V=one ;& esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), Some(b"one" as &[u8]));
        });
    }

    #[test]
    fn case_no_match_returns_zero() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("case z in a) V=bad ;; b) V=bad ;; esac").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"V"), None);
        });
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn for_readonly_variable_error() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: line 1: item: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.readonly_mut().insert(b"item".to_vec());
                let err = shell
                    .execute_string(b"for item in a b; do :; done")
                    .expect_err("readonly loop var");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn group_command_executes_inline() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("{ X=hello; Y=world; }").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"X"), Some(b"hello" as &[u8]));
            assert_eq!(shell.get_var(b"Y"), Some(b"world" as &[u8]));
        });
    }

    #[test]
    fn subshell_program_error() {
        run_trace(
            trace_entries![
                fork() -> pid(100), child: [
                    write(fd(2), bytes(b"meiksh: line 1: bad: parameter null or not set\n")) -> auto,
                ],
                waitpid(100, _) -> status(1),
            ],
            || {
                let mut shell = test_shell();
                let subshell_program = parse_test("( ${bad:?} )").expect("parse");
                let status = execute_program(&mut shell, &subshell_program).expect("execute");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn subshell_wait_errors() {
        run_trace(
            trace_entries![
                fork() -> pid(100), child: [],
                waitpid(100, _) -> int(0),
                waitpid(100, _) -> err(sys::constants::EINTR),
                waitpid(100, _) -> err(sys::constants::ECHILD),
                write(fd(2), bytes(b"meiksh: line 1: No child processes\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let subshell_program = parse_test("( true )").expect("parse");
                let err = execute_program(&mut shell, &subshell_program).unwrap_err();
                assert_eq!(err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn compound_command_redirection_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10),
                open(_, _, _) -> err(sys::constants::EACCES),
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
                write(fd(2), bytes(b"meiksh: line 1: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let program = parse_test("{ true; } > /forbidden").expect("parse");
                let status = execute_program(&mut shell, &program).expect("execute");
                assert_eq!(status, 1);
            },
        );
    }

    #[test]
    fn function_def_registers_and_calls() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("myfn() { RESULT=ok; }; myfn").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"RESULT"), Some(b"ok" as &[u8]));
            assert!(shell.functions().contains_key(&b"myfn".to_vec()));
        });
    }
}
