use std::collections::BTreeSet;
use std::rc::Rc;

use super::alias::shell_quote;
use super::{BuiltinOutcome, write_stdout_line};
use crate::bstr;
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::shell::traps::{TrapAction, TrapCondition};
use crate::syntax::ast::Program;
use crate::sys;

pub(super) fn trap(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
    match trap_impl(shell, argv) {
        Ok(status) => BuiltinOutcome::Status(status),
        Err(error) => BuiltinOutcome::Status(error.exit_status()),
    }
}

pub(super) fn trap_impl(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<i32, ShellError> {
    if argv.len() == 1 {
        print_traps(shell, false, &[])?;
        return Ok(0);
    }
    if argv[1] == b"-p" {
        print_traps(shell, true, &argv[2..])?;
        return Ok(0);
    }
    let (action_index, conditions_start) = if argv[1] == b"--" {
        if argv.len() == 2 {
            print_traps(shell, false, &[])?;
            return Ok(0);
        }
        (2, 3)
    } else {
        (1, 2)
    };
    if is_unsigned_decimal(&argv[action_index]) {
        for condition in &argv[action_index..] {
            if let Some(condition) = parse_trap_condition(condition) {
                shell.set_trap(condition, None)?;
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"trap: invalid condition: ")
                    .bytes(condition)
                    .finish();
                shell.diagnostic(1, &msg);
                return Ok(1);
            }
        }
        return Ok(0);
    }
    let action = &argv[action_index];
    if argv.len() <= conditions_start {
        return Err(shell.diagnostic(1, b"trap: condition argument required"));
    }
    let trap_action = parse_trap_action(action);
    let mut status = 0;
    for condition in &argv[conditions_start..] {
        let Some(condition) = parse_trap_condition(condition) else {
            let msg = ByteWriter::new()
                .bytes(b"trap: invalid condition: ")
                .bytes(condition)
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
            continue;
        };
        shell.set_trap(condition, trap_action.clone())?;
    }
    Ok(status)
}

pub(super) fn print_traps(
    shell: &Shell,
    include_defaults: bool,
    operands: &[Vec<u8>],
) -> Result<(), ShellError> {
    let conditions = if operands.is_empty() {
        if include_defaults {
            supported_trap_conditions()
        } else if let Some(saved) = &shell.subshell_saved_traps {
            let mut keys: BTreeSet<TrapCondition> = shell.trap_actions.keys().copied().collect();
            keys.extend(saved.keys().copied());
            keys.into_iter().collect()
        } else {
            shell.trap_actions.keys().copied().collect()
        }
    } else {
        let mut parsed = Vec::new();
        for operand in operands {
            let Some(condition) = parse_trap_condition(operand) else {
                let msg = ByteWriter::new()
                    .bytes(b"trap: invalid condition: ")
                    .bytes(operand)
                    .finish();
                return Err(shell.diagnostic(1, &msg));
            };
            parsed.push(condition);
        }
        parsed
    };
    for condition in conditions {
        if let Some(action) =
            trap_output_action(shell, condition, include_defaults, !operands.is_empty())
        {
            let line = ByteWriter::new()
                .bytes(b"trap -- ")
                .bytes(&action)
                .byte(b' ')
                .bytes(&format_trap_condition(condition))
                .finish();
            write_stdout_line(&line);
        }
    }
    Ok(())
}

pub(super) fn supported_trap_conditions() -> Vec<TrapCondition> {
    let mut conditions = vec![TrapCondition::Exit];
    conditions.extend(
        sys::process::supported_trap_signals()
            .iter()
            .copied()
            .map(TrapCondition::Signal),
    );
    conditions
}

pub(super) fn parse_trap_action(action: &[u8]) -> Option<TrapAction> {
    match action {
        b"-" => None,
        _ if action.is_empty() => Some(TrapAction::Ignore),
        _ => {
            let program =
                crate::syntax::parse_with_aliases(action, &crate::hash::ShellMap::default())
                    .unwrap_or_else(|_| Program {
                        items: Box::new([]),
                    });
            Some(TrapAction::Command {
                text: action.into(),
                program: Rc::new(program),
            })
        }
    }
}

pub(super) fn parse_trap_condition(text: &[u8]) -> Option<TrapCondition> {
    let name = if text.starts_with(b"SIG") {
        &text[3..]
    } else {
        text
    };
    match name {
        b"0" | b"EXIT" => Some(TrapCondition::Exit),
        b"HUP" | b"1" => Some(TrapCondition::Signal(sys::constants::SIGHUP)),
        b"INT" | b"2" => Some(TrapCondition::Signal(sys::constants::SIGINT)),
        b"QUIT" | b"3" => Some(TrapCondition::Signal(sys::constants::SIGQUIT)),
        b"ILL" | b"4" => Some(TrapCondition::Signal(sys::constants::SIGILL)),
        b"ABRT" | b"6" => Some(TrapCondition::Signal(sys::constants::SIGABRT)),
        b"FPE" | b"8" => Some(TrapCondition::Signal(sys::constants::SIGFPE)),
        b"KILL" | b"9" => Some(TrapCondition::Signal(sys::constants::SIGKILL)),
        b"USR1" | b"10" => Some(TrapCondition::Signal(sys::constants::SIGUSR1)),
        b"SEGV" | b"11" => Some(TrapCondition::Signal(sys::constants::SIGSEGV)),
        b"USR2" | b"12" => Some(TrapCondition::Signal(sys::constants::SIGUSR2)),
        b"PIPE" | b"13" => Some(TrapCondition::Signal(sys::constants::SIGPIPE)),
        b"ALRM" | b"14" => Some(TrapCondition::Signal(sys::constants::SIGALRM)),
        b"TERM" | b"15" => Some(TrapCondition::Signal(sys::constants::SIGTERM)),
        b"CHLD" | b"17" => Some(TrapCondition::Signal(sys::constants::SIGCHLD)),
        b"STOP" | b"19" => Some(TrapCondition::Signal(sys::constants::SIGSTOP)),
        b"CONT" | b"18" => Some(TrapCondition::Signal(sys::constants::SIGCONT)),
        b"TRAP" | b"5" => Some(TrapCondition::Signal(sys::constants::SIGTRAP)),
        b"TSTP" | b"20" => Some(TrapCondition::Signal(sys::constants::SIGTSTP)),
        b"TTIN" | b"21" => Some(TrapCondition::Signal(sys::constants::SIGTTIN)),
        b"TTOU" | b"22" => Some(TrapCondition::Signal(sys::constants::SIGTTOU)),
        b"BUS" => Some(TrapCondition::Signal(sys::constants::SIGBUS)),
        b"SYS" => Some(TrapCondition::Signal(sys::constants::SIGSYS)),
        _ => None,
    }
}

pub(super) fn format_trap_condition(condition: TrapCondition) -> Vec<u8> {
    match condition {
        TrapCondition::Exit => b"EXIT".to_vec(),
        TrapCondition::Signal(sys::constants::SIGHUP) => b"HUP".to_vec(),
        TrapCondition::Signal(sys::constants::SIGINT) => b"INT".to_vec(),
        TrapCondition::Signal(sys::constants::SIGQUIT) => b"QUIT".to_vec(),
        TrapCondition::Signal(sys::constants::SIGILL) => b"ILL".to_vec(),
        TrapCondition::Signal(sys::constants::SIGABRT) => b"ABRT".to_vec(),
        TrapCondition::Signal(sys::constants::SIGFPE) => b"FPE".to_vec(),
        TrapCondition::Signal(sys::constants::SIGKILL) => b"KILL".to_vec(),
        TrapCondition::Signal(sys::constants::SIGUSR1) => b"USR1".to_vec(),
        TrapCondition::Signal(sys::constants::SIGSEGV) => b"SEGV".to_vec(),
        TrapCondition::Signal(sys::constants::SIGUSR2) => b"USR2".to_vec(),
        TrapCondition::Signal(sys::constants::SIGPIPE) => b"PIPE".to_vec(),
        TrapCondition::Signal(sys::constants::SIGALRM) => b"ALRM".to_vec(),
        TrapCondition::Signal(sys::constants::SIGTERM) => b"TERM".to_vec(),
        TrapCondition::Signal(sys::constants::SIGCHLD) => b"CHLD".to_vec(),
        TrapCondition::Signal(sys::constants::SIGCONT) => b"CONT".to_vec(),
        TrapCondition::Signal(sys::constants::SIGTRAP) => b"TRAP".to_vec(),
        TrapCondition::Signal(sys::constants::SIGTSTP) => b"TSTP".to_vec(),
        TrapCondition::Signal(sys::constants::SIGTTIN) => b"TTIN".to_vec(),
        TrapCondition::Signal(sys::constants::SIGTTOU) => b"TTOU".to_vec(),
        TrapCondition::Signal(sys::constants::SIGBUS) => b"BUS".to_vec(),
        TrapCondition::Signal(sys::constants::SIGSYS) => b"SYS".to_vec(),
        TrapCondition::Signal(signal) => bstr::i64_to_bytes(signal as i64),
    }
}

pub(super) fn trap_output_action(
    shell: &Shell,
    condition: TrapCondition,
    include_defaults: bool,
    explicit_operand: bool,
) -> Option<Vec<u8>> {
    let action = shell
        .subshell_saved_traps
        .as_ref()
        .and_then(|saved| saved.get(&condition))
        .or_else(|| shell.trap_action(condition));
    match action {
        Some(TrapAction::Ignore) => Some(b"''".to_vec()),
        Some(TrapAction::Command { text, .. }) => Some(shell_quote(text)),
        None if include_defaults || explicit_operand => Some(b"-".to_vec()),
        None => None,
    }
}

pub(super) fn is_unsigned_decimal(text: &[u8]) -> bool {
    !text.is_empty() && text.iter().all(|&ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bstr;
    use crate::bstr::ByteWriter;
    use crate::builtin::test_support::{invoke, test_shell};
    use crate::shell::traps::{TrapAction, TrapCondition};
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn trap_set_and_reset_via_dash() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .trap_actions
                .insert(TrapCondition::Exit, TrapAction::command(b"echo bye"));
            invoke(
                &mut shell,
                &[b"trap".to_vec(), b"-".to_vec(), b"EXIT".to_vec()],
            )
            .expect("trap - EXIT");
            assert!(!shell.trap_actions.contains_key(&TrapCondition::Exit));
        });
    }

    #[test]
    fn trap_set_ignore() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(
                &mut shell,
                &[b"trap".to_vec(), b"".to_vec(), b"EXIT".to_vec()],
            )
            .expect("trap '' EXIT");
            assert!(matches!(
                shell.trap_actions.get(&TrapCondition::Exit),
                Some(TrapAction::Ignore)
            ));
        });
    }

    #[test]
    fn trap_invalid_condition_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: trap: invalid condition: BOGUS\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"trap".to_vec(), b"echo hi".to_vec(), b"BOGUS".to_vec()],
                )
                .expect("trap bogus");
            },
        );
    }

    #[test]
    fn trap_numeric_reset() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .trap_actions
                .insert(TrapCondition::Exit, TrapAction::command(b"echo bye"));
            invoke(&mut shell, &[b"trap".to_vec(), b"0".to_vec()]).expect("trap 0");
            assert!(!shell.trap_actions.contains_key(&TrapCondition::Exit));
        });
    }

    #[test]
    fn trap_dash_dash_no_args_prints() {
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"trap -- 'echo bye' EXIT\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                shell
                    .trap_actions
                    .insert(TrapCondition::Exit, TrapAction::command(b"echo bye"));
                invoke(&mut shell, &[b"trap".to_vec(), b"--".to_vec()]).expect("trap --");
            },
        );
    }

    #[test]
    fn trap_dash_p_specific_condition_prints() {
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"trap -- 'echo done' EXIT\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                shell
                    .trap_actions
                    .insert(TrapCondition::Exit, TrapAction::command(b"echo done"));
                invoke(
                    &mut shell,
                    &[b"trap".to_vec(), b"-p".to_vec(), b"EXIT".to_vec()],
                )
                .expect("trap -p EXIT");
            },
        );
    }

    #[test]
    fn parse_trap_condition_coverage() {
        assert_no_syscalls(|| {
            assert_eq!(parse_trap_condition(b"0"), Some(TrapCondition::Exit));
            assert_eq!(parse_trap_condition(b"EXIT"), Some(TrapCondition::Exit));
            assert_eq!(
                parse_trap_condition(b"SIGHUP"),
                Some(TrapCondition::Signal(sys::constants::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"HUP"),
                Some(TrapCondition::Signal(sys::constants::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"1"),
                Some(TrapCondition::Signal(sys::constants::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"INT"),
                Some(TrapCondition::Signal(sys::constants::SIGINT))
            );
            assert_eq!(
                parse_trap_condition(b"QUIT"),
                Some(TrapCondition::Signal(sys::constants::SIGQUIT))
            );
            assert_eq!(
                parse_trap_condition(b"ILL"),
                Some(TrapCondition::Signal(sys::constants::SIGILL))
            );
            assert_eq!(
                parse_trap_condition(b"ABRT"),
                Some(TrapCondition::Signal(sys::constants::SIGABRT))
            );
            assert_eq!(
                parse_trap_condition(b"FPE"),
                Some(TrapCondition::Signal(sys::constants::SIGFPE))
            );
            assert_eq!(
                parse_trap_condition(b"KILL"),
                Some(TrapCondition::Signal(sys::constants::SIGKILL))
            );
            assert_eq!(
                parse_trap_condition(b"USR1"),
                Some(TrapCondition::Signal(sys::constants::SIGUSR1))
            );
            assert_eq!(
                parse_trap_condition(b"SEGV"),
                Some(TrapCondition::Signal(sys::constants::SIGSEGV))
            );
            assert_eq!(
                parse_trap_condition(b"USR2"),
                Some(TrapCondition::Signal(sys::constants::SIGUSR2))
            );
            assert_eq!(
                parse_trap_condition(b"PIPE"),
                Some(TrapCondition::Signal(sys::constants::SIGPIPE))
            );
            assert_eq!(
                parse_trap_condition(b"ALRM"),
                Some(TrapCondition::Signal(sys::constants::SIGALRM))
            );
            assert_eq!(
                parse_trap_condition(b"TERM"),
                Some(TrapCondition::Signal(sys::constants::SIGTERM))
            );
            assert_eq!(
                parse_trap_condition(b"CHLD"),
                Some(TrapCondition::Signal(sys::constants::SIGCHLD))
            );
            assert_eq!(
                parse_trap_condition(b"CONT"),
                Some(TrapCondition::Signal(sys::constants::SIGCONT))
            );
            assert_eq!(
                parse_trap_condition(b"STOP"),
                Some(TrapCondition::Signal(sys::constants::SIGSTOP))
            );
            assert_eq!(
                parse_trap_condition(b"TSTP"),
                Some(TrapCondition::Signal(sys::constants::SIGTSTP))
            );
            assert_eq!(
                parse_trap_condition(b"TTIN"),
                Some(TrapCondition::Signal(sys::constants::SIGTTIN))
            );
            assert_eq!(
                parse_trap_condition(b"TTOU"),
                Some(TrapCondition::Signal(sys::constants::SIGTTOU))
            );
            assert_eq!(
                parse_trap_condition(b"BUS"),
                Some(TrapCondition::Signal(sys::constants::SIGBUS))
            );
            assert_eq!(
                parse_trap_condition(b"SYS"),
                Some(TrapCondition::Signal(sys::constants::SIGSYS))
            );
            assert_eq!(
                parse_trap_condition(b"TRAP"),
                Some(TrapCondition::Signal(sys::constants::SIGTRAP))
            );
            assert_eq!(parse_trap_condition(b"BOGUS"), None);
        });
    }

    #[test]
    fn format_trap_condition_coverage() {
        assert_no_syscalls(|| {
            assert_eq!(format_trap_condition(TrapCondition::Exit), b"EXIT");
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGHUP)),
                b"HUP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGINT)),
                b"INT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGQUIT)),
                b"QUIT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGILL)),
                b"ILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGABRT)),
                b"ABRT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGFPE)),
                b"FPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGKILL)),
                b"KILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGUSR1)),
                b"USR1"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGSEGV)),
                b"SEGV"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGUSR2)),
                b"USR2"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGPIPE)),
                b"PIPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGALRM)),
                b"ALRM"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGTERM)),
                b"TERM"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGCHLD)),
                b"CHLD"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGCONT)),
                b"CONT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGTRAP)),
                b"TRAP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGTSTP)),
                b"TSTP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGTTIN)),
                b"TTIN"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGTTOU)),
                b"TTOU"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGBUS)),
                b"BUS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGSYS)),
                b"SYS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::constants::SIGSTOP)),
                bstr::i64_to_bytes(sys::constants::SIGSTOP as i64)
            );
        });
    }

    #[test]
    fn parse_trap_action_variants() {
        assert_no_syscalls(|| {
            assert!(parse_trap_action(b"-").is_none());
            assert!(matches!(parse_trap_action(b""), Some(TrapAction::Ignore)));
            match parse_trap_action(b"echo hi") {
                Some(TrapAction::Command { text, .. }) => {
                    assert_eq!(&*text, b"echo hi");
                }
                other => panic!("expected Command, got {:?}", other),
            }
            match parse_trap_action(b"if ;") {
                Some(TrapAction::Command { text, program }) => {
                    assert_eq!(&*text, b"if ;");
                    assert!(program.items.is_empty());
                }
                other => panic!("expected Command with empty program, got {:?}", other),
            }
        });
    }

    #[test]
    fn is_unsigned_decimal_helper() {
        assert_no_syscalls(|| {
            assert!(is_unsigned_decimal(b"0"));
            assert!(is_unsigned_decimal(b"123"));
            assert!(!is_unsigned_decimal(b""));
            assert!(!is_unsigned_decimal(b"abc"));
            assert!(!is_unsigned_decimal(b"-1"));
        });
    }

    #[test]
    fn trap_dash_dash_with_action_and_condition() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(
                &mut shell,
                &[
                    b"trap".to_vec(),
                    b"--".to_vec(),
                    b"echo goodbye".to_vec(),
                    b"EXIT".to_vec(),
                ],
            )
            .expect("trap -- 'echo goodbye' EXIT");
            match shell.trap_actions.get(&TrapCondition::Exit) {
                Some(TrapAction::Command { text, .. }) => {
                    assert_eq!(&**text, b"echo goodbye".as_slice());
                }
                other => panic!("expected Command, got {:?}", other),
            }
        });
    }

    #[test]
    fn trap_numeric_reset_invalid_condition() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: trap: invalid condition: BOGUS\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                let result = invoke(
                    &mut shell,
                    &[b"trap".to_vec(), b"0".to_vec(), b"BOGUS".to_vec()],
                )
                .expect("trap 0 BOGUS");
                assert!(matches!(result, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn trap_action_without_condition_errors() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: trap: condition argument required\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                let result = invoke(&mut shell, &[b"trap".to_vec(), b"echo hi".to_vec()])
                    .expect("trap echo_hi");
                assert!(matches!(result, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn trap_dash_p_no_operands_lists_all() {
        let signals = supported_trap_conditions();
        let mut writes = Vec::new();
        for condition in &signals {
            let line = ByteWriter::new()
                .bytes(b"trap -- - ")
                .bytes(&format_trap_condition(*condition))
                .byte(b'\n')
                .finish();
            writes.push(crate::sys::test_support::t(
                "write",
                vec![
                    crate::sys::test_support::ArgMatcher::Fd(1),
                    crate::sys::test_support::ArgMatcher::Bytes(line),
                ],
                crate::sys::test_support::TraceResult::Auto,
            ));
        }
        run_trace(trace_entries![..writes], || {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"trap".to_vec(), b"-p".to_vec()]).expect("trap -p");
        });
    }

    #[test]
    fn trap_dash_p_invalid_condition_errors() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: trap: invalid condition: NOPE\n")) -> auto
            ],
            || {
                let mut shell = test_shell();
                let result = invoke(
                    &mut shell,
                    &[b"trap".to_vec(), b"-p".to_vec(), b"NOPE".to_vec()],
                )
                .expect("trap -p NOPE");
                assert!(matches!(result, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn trap_output_action_variants() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .trap_actions
                .insert(TrapCondition::Exit, TrapAction::Ignore);
            let action = trap_output_action(&shell, TrapCondition::Exit, false, false);
            assert_eq!(action, Some(b"''".to_vec()));

            shell
                .trap_actions
                .insert(TrapCondition::Exit, TrapAction::command(b"echo bye"));
            let action = trap_output_action(&shell, TrapCondition::Exit, false, false);
            assert_eq!(action, Some(b"'echo bye'".to_vec()));

            shell.trap_actions.remove(&TrapCondition::Exit);
            let action = trap_output_action(&shell, TrapCondition::Exit, true, false);
            assert_eq!(action, Some(b"-".to_vec()));

            let action = trap_output_action(&shell, TrapCondition::Exit, false, true);
            assert_eq!(action, Some(b"-".to_vec()));

            let action = trap_output_action(&shell, TrapCondition::Exit, false, false);
            assert!(action.is_none());
        });
    }
}
