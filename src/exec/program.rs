#![allow(unused_imports)]

use std::collections::HashMap;

use crate::arena::ByteArena;
use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand;
use crate::shell::{
    BlockingWaitOutcome, FlowSignal, JobState, PendingControl, Shell, ShellError, VarError,
};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};
use crate::sys;

use super::*;

pub fn execute_program(shell: &mut Shell, program: &Program) -> Result<i32, ShellError> {
    let mut status = 0;
    for item in &program.items {
        status = execute_list_item(shell, item)?;
        shell.last_status = status;
        shell.run_pending_traps()?;
        if !shell.running || shell.has_pending_control() {
            break;
        }
    }
    Ok(status)
}

pub(super) fn check_errexit(shell: &mut Shell, status: i32) {
    if status != 0 && shell.options.errexit && !shell.errexit_suppressed {
        shell.running = false;
        shell.last_status = status;
    }
}

pub(super) fn execute_list_item(shell: &mut Shell, item: &ListItem) -> Result<i32, ShellError> {
    shell.lineno = item.line;
    shell.env.insert(
        b"LINENO".to_vec(),
        crate::bstr::u64_to_bytes(item.line as u64),
    );
    if item.asynchronous {
        let stdin_override = if !shell.options.monitor {
            Some(
                sys::open_file(b"/dev/null", sys::O_RDONLY, 0)
                    .map_err(|e| shell.diagnostic_syserr(1, &e))?,
            )
        } else {
            None
        };
        let spawned = spawn_and_or(shell, &item.and_or, stdin_override)?;
        let last_pid = spawned.children.last().map(|c| c.pid).unwrap_or(0);
        let description = render_and_or(&item.and_or);
        let id = shell.register_background_job(description.into(), spawned.pgid, spawned.children);
        if shell.interactive {
            let msg = ByteWriter::new()
                .byte(b'[')
                .usize_val(id)
                .bytes(b"] ")
                .i64_val(last_pid as i64)
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        }
        Ok(0)
    } else {
        execute_and_or(shell, &item.and_or)
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::test_support::*;
    use crate::shell::Shell;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::trace_entries;

    #[test]
    fn execute_and_or_skips_rhs_when_guard_fails() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("true || false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);

            let program = parse_test("false && true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
        });
    }

    #[test]
    fn errexit_exits_on_nonzero_simple_command() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("false").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 1);
            assert!(!shell.running);
        });
    }

    #[test]
    fn errexit_does_not_exit_on_zero_status() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn errexit_suppressed_in_if_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            let program = parse_test("if false; then :; fi; true").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn check_errexit_does_nothing_when_disabled() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = false;
            check_errexit(&mut shell, 1);
            assert!(shell.running);
        });
    }

    #[test]
    fn check_errexit_does_nothing_when_suppressed() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            shell.errexit_suppressed = true;
            check_errexit(&mut shell, 1);
            assert!(shell.running);
        });
    }

    #[test]
    fn check_errexit_stops_shell_on_failure() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            check_errexit(&mut shell, 1);
            assert!(!shell.running);
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn lineno_parse_error_unterminated_single_quote() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 3: unterminated single quote")]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\ntrue\necho '");
            },
        );
    }

    #[test]
    fn lineno_parse_error_unterminated_double_quote() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 2: unterminated double quote")]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\necho \"hello");
            },
        );
    }

    #[test]
    fn lineno_parse_error_empty_if_condition() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 3: expected command list after 'if'",)]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\nif\nthen true; fi");
            },
        );
    }

    #[test]
    fn lineno_expand_nounset_on_line_2() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 2: MISSING: parameter not set")]],
            || {
                let mut shell = test_shell();
                shell.options.nounset = true;
                let _ = shell.execute_string(b"true\necho $MISSING");
            },
        );
    }

    #[test]
    fn lineno_expand_error_on_line_3() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 3: must be set")]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\ntrue\n: ${NOVAR?must be set}");
            },
        );
    }

    #[test]
    fn lineno_runtime_break_outside_loop() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 2: break: only meaningful in a loop",)]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"true\nbreak");
            },
        );
    }

    #[test]
    fn lineno_runtime_readonly_assignment() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 2: X: readonly variable")]],
            || {
                let mut shell = test_shell();
                let _ = shell.execute_string(b"readonly X=1\nX=2");
            },
        );
    }

    #[test]
    fn lineno_env_var_matches_shell_lineno() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"true\ntrue\ntrue");
            assert_eq!(shell.get_var(b"LINENO"), Some(b"3" as &[u8]));
        });
    }

    #[test]
    fn lineno_env_var_updates_per_list_item() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let _ = shell.execute_string(b"A=$LINENO\ntrue\nB=$LINENO");
            assert_eq!(shell.get_var(b"A"), Some(b"1" as &[u8]));
            assert_eq!(shell.get_var(b"B"), Some(b"3" as &[u8]));
        });
    }

    #[test]
    fn special_builtin_utility_error_exits_noninteractive() {
        run_trace(
            trace_entries![write(
                fd(sys::STDERR_FILENO),
                bytes(b"meiksh: line 1: set: invalid option: Z\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                let err = shell.execute_string(b"set -Z").expect_err("sbi error");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }

    #[test]
    fn special_builtin_utility_error_continues_interactive() {
        run_trace(
            trace_entries![write(
                fd(sys::STDERR_FILENO),
                bytes(b"meiksh: set: invalid option: Z\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let status = shell.execute_string(b"set -Z").expect("sbi interactive");
                assert_ne!(status, 0);
            },
        );
    }

    #[test]
    fn check_errexit_zero_status_does_nothing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            check_errexit(&mut shell, 0);
            assert!(shell.running);
        });
    }

    #[test]
    fn execute_program_stops_on_shell_not_running() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let program = parse_test("exit 5; AFTER=bad").expect("parse");
            let status = execute_program(&mut shell, &program).expect("execute");
            assert_eq!(status, 5);
            assert!(!shell.running);
            assert_eq!(shell.get_var(b"AFTER"), None);
        });
    }
}
