use super::*;

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
        sys::supported_trap_signals()
            .into_iter()
            .map(TrapCondition::Signal),
    );
    conditions
}

pub(super) fn parse_trap_action(action: &[u8]) -> Option<TrapAction> {
    match action {
        b"-" => None,
        _ if action.is_empty() => Some(TrapAction::Ignore),
        _ => Some(TrapAction::Command(action.into())),
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
        b"HUP" | b"1" => Some(TrapCondition::Signal(sys::SIGHUP)),
        b"INT" | b"2" => Some(TrapCondition::Signal(sys::SIGINT)),
        b"QUIT" | b"3" => Some(TrapCondition::Signal(sys::SIGQUIT)),
        b"ILL" | b"4" => Some(TrapCondition::Signal(sys::SIGILL)),
        b"ABRT" | b"6" => Some(TrapCondition::Signal(sys::SIGABRT)),
        b"FPE" | b"8" => Some(TrapCondition::Signal(sys::SIGFPE)),
        b"KILL" | b"9" => Some(TrapCondition::Signal(sys::SIGKILL)),
        b"USR1" | b"10" => Some(TrapCondition::Signal(sys::SIGUSR1)),
        b"SEGV" | b"11" => Some(TrapCondition::Signal(sys::SIGSEGV)),
        b"USR2" | b"12" => Some(TrapCondition::Signal(sys::SIGUSR2)),
        b"PIPE" | b"13" => Some(TrapCondition::Signal(sys::SIGPIPE)),
        b"ALRM" | b"14" => Some(TrapCondition::Signal(sys::SIGALRM)),
        b"TERM" | b"15" => Some(TrapCondition::Signal(sys::SIGTERM)),
        b"CHLD" | b"17" => Some(TrapCondition::Signal(sys::SIGCHLD)),
        b"STOP" | b"19" => Some(TrapCondition::Signal(sys::SIGSTOP)),
        b"CONT" | b"18" => Some(TrapCondition::Signal(sys::SIGCONT)),
        b"TRAP" | b"5" => Some(TrapCondition::Signal(sys::SIGTRAP)),
        b"TSTP" | b"20" => Some(TrapCondition::Signal(sys::SIGTSTP)),
        b"TTIN" | b"21" => Some(TrapCondition::Signal(sys::SIGTTIN)),
        b"TTOU" | b"22" => Some(TrapCondition::Signal(sys::SIGTTOU)),
        b"BUS" => Some(TrapCondition::Signal(sys::SIGBUS)),
        b"SYS" => Some(TrapCondition::Signal(sys::SIGSYS)),
        _ => None,
    }
}

pub(super) fn format_trap_condition(condition: TrapCondition) -> Vec<u8> {
    match condition {
        TrapCondition::Exit => b"EXIT".to_vec(),
        TrapCondition::Signal(sys::SIGHUP) => b"HUP".to_vec(),
        TrapCondition::Signal(sys::SIGINT) => b"INT".to_vec(),
        TrapCondition::Signal(sys::SIGQUIT) => b"QUIT".to_vec(),
        TrapCondition::Signal(sys::SIGILL) => b"ILL".to_vec(),
        TrapCondition::Signal(sys::SIGABRT) => b"ABRT".to_vec(),
        TrapCondition::Signal(sys::SIGFPE) => b"FPE".to_vec(),
        TrapCondition::Signal(sys::SIGKILL) => b"KILL".to_vec(),
        TrapCondition::Signal(sys::SIGUSR1) => b"USR1".to_vec(),
        TrapCondition::Signal(sys::SIGSEGV) => b"SEGV".to_vec(),
        TrapCondition::Signal(sys::SIGUSR2) => b"USR2".to_vec(),
        TrapCondition::Signal(sys::SIGPIPE) => b"PIPE".to_vec(),
        TrapCondition::Signal(sys::SIGALRM) => b"ALRM".to_vec(),
        TrapCondition::Signal(sys::SIGTERM) => b"TERM".to_vec(),
        TrapCondition::Signal(sys::SIGCHLD) => b"CHLD".to_vec(),
        TrapCondition::Signal(sys::SIGCONT) => b"CONT".to_vec(),
        TrapCondition::Signal(sys::SIGTRAP) => b"TRAP".to_vec(),
        TrapCondition::Signal(sys::SIGTSTP) => b"TSTP".to_vec(),
        TrapCondition::Signal(sys::SIGTTIN) => b"TTIN".to_vec(),
        TrapCondition::Signal(sys::SIGTTOU) => b"TTOU".to_vec(),
        TrapCondition::Signal(sys::SIGBUS) => b"BUS".to_vec(),
        TrapCondition::Signal(sys::SIGSYS) => b"SYS".to_vec(),
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
        Some(TrapAction::Command(command)) => Some(shell_quote(command)),
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
    use crate::builtin::test_support::*;

    #[test]
    fn trap_set_and_reset_via_dash() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.trap_actions.insert(
                TrapCondition::Exit,
                TrapAction::Command(b"echo bye"[..].into()),
            );
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
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: trap: invalid condition: BOGUS\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
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
            shell.trap_actions.insert(
                TrapCondition::Exit,
                TrapAction::Command(b"echo bye"[..].into()),
            );
            invoke(&mut shell, &[b"trap".to_vec(), b"0".to_vec()]).expect("trap 0");
            assert!(!shell.trap_actions.contains_key(&TrapCondition::Exit));
        });
    }

    #[test]
    fn trap_dash_dash_no_args_prints() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'echo bye' EXIT\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.trap_actions.insert(
                    TrapCondition::Exit,
                    TrapAction::Command(b"echo bye"[..].into()),
                );
                invoke(&mut shell, &[b"trap".to_vec(), b"--".to_vec()]).expect("trap --");
            },
        );
    }

    #[test]
    fn trap_dash_p_specific_condition_prints() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"trap -- 'echo done' EXIT\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                shell.trap_actions.insert(
                    TrapCondition::Exit,
                    TrapAction::Command(b"echo done"[..].into()),
                );
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
                Some(TrapCondition::Signal(sys::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"HUP"),
                Some(TrapCondition::Signal(sys::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"1"),
                Some(TrapCondition::Signal(sys::SIGHUP))
            );
            assert_eq!(
                parse_trap_condition(b"INT"),
                Some(TrapCondition::Signal(sys::SIGINT))
            );
            assert_eq!(
                parse_trap_condition(b"QUIT"),
                Some(TrapCondition::Signal(sys::SIGQUIT))
            );
            assert_eq!(
                parse_trap_condition(b"ILL"),
                Some(TrapCondition::Signal(sys::SIGILL))
            );
            assert_eq!(
                parse_trap_condition(b"ABRT"),
                Some(TrapCondition::Signal(sys::SIGABRT))
            );
            assert_eq!(
                parse_trap_condition(b"FPE"),
                Some(TrapCondition::Signal(sys::SIGFPE))
            );
            assert_eq!(
                parse_trap_condition(b"KILL"),
                Some(TrapCondition::Signal(sys::SIGKILL))
            );
            assert_eq!(
                parse_trap_condition(b"USR1"),
                Some(TrapCondition::Signal(sys::SIGUSR1))
            );
            assert_eq!(
                parse_trap_condition(b"SEGV"),
                Some(TrapCondition::Signal(sys::SIGSEGV))
            );
            assert_eq!(
                parse_trap_condition(b"USR2"),
                Some(TrapCondition::Signal(sys::SIGUSR2))
            );
            assert_eq!(
                parse_trap_condition(b"PIPE"),
                Some(TrapCondition::Signal(sys::SIGPIPE))
            );
            assert_eq!(
                parse_trap_condition(b"ALRM"),
                Some(TrapCondition::Signal(sys::SIGALRM))
            );
            assert_eq!(
                parse_trap_condition(b"TERM"),
                Some(TrapCondition::Signal(sys::SIGTERM))
            );
            assert_eq!(
                parse_trap_condition(b"CHLD"),
                Some(TrapCondition::Signal(sys::SIGCHLD))
            );
            assert_eq!(
                parse_trap_condition(b"CONT"),
                Some(TrapCondition::Signal(sys::SIGCONT))
            );
            assert_eq!(
                parse_trap_condition(b"STOP"),
                Some(TrapCondition::Signal(sys::SIGSTOP))
            );
            assert_eq!(
                parse_trap_condition(b"TSTP"),
                Some(TrapCondition::Signal(sys::SIGTSTP))
            );
            assert_eq!(
                parse_trap_condition(b"TTIN"),
                Some(TrapCondition::Signal(sys::SIGTTIN))
            );
            assert_eq!(
                parse_trap_condition(b"TTOU"),
                Some(TrapCondition::Signal(sys::SIGTTOU))
            );
            assert_eq!(
                parse_trap_condition(b"BUS"),
                Some(TrapCondition::Signal(sys::SIGBUS))
            );
            assert_eq!(
                parse_trap_condition(b"SYS"),
                Some(TrapCondition::Signal(sys::SIGSYS))
            );
            assert_eq!(
                parse_trap_condition(b"TRAP"),
                Some(TrapCondition::Signal(sys::SIGTRAP))
            );
            assert_eq!(parse_trap_condition(b"BOGUS"), None);
        });
    }

    #[test]
    fn format_trap_condition_coverage() {
        assert_no_syscalls(|| {
            assert_eq!(format_trap_condition(TrapCondition::Exit), b"EXIT");
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGHUP)),
                b"HUP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGINT)),
                b"INT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGQUIT)),
                b"QUIT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGILL)),
                b"ILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGABRT)),
                b"ABRT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGFPE)),
                b"FPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGKILL)),
                b"KILL"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGUSR1)),
                b"USR1"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGSEGV)),
                b"SEGV"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGUSR2)),
                b"USR2"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGPIPE)),
                b"PIPE"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGALRM)),
                b"ALRM"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTERM)),
                b"TERM"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGCHLD)),
                b"CHLD"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGCONT)),
                b"CONT"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTRAP)),
                b"TRAP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTSTP)),
                b"TSTP"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTTIN)),
                b"TTIN"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGTTOU)),
                b"TTOU"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGBUS)),
                b"BUS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGSYS)),
                b"SYS"
            );
            assert_eq!(
                format_trap_condition(TrapCondition::Signal(sys::SIGSTOP)),
                bstr::i64_to_bytes(sys::SIGSTOP as i64)
            );
        });
    }

    #[test]
    fn parse_trap_action_variants() {
        assert_no_syscalls(|| {
            assert!(parse_trap_action(b"-").is_none());
            assert_eq!(parse_trap_action(b""), Some(TrapAction::Ignore));
            assert_eq!(
                parse_trap_action(b"echo hi"),
                Some(TrapAction::Command(b"echo hi"[..].into()))
            );
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
    fn trap_output_action_variants() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .trap_actions
                .insert(TrapCondition::Exit, TrapAction::Ignore);
            let action = trap_output_action(&shell, TrapCondition::Exit, false, false);
            assert_eq!(action, Some(b"''".to_vec()));

            shell.trap_actions.insert(
                TrapCondition::Exit,
                TrapAction::Command(b"echo bye"[..].into()),
            );
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
