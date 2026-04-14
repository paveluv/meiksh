use super::*;

pub(super) fn test_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let is_bracket = argv[0] == b"[";
    let args: &[Vec<u8>] = if is_bracket {
        if argv.last().map(|s| s.as_slice()) != Some(b"]") {
            shell.diagnostic(2, b"[: missing ']'");
            return Ok(BuiltinOutcome::Status(2));
        }
        &argv[1..argv.len() - 1]
    } else {
        &argv[1..]
    };
    let result = match args.len() {
        0 => Ok(false),
        1 => Ok(!args[0].is_empty()),
        2 => test_two_args(shell, &args[0], &args[1]),
        3 => test_three_args(shell, &args[0], &args[1], &args[2]),
        4 => {
            if args[0] == b"!" {
                test_three_args(shell, &args[1], &args[2], &args[3]).map(|r| !r)
            } else {
                shell.diagnostic(2, b"test: too many arguments");
                return Ok(BuiltinOutcome::Status(2));
            }
        }
        _ => {
            shell.diagnostic(2, b"test: too many arguments");
            return Ok(BuiltinOutcome::Status(2));
        }
    };
    match result {
        Ok(true) => Ok(BuiltinOutcome::Status(0)),
        Ok(false) => Ok(BuiltinOutcome::Status(1)),
        Err(msg) => {
            let full = ByteWriter::new().bytes(b"test: ").bytes(&msg).finish();
            shell.diagnostic(2, &full);
            Ok(BuiltinOutcome::Status(2))
        }
    }
}

type TestResult = Result<bool, Vec<u8>>;

pub(super) fn test_two_args(shell: &Shell, op: &[u8], operand: &[u8]) -> TestResult {
    if op == b"!" {
        return Ok(operand.is_empty());
    }
    test_unary(shell, op, operand)
}

pub(super) fn test_three_args(_shell: &Shell, left: &[u8], op: &[u8], right: &[u8]) -> TestResult {
    if op == b"=" {
        return Ok(left == right);
    }
    if op == b"!=" {
        return Ok(left != right);
    }
    if op == b">" {
        return Ok(left > right);
    }
    if op == b"<" {
        return Ok(left < right);
    }
    if let Some(r) = test_integer_binary(left, op, right) {
        return r;
    }
    if let Some(r) = test_file_binary(left, op, right) {
        return r;
    }
    if left == b"!" {
        return test_two_args(_shell, op, right).map(|r| !r);
    }
    let mut msg = b"unknown operator: ".to_vec();
    msg.extend_from_slice(op);
    Err(msg)
}

pub(super) fn test_unary(_shell: &Shell, op: &[u8], operand: &[u8]) -> TestResult {
    match op {
        b"-n" => Ok(!operand.is_empty()),
        b"-z" => Ok(operand.is_empty()),
        b"-b" => Ok(sys::stat_path(operand)
            .map(|s| s.is_block_special())
            .unwrap_or(false)),
        b"-c" => Ok(sys::stat_path(operand)
            .map(|s| s.is_char_special())
            .unwrap_or(false)),
        b"-d" => Ok(sys::stat_path(operand).map(|s| s.is_dir()).unwrap_or(false)),
        b"-e" => Ok(sys::stat_path(operand).is_ok()),
        b"-f" => Ok(sys::stat_path(operand)
            .map(|s| s.is_regular_file())
            .unwrap_or(false)),
        b"-g" => Ok(sys::stat_path(operand)
            .map(|s| s.is_setgid())
            .unwrap_or(false)),
        b"-h" | b"-L" => Ok(sys::lstat_path(operand)
            .map(|s| s.is_symlink())
            .unwrap_or(false)),
        b"-p" => Ok(sys::stat_path(operand)
            .map(|s| s.is_fifo())
            .unwrap_or(false)),
        b"-r" => Ok(sys::access_path(operand, libc::R_OK).is_ok()),
        b"-s" => Ok(sys::stat_path(operand).map(|s| s.size > 0).unwrap_or(false)),
        b"-S" => Ok(sys::stat_path(operand)
            .map(|s| s.is_socket())
            .unwrap_or(false)),
        b"-t" => {
            let fd: i32 = bstr::parse_i64(operand)
                .and_then(|v| {
                    if v >= 0 && v <= i32::MAX as i64 {
                        Some(v as i32)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    let mut msg = operand.to_vec();
                    msg.extend_from_slice(b": not a valid fd");
                    msg
                })?;
            Ok(sys::isatty_fd(fd))
        }
        b"-u" => Ok(sys::stat_path(operand)
            .map(|s| s.is_setuid())
            .unwrap_or(false)),
        b"-w" => Ok(sys::access_path(operand, libc::W_OK).is_ok()),
        b"-x" => Ok(sys::access_path(operand, libc::X_OK).is_ok()),
        _ => {
            let mut msg = b"unknown unary operator: ".to_vec();
            msg.extend_from_slice(op);
            Err(msg)
        }
    }
}

pub(super) fn test_integer_binary(left: &[u8], op: &[u8], right: &[u8]) -> Option<TestResult> {
    let cmp = match op {
        b"-eq" | b"-ne" | b"-gt" | b"-ge" | b"-lt" | b"-le" => op,
        _ => return None,
    };
    let l: i64 = match bstr::parse_i64(left) {
        Some(v) => v,
        None => {
            let mut msg = left.to_vec();
            msg.extend_from_slice(b": integer expression expected");
            return Some(Err(msg));
        }
    };
    let r: i64 = match bstr::parse_i64(right) {
        Some(v) => v,
        None => {
            let mut msg = right.to_vec();
            msg.extend_from_slice(b": integer expression expected");
            return Some(Err(msg));
        }
    };
    let result = match cmp {
        b"-eq" => l == r,
        b"-ne" => l != r,
        b"-gt" => l > r,
        b"-ge" => l >= r,
        b"-lt" => l < r,
        b"-le" => l <= r,
        _ => unreachable!(),
    };
    Some(Ok(result))
}

pub(super) fn test_file_binary(left: &[u8], op: &[u8], right: &[u8]) -> Option<TestResult> {
    match op {
        b"-ef" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(a.is_some()
                && b.is_some()
                && a.as_ref().unwrap().same_file(b.as_ref().unwrap())))
        }
        b"-nt" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(match (a, b) {
                (Some(a), Some(b)) => a.newer_than(&b),
                (Some(_), None) => true,
                _ => false,
            }))
        }
        b"-ot" => {
            let a = sys::stat_path(left).ok();
            let b = sys::stat_path(right).ok();
            Some(Ok(match (a, b) {
                (Some(a), Some(b)) => b.newer_than(&a),
                (None, Some(_)) => true,
                _ => false,
            }))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// echo builtin
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;

    #[test]
    fn test_string_less_than_operator() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_three_args(&shell, b"abc", b"<", b"def");
            assert_eq!(result, Ok(true));

            let result = test_three_args(&shell, b"def", b"<", b"abc");
            assert_eq!(result, Ok(false));
        });
    }

    #[test]
    fn test_string_greater_than_operator() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_three_args(&shell, b"def", b">", b"abc");
            assert_eq!(result, Ok(true));
        });
    }

    #[test]
    fn test_unknown_unary_operator() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_unary(&shell, b"-Q", b"whatever");
            assert!(result.is_err());
            let msg = result.unwrap_err();
            assert!(msg.starts_with(b"unknown unary operator: "));
        });
    }

    #[test]
    fn test_ef_same_file() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/file1".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/file2".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let result = test_file_binary(b"/file1", b"-ef", b"/file2");
                assert!(result.is_some());
                let val = result.unwrap().unwrap();
                assert!(val);
            },
        );
    }

    #[test]
    fn test_ef_different_files() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/a".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/b".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o755),
                ),
            ],
            || {
                let result = test_file_binary(b"/a", b"-ef", b"/b");
                assert!(result.is_some());
            },
        );
    }

    #[test]
    fn test_nt_newer_than() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/new".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/old".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let result = test_file_binary(b"/new", b"-nt", b"/old");
                assert!(result.is_some());
            },
        );
    }

    #[test]
    fn test_nt_first_exists_second_not() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/exists".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/gone".to_vec()), ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
            ],
            || {
                let result = test_file_binary(b"/exists", b"-nt", b"/gone");
                assert_eq!(result, Some(Ok(true)));
            },
        );
    }

    #[test]
    fn test_ot_older_than() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/old".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/new".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let result = test_file_binary(b"/old", b"-ot", b"/new");
                assert!(result.is_some());
            },
        );
    }

    #[test]
    fn test_ot_first_missing_second_exists() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/gone".to_vec()), ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"/exists".to_vec()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let result = test_file_binary(b"/gone", b"-ot", b"/exists");
                assert_eq!(result, Some(Ok(true)));
            },
        );
    }

    #[test]
    fn test_socket_file_operator() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Str(b"/sock".to_vec()), ArgMatcher::Any],
                TraceResult::StatFile(0o644),
            )],
            || {
                let shell = test_shell();
                let result = test_unary(&shell, b"-S", b"/sock");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    fn test_unknown_binary_operator() {
        assert_no_syscalls(|| {
            let result = test_file_binary(b"/a", b"-zz", b"/b");
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_integer_binary_operators() {
        assert_no_syscalls(|| {
            assert_eq!(test_integer_binary(b"5", b"-eq", b"5"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"5", b"-ne", b"3"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"5", b"-gt", b"3"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"5", b"-ge", b"5"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"3", b"-lt", b"5"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"5", b"-le", b"5"), Some(Ok(true)));
            assert_eq!(test_integer_binary(b"5", b"=", b"5"), None);
            assert!(test_integer_binary(b"abc", b"-eq", b"5").unwrap().is_err());
            assert!(test_integer_binary(b"5", b"-eq", b"abc").unwrap().is_err());
        });
    }

    #[test]
    fn test_bracket_missing_closing() {
        let msg = diag(b"[: missing ']'");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome =
                invoke(&mut shell, &[b"[".to_vec(), b"-n".to_vec(), b"x".to_vec()]).expect("[");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn test_zero_args() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"test".to_vec()]).expect("test (0 args)");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_four_args_negated() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"!".to_vec(),
                        b"-e".to_vec(),
                        b"/nonexistent_file_xyzzy".to_vec(),
                    ],
                )
                .expect("test ! -e /nonexistent");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn test_four_args_invalid() {
        let msg = diag(b"test: unknown operator: b");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"c".to_vec(),
                ],
            )
            .expect("test a b c");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn test_five_args_too_many() {
        let msg = diag(b"test: too many arguments");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"b".to_vec(),
                    b"c".to_vec(),
                    b"d".to_vec(),
                ],
            )
            .expect("test a b c d");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn test_unknown_operator_error() {
        let msg = diag(b"test: unknown operator: -zz");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"-zz".to_vec(),
                    b"b".to_vec(),
                ],
            )
            .expect("test a -zz b");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn test_unary_setgid() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-g".to_vec(), b"/no".to_vec()],
                )
                .expect("test -g");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_setuid() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-u".to_vec(), b"/no".to_vec()],
                )
                .expect("test -u");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_symlink() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"-h".to_vec(),
                    b"/nonexistent_xyzzy".to_vec(),
                ],
            )
            .expect("test -h");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_unary_fifo() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-p".to_vec(), b"/no".to_vec()],
                )
                .expect("test -p");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_readable() {
        run_trace(
            vec![t(
                "access",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-r".to_vec(), b"/no".to_vec()],
                )
                .expect("test -r");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_writable() {
        run_trace(
            vec![t(
                "access",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-w".to_vec(), b"/no".to_vec()],
                )
                .expect("test -w");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_executable() {
        run_trace(
            vec![t(
                "access",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-x".to_vec(), b"/no".to_vec()],
                )
                .expect("test -x");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_size_nonzero() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-s".to_vec(), b"/no".to_vec()],
                )
                .expect("test -s");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_socket() {
        run_trace(
            vec![t(
                "stat",
                vec![ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT),
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-S".to_vec(), b"/no".to_vec()],
                )
                .expect("test -S");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_tty_bad_fd() {
        run_trace(
            vec![t("isatty", vec![ArgMatcher::Int(999)], TraceResult::Int(0))],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-t".to_vec(), b"999".to_vec()],
                )
                .expect("test -t 999");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn test_unary_tty_invalid_fd() {
        let msg = diag(b"test: abc: not a valid fd");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[b"test".to_vec(), b"-t".to_vec(), b"abc".to_vec()],
            )
            .expect("test -t abc");
            assert!(matches!(outcome, BuiltinOutcome::Status(2)));
        });
    }

    #[test]
    fn test_file_binary_nt_with_missing() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"/etc/passwd".to_vec(),
                        b"-nt".to_vec(),
                        b"/nonexistent".to_vec(),
                    ],
                )
                .expect("test -nt");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn test_file_binary_ot_both_missing() {
        run_trace(
            vec![
                t(
                    "stat",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"/no1".to_vec(),
                        b"-ot".to_vec(),
                        b"/no2".to_vec(),
                    ],
                )
                .expect("test -ot");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}
