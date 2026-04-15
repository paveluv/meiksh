use super::*;

pub(super) fn set(shell: &mut Shell, argv: &[Vec<u8>]) -> BuiltinOutcome {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.env.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            let quoted = shell_quote_if_needed(value);
            let mut line = name.clone();
            line.push(b'=');
            line.extend_from_slice(&quoted);
            write_stdout_line(&line);
        }
    } else {
        let mut index = 1usize;
        while let Some(arg) = argv.get(index) {
            match arg.as_slice() {
                b"-o" | b"+o" => {
                    let reinput = arg == b"+o";
                    if let Some(name) = argv.get(index + 1) {
                        if let Err(e) = shell.options.set_named_option(name, !reinput) {
                            return BuiltinOutcome::UtilityError(
                                shell
                                    .diagnostic(2, &option_error_msg(b"set", &e))
                                    .exit_status(),
                            );
                        }
                        index += 2;
                    } else {
                        for (name, enabled) in shell.options.reportable_options() {
                            if reinput {
                                let prefix = if enabled { b'-' } else { b'+' };
                                let line = ByteWriter::new()
                                    .bytes(b"set ")
                                    .byte(prefix)
                                    .bytes(b"o ")
                                    .bytes(name)
                                    .finish();
                                write_stdout_line(&line);
                            } else {
                                let line = ByteWriter::new()
                                    .bytes(name)
                                    .byte(b' ')
                                    .bytes(if enabled { b"on" } else { b"off" })
                                    .finish();
                                write_stdout_line(&line);
                            }
                        }
                        return BuiltinOutcome::Status(0);
                    }
                }
                b"--" => {
                    shell.set_positional(argv[index + 1..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
                _ if (arg.first() == Some(&b'-') || arg.first() == Some(&b'+'))
                    && arg != b"-"
                    && arg != b"+" =>
                {
                    let enabled = arg.first() == Some(&b'-');
                    for &ch in &arg[1..] {
                        if let Err(e) = shell.options.set_short_option(ch, enabled) {
                            return BuiltinOutcome::UtilityError(
                                shell
                                    .diagnostic(2, &option_error_msg(b"set", &e))
                                    .exit_status(),
                            );
                        }
                    }
                    index += 1;
                }
                _ => {
                    shell.set_positional(argv[index..].to_vec());
                    return BuiltinOutcome::Status(0);
                }
            }
        }
    }
    BuiltinOutcome::Status(0)
}

pub(super) fn shift(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let count = argv
        .get(1)
        .map(|value| parse_usize(value).ok_or(()))
        .transpose()
        .map_err(|_| shell.diagnostic(1, b"shift: numeric argument required"))?
        .unwrap_or(1);
    if count > shell.positional.len() {
        let msg = ByteWriter::new()
            .bytes(b"shift: ")
            .usize_val(count)
            .bytes(b": shift count out of range")
            .finish();
        return Ok(diag_status(shell, 1, &msg));
    }
    shell.positional.drain(0..count);
    Ok(BuiltinOutcome::Status(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn shift_rejects_invalid_arguments() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: shift: 5: shift count out of range\n")) -> auto,
                write(fd(2), bytes(b"meiksh: shift: numeric argument required\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();

                shell.positional = vec![b"a".to_vec()];
                let outcome =
                    invoke(&mut shell, &[b"shift".to_vec(), b"5".to_vec()]).expect("shift");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                let error = invoke(&mut shell, &[b"shift".to_vec(), b"bad".to_vec()])
                    .expect_err("bad shift");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn set_dash_o_named_option() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(
                &mut shell,
                &[b"set".to_vec(), b"-o".to_vec(), b"allexport".to_vec()],
            )
            .expect("set -o allexport");
            assert!(shell.options.allexport);

            invoke(
                &mut shell,
                &[b"set".to_vec(), b"+o".to_vec(), b"allexport".to_vec()],
            )
            .expect("set +o allexport");
            assert!(!shell.options.allexport);
        });
    }

    #[test]
    fn set_dash_o_invalid_name() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: set: invalid option: bogus\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"set".to_vec(), b"-o".to_vec(), b"bogus".to_vec()],
                )
                .expect("set -o bogus");
                assert!(matches!(outcome, BuiltinOutcome::UtilityError(2)));
            },
        );
    }

    #[test]
    fn set_short_option_flags() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"set".to_vec(), b"-x".to_vec()]).expect("set -x");
            assert!(shell.options.xtrace);
            invoke(&mut shell, &[b"set".to_vec(), b"+x".to_vec()]).expect("set +x");
            assert!(!shell.options.xtrace);
        });
    }

    #[test]
    fn shift_count_exceeds_positional_len() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: shift: 5: shift count out of range\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.positional = vec![b"a".to_vec(), b"b".to_vec()];
                let outcome =
                    invoke(&mut shell, &[b"shift".to_vec(), b"5".to_vec()]).expect("shift 5");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn set_no_args_lists_env() {
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"A=1\n")) -> auto,
                write(fd(1), bytes(b"B=2\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"A".to_vec(), b"1".to_vec());
                shell.env.insert(b"B".to_vec(), b"2".to_vec());
                let outcome = invoke(&mut shell, &[b"set".to_vec()]);
                assert!(matches!(outcome, Ok(BuiltinOutcome::Status(0))));
            },
        );
    }

    #[test]
    fn set_minus_o_no_name_lists_options() {
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"allexport off\n")) -> auto,
                write(fd(1), bytes(b"errexit off\n")) -> auto,
                write(fd(1), bytes(b"hashall off\n")) -> auto,
                write(fd(1), bytes(b"monitor off\n")) -> auto,
                write(fd(1), bytes(b"noclobber off\n")) -> auto,
                write(fd(1), bytes(b"noglob off\n")) -> auto,
                write(fd(1), bytes(b"noexec off\n")) -> auto,
                write(fd(1), bytes(b"notify off\n")) -> auto,
                write(fd(1), bytes(b"nounset off\n")) -> auto,
                write(fd(1), bytes(b"pipefail off\n")) -> auto,
                write(fd(1), bytes(b"verbose off\n")) -> auto,
                write(fd(1), bytes(b"xtrace off\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"set".to_vec(), b"-o".to_vec()]);
            },
        );
    }

    #[test]
    fn set_dash_dash_sets_positional() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"set".to_vec(),
                    b"--".to_vec(),
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"c".to_vec(),
                ],
            )
            .expect("set --");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(
                shell.positional,
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]
            );
        });
    }

    #[test]
    fn shift_success_drains_positional() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.positional = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
            let outcome = invoke(&mut shell, &[b"shift".to_vec(), b"2".to_vec()]).expect("shift 2");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(shell.positional, vec![b"c".to_vec()]);
        });
    }

    #[test]
    fn set_plus_o_no_name_lists_reinput() {
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"set +o allexport\n")) -> auto,
                write(fd(1), bytes(b"set +o errexit\n")) -> auto,
                write(fd(1), bytes(b"set +o hashall\n")) -> auto,
                write(fd(1), bytes(b"set +o monitor\n")) -> auto,
                write(fd(1), bytes(b"set +o noclobber\n")) -> auto,
                write(fd(1), bytes(b"set +o noglob\n")) -> auto,
                write(fd(1), bytes(b"set +o noexec\n")) -> auto,
                write(fd(1), bytes(b"set +o notify\n")) -> auto,
                write(fd(1), bytes(b"set +o nounset\n")) -> auto,
                write(fd(1), bytes(b"set +o pipefail\n")) -> auto,
                write(fd(1), bytes(b"set +o verbose\n")) -> auto,
                write(fd(1), bytes(b"set +o xtrace\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"set".to_vec(), b"+o".to_vec()]);
            },
        );
    }
}
