use super::BuiltinOutcome;
use crate::bstr;
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

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
    // Compound expressions (XSI: `-a`, `-o`, `(`, `)`) always go through
    // the recursive-descent evaluator. The Issue 7 basic-grammar fast
    // paths (argc 0..=4 without any compound operator) stay on the
    // direct helpers for both performance and message-compatibility with
    // the existing unit tests.
    let has_compound = args.iter().any(|a| {
        let s = a.as_slice();
        s == b"(" || s == b")" || s == b"-a" || s == b"-o"
    });
    let result = if has_compound {
        compound::evaluate_compound(shell, args)
    } else {
        match args.len() {
            0 => Ok(false),
            1 => Ok(!args[0].is_empty()),
            2 => test_two_args(shell, &args[0], &args[1]),
            3 => test_three_args(shell, &args[0], &args[1], &args[2]),
            4 if args[0] == b"!" => {
                test_three_args(shell, &args[1], &args[2], &args[3]).map(|r| !r)
            }
            _ => compound::evaluate_compound(shell, args),
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
        return Ok(crate::sys::locale::strcoll(left, right).is_gt());
    }
    if op == b"<" {
        return Ok(crate::sys::locale::strcoll(left, right).is_lt());
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
        b"-b" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_block_special())
            .unwrap_or(false)),
        b"-c" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_char_special())
            .unwrap_or(false)),
        b"-d" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_dir())
            .unwrap_or(false)),
        b"-e" => Ok(sys::fs::stat_path(operand).is_ok()),
        b"-f" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_regular_file())
            .unwrap_or(false)),
        b"-g" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_setgid())
            .unwrap_or(false)),
        b"-h" | b"-L" => Ok(sys::fs::lstat_path(operand)
            .map(|s| s.is_symlink())
            .unwrap_or(false)),
        b"-p" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_fifo())
            .unwrap_or(false)),
        b"-r" => Ok(sys::fs::access_path(operand, sys::constants::R_OK).is_ok()),
        b"-s" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.size > 0)
            .unwrap_or(false)),
        b"-S" => Ok(sys::fs::stat_path(operand)
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
            Ok(sys::tty::isatty_fd(fd))
        }
        b"-u" => Ok(sys::fs::stat_path(operand)
            .map(|s| s.is_setuid())
            .unwrap_or(false)),
        b"-w" => Ok(sys::fs::access_path(operand, sys::constants::W_OK).is_ok()),
        b"-x" => Ok(sys::fs::access_path(operand, sys::constants::X_OK).is_ok()),
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
        _ => l <= r,
    };
    Some(Ok(result))
}

pub(super) fn test_file_binary(left: &[u8], op: &[u8], right: &[u8]) -> Option<TestResult> {
    match op {
        b"-ef" => {
            let a = sys::fs::stat_path(left).ok();
            let b = sys::fs::stat_path(right).ok();
            Some(Ok(a.is_some()
                && b.is_some()
                && a.as_ref().unwrap().same_file(b.as_ref().unwrap())))
        }
        b"-nt" => {
            let a = sys::fs::stat_path(left).ok();
            let b = sys::fs::stat_path(right).ok();
            Some(Ok(match (a, b) {
                (Some(a), Some(b)) => a.newer_than(&b),
                (Some(_), None) => true,
                _ => false,
            }))
        }
        b"-ot" => {
            let a = sys::fs::stat_path(left).ok();
            let b = sys::fs::stat_path(right).ok();
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
// Compound expression parser (POSIX Issue 7 XSI semantics).
//
// Implements `-a`, `-o`, `!`, `(`, `)` exactly per the Issue 7
// `test`/`[` RATIONALE
// (<https://pubs.opengroup.org/onlinepubs/9699919799/utilities/test.html>).
// Issue 8 removes the combinators from normative text but explicitly
// permits implementation-defined operators of the form `-operator` and
// classes >4-arg behavior as "unspecified", so providing Issue 7 XSI
// semantics is conformant with Issue 8 while preserving compatibility
// with the large body of Issue 7-era scripts (including Debian's
// `/etc/profile.d/*.sh`).
//
// The six precedence rules, verbatim from Issue 7:
//
//   1. The unary primaries have higher precedence than the algebraic
//      binary primaries.
//   2. The unary primaries have lower precedence than the string binary
//      primaries.
//   3. The unary and binary primaries have higher precedence than the
//      unary string primary.
//   4. The `!` operator has higher precedence than the `-a` operator,
//      and the `-a` operator has higher precedence than the `-o`
//      operator.
//   5. The `-a` and `-o` operators are left associative.
//   6. The parentheses can be used to alter the normal precedence and
//      associativity.
//
// Rules 1-3 describe the existing argc 0..=4 "basic grammar" dispatch
// (via the `test_one_arg`-in-line / `test_two_args` / `test_three_args`
// helpers). The parser here implements rules 4-6 on top of those
// primitives: rule 4 fixes the outer-level precedence (`or_expr` /
// `and_expr` / `not_expr` / `primary`); rule 5 is realized by the
// iterative `( ... )*` form in `or_expr` and `and_expr`; rule 6 by the
// `(` / `)` arm of `primary`.
//
// Issue 7 is silent on short-circuit evaluation. We choose short-circuit
// — `-o` skips its RHS when the LHS is true, `-a` skips its RHS when the
// LHS is false — because it is the only choice that gives deterministic
// minimal syscall counts for file primaries (`-r`, `-w`, `-x`, `-e`,
// `-f`, …). Both eager and short-circuit evaluation are Issue-7
// conformant; the choice is locked in by the unit tests.
// ---------------------------------------------------------------------------

mod compound {
    use super::{Shell, TestResult, test_three_args, test_two_args};

    struct Parser<'a> {
        args: &'a [Vec<u8>],
        pos: usize,
    }

    pub(super) fn evaluate_compound(shell: &Shell, args: &[Vec<u8>]) -> TestResult {
        let mut parser = Parser { args, pos: 0 };
        let result = parser.parse_or(shell, false)?;
        if parser.pos != args.len() {
            let tok = &args[parser.pos];
            if tok.as_slice() == b")" {
                return Err(b"syntax error: unexpected ')'".to_vec());
            }
            let mut msg = b"syntax error: unexpected token '".to_vec();
            msg.extend_from_slice(tok);
            msg.push(b'\'');
            return Err(msg);
        }
        Ok(result)
    }

    impl<'a> Parser<'a> {
        fn peek(&self) -> Option<&'a [u8]> {
            self.args.get(self.pos).map(|v| v.as_slice())
        }

        fn is_compound_op(tok: &[u8]) -> bool {
            tok == b"(" || tok == b")" || tok == b"-a" || tok == b"-o"
        }

        fn can_start_primary(tok: Option<&[u8]>) -> bool {
            match tok {
                None => false,
                Some(t) => t != b"-a" && t != b"-o" && t != b")",
            }
        }

        fn parse_or(&mut self, shell: &Shell, skip: bool) -> TestResult {
            let mut left = self.parse_and(shell, skip)?;
            while self.peek() == Some(b"-o") {
                self.pos += 1;
                if !Self::can_start_primary(self.peek()) {
                    return Err(b"syntax error: expected operand after -o".to_vec());
                }
                let right_skip = skip || left;
                let right = self.parse_and(shell, right_skip)?;
                if !skip && !left {
                    left = right;
                }
            }
            Ok(left)
        }

        fn parse_and(&mut self, shell: &Shell, skip: bool) -> TestResult {
            let mut left = self.parse_not(shell, skip)?;
            while self.peek() == Some(b"-a") {
                self.pos += 1;
                if !Self::can_start_primary(self.peek()) {
                    return Err(b"syntax error: expected operand after -a".to_vec());
                }
                let right_skip = skip || !left;
                let right = self.parse_not(shell, right_skip)?;
                if !skip && left {
                    left = right;
                }
            }
            Ok(left)
        }

        fn parse_not(&mut self, shell: &Shell, skip: bool) -> TestResult {
            if self.peek() == Some(b"!") {
                self.pos += 1;
                let inner = self.parse_not(shell, skip)?;
                return Ok(!inner);
            }
            self.parse_primary(shell, skip)
        }

        fn parse_primary(&mut self, shell: &Shell, skip: bool) -> TestResult {
            if self.peek() == Some(b"(") {
                self.pos += 1;
                let inner = self.parse_or(shell, skip)?;
                if self.peek() != Some(b")") {
                    return Err(b"syntax error: missing ')'".to_vec());
                }
                self.pos += 1;
                return Ok(inner);
            }
            // Collect a run of up to 3 non-compound tokens and hand them
            // to the Issue 7 basic-grammar dispatcher (rules 1-3).
            let start = self.pos;
            let mut end = start;
            while end < self.args.len() && end - start < 3 {
                if Self::is_compound_op(&self.args[end]) {
                    break;
                }
                end += 1;
            }
            let run_len = end - start;
            if run_len == 0 {
                return match self.peek() {
                    None => Err(b"syntax error: expected expression".to_vec()),
                    Some(t) if t == b")" => Err(b"syntax error: expected expression".to_vec()),
                    Some(t) if t == b"-o" || t == b"-a" => {
                        let mut msg = b"syntax error: expected operand before ".to_vec();
                        msg.extend_from_slice(t);
                        Err(msg)
                    }
                    Some(t) => {
                        let mut msg = b"syntax error: unexpected token '".to_vec();
                        msg.extend_from_slice(t);
                        msg.push(b'\'');
                        Err(msg)
                    }
                };
            }
            self.pos = end;
            if skip {
                return Ok(false);
            }
            match run_len {
                1 => Ok(!self.args[start].is_empty()),
                2 => test_two_args(shell, &self.args[start], &self.args[start + 1]),
                3 => test_three_args(
                    shell,
                    &self.args[start],
                    &self.args[start + 1],
                    &self.args[start + 2],
                ),
                _ => unreachable!(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// echo builtin
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

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
            trace_entries![
                stat(str(b"/file1"), any) -> stat_file(0o644),
                stat(str(b"/file2"), any) -> stat_file(0o644),
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
            trace_entries![
                stat(str(b"/a"), any) -> stat_file(0o644),
                stat(str(b"/b"), any) -> stat_file(0o755),
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
            trace_entries![
                stat(str(b"/new"), any) -> stat_file(0o644),
                stat(str(b"/old"), any) -> stat_file(0o644),
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
            trace_entries![
                stat(str(b"/exists"), any) -> stat_file(0o644),
                stat(str(b"/gone"), any) -> err(sys::constants::ENOENT),
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
            trace_entries![
                stat(str(b"/old"), any) -> stat_file(0o644),
                stat(str(b"/new"), any) -> stat_file(0o644),
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
            trace_entries![
                stat(str(b"/gone"), any) -> err(sys::constants::ENOENT),
                stat(str(b"/exists"), any) -> stat_file(0o644),
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
            trace_entries![stat(str(b"/sock"), any) -> stat_file(0o644),],
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
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"[".to_vec(), b"-n".to_vec(), b"x".to_vec()]).expect("[");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
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
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
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
            },
        );
    }

    #[test]
    fn test_compound_or_true_right() {
        // Issue 7 rule 4: `-o` is lowest-precedence, so `a = a -o b = c`
        // parses as `(a = a) -o (b = c)` = true -o false = true. Pure
        // string comparisons, no syscalls.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b"-o".to_vec(),
                    b"b".to_vec(),
                    b"=".to_vec(),
                    b"c".to_vec(),
                ],
            )
            .expect("test a = a -o b = c");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_unknown_operator_error() {
        let msg = diag(b"test: unknown operator: -zz");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
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
            },
        );
    }

    #[test]
    fn test_unary_setgid() {
        run_trace(
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
        run_trace(
            trace_entries![
                ..vec![crate::sys::test_support::t(
                    "lstat",
                    vec![
                        crate::sys::test_support::ArgMatcher::Str(b"/nonexistent_xyzzy".to_vec()),
                        crate::sys::test_support::ArgMatcher::Any
                    ],
                    crate::sys::test_support::TraceResult::Err(sys::constants::ENOENT),
                )]
            ],
            || {
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
            },
        );
    }

    #[test]
    fn test_unary_fifo() {
        run_trace(
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![access(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![access(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![access(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT),],
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
        run_trace(trace_entries![isatty(int(999)) -> 0,], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[b"test".to_vec(), b"-t".to_vec(), b"999".to_vec()],
            )
            .expect("test -t 999");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_unary_tty_invalid_fd() {
        let msg = diag(b"test: abc: not a valid fd");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-t".to_vec(), b"abc".to_vec()],
                )
                .expect("test -t abc");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_file_binary_nt_with_missing() {
        run_trace(
            trace_entries![
                stat(any, any) -> stat_file(0o644),
                stat(any, any) -> err(sys::constants::ENOENT),
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
            trace_entries![
                stat(any, any) -> err(sys::constants::ENOENT),
                stat(any, any) -> err(sys::constants::ENOENT),
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

    #[test]
    fn test_single_nonempty_arg_is_true() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome =
                invoke(&mut shell, &[b"test".to_vec(), b"hello".to_vec()]).expect("test hello");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_single_empty_arg_is_false() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"test".to_vec(), b"".to_vec()]).expect("test ''");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_four_args_negated_true_becomes_false() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"!".to_vec(),
                    b"abc".to_vec(),
                    b"=".to_vec(),
                    b"abc".to_vec(),
                ],
            )
            .expect("test ! abc = abc");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_and_binds_tighter_than_or() {
        // Issue 7 rule 4: `-a` tighter than `-o`, so
        // `a = a -a b = c -o d = d` parses as `(a=a -a b=c) -o d=d`
        // = (true -a false) -o true = false -o true = true.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b"-a".to_vec(),
                    b"b".to_vec(),
                    b"=".to_vec(),
                    b"c".to_vec(),
                    b"-o".to_vec(),
                    b"d".to_vec(),
                    b"=".to_vec(),
                    b"d".to_vec(),
                ],
            )
            .expect("test a = a -a b = c -o d = d");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_compound_and_false_chain() {
        // `a = b -a c = c` → (false -a true) = false. Locks in that `-a`
        // evaluates both sides at its own precedence level.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                    b"-a".to_vec(),
                    b"c".to_vec(),
                    b"=".to_vec(),
                    b"c".to_vec(),
                ],
            )
            .expect("test a = b -a c = c");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_or_left_associative() {
        // Issue 7 rule 5: `-o` is left-associative, so
        // `a = b -o c = d -o e = e` parses as `((a=b -o c=d) -o e=e)` =
        // `(false -o false) -o true` = true.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                    b"-o".to_vec(),
                    b"c".to_vec(),
                    b"=".to_vec(),
                    b"d".to_vec(),
                    b"-o".to_vec(),
                    b"e".to_vec(),
                    b"=".to_vec(),
                    b"e".to_vec(),
                ],
            )
            .expect("test a = b -o c = d -o e = e");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_compound_parens_override_precedence() {
        // Issue 7 rule 6: parentheses override the default precedence.
        // Without them, `a = b -o a = a -a b = b` would parse as
        // `a=b -o (a=a -a b=b)` = `false -o (true -a true)` = true.
        // With `( a = b -o a = a ) -a b = b` we force the `-o` to bind
        // tighter, yielding `(false -o true) -a true` = `true -a true`
        // = true. Same result here is not the point — the point is that
        // the parsing path goes through the `(`/`)` arm of `primary`
        // with a nested `or_expr`.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"(".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                    b"-o".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b")".to_vec(),
                    b"-a".to_vec(),
                    b"b".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                ],
            )
            .expect("test ( a = b -o a = a ) -a b = b");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_compound_not_binds_tighter_than_and() {
        // Issue 7 rule 4: `!` is tightest, so `! a = a -a b = b` parses
        // as `(!(a=a)) -a (b=b)` = `false -a true` = false.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"!".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b"-a".to_vec(),
                    b"b".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                ],
            )
            .expect("test ! a = a -a b = b");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_not_with_parenthesized_inner_or() {
        // `! ( a = a -o a = b )` = `!(true -o false)` = `!true` = false.
        // Forces both the parens arm of `primary` and the `!` arm of
        // `not_expr` on the same parse path.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"!".to_vec(),
                    b"(".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b"-o".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                    b")".to_vec(),
                ],
            )
            .expect("test ! ( a = a -o a = b )");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_vte_regression() {
        // Exactly the shape of `/etc/profile.d/vte-2.91.sh`'s guard with
        // both variables unset: `[ -n "" -o -n "" ]` = false -o false
        // = false. Locks in that the unary-in-compound 5-arg case parses
        // via the 2-arg primary path, not the ill-fated "too many args"
        // diagnostic that existed pre-Fix 2.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"[".to_vec(),
                    b"-n".to_vec(),
                    b"".to_vec(),
                    b"-o".to_vec(),
                    b"-n".to_vec(),
                    b"".to_vec(),
                    b"]".to_vec(),
                ],
            )
            .expect("[ -n '' -o -n '' ]");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_bash_completion_regression() {
        // Exactly the shape of Debian's `/etc/profile.d/bash_completion.sh`
        // guard reduced to literal argv, with every shell-version variable
        // unset (all empty strings). Issue 7 rule 4 precedence is
        // `(z "" -a z "") -o (n "" -a n "")` = `(T -a T) -o (F -a F)` =
        // `T -o F` = T. Hand-derivation from the six precedence rules;
        // no external shell's evaluator is consulted.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"[".to_vec(),
                    b"-z".to_vec(),
                    b"".to_vec(),
                    b"-a".to_vec(),
                    b"-z".to_vec(),
                    b"".to_vec(),
                    b"-o".to_vec(),
                    b"-n".to_vec(),
                    b"".to_vec(),
                    b"-a".to_vec(),
                    b"-n".to_vec(),
                    b"".to_vec(),
                    b"]".to_vec(),
                ],
            )
            .expect("bash_completion shape");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_compound_or_short_circuits_on_true_lhs() {
        // With `-o` short-circuit, the RHS `-r /no/such/path` must not
        // trigger an access() syscall when the LHS `a = a` is already
        // true. Locks in the chosen short-circuit semantics (Issue 7 is
        // silent; both eager and short-circuit are conformant).
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"a".to_vec(),
                    b"-o".to_vec(),
                    b"-r".to_vec(),
                    b"/no/such/path".to_vec(),
                ],
            )
            .expect("test a = a -o -r /no/such/path");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn test_compound_and_short_circuits_on_false_lhs() {
        // With `-a` short-circuit, the RHS `-r /no/such/path` must not
        // trigger an access() syscall when the LHS `a = b` is already
        // false.
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            let outcome = invoke(
                &mut shell,
                &[
                    b"test".to_vec(),
                    b"a".to_vec(),
                    b"=".to_vec(),
                    b"b".to_vec(),
                    b"-a".to_vec(),
                    b"-r".to_vec(),
                    b"/no/such/path".to_vec(),
                ],
            )
            .expect("test a = b -a -r /no/such/path");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
        });
    }

    #[test]
    fn test_compound_unbalanced_open_paren() {
        // `test ( a = b` (argc 4) routes through the compound parser.
        // Inside `primary`'s `(` arm, after the inner `or_expr` returns
        // we expect `)`; finding end-of-input instead is a structural
        // error with the canonical "missing ')'" wording.
        let msg = diag(b"test: syntax error: missing ')'");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"(".to_vec(),
                        b"a".to_vec(),
                        b"=".to_vec(),
                        b"b".to_vec(),
                    ],
                )
                .expect("test ( a = b");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_compound_unexpected_close_paren() {
        // A `)` at the top level after a completed primary has nothing
        // to match; the post-parse leftover-token check fires with the
        // canonical "unexpected ')'" wording.
        let msg = diag(b"test: syntax error: unexpected ')'");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"a".to_vec(),
                        b"=".to_vec(),
                        b"b".to_vec(),
                        b")".to_vec(),
                    ],
                )
                .expect("test a = b )");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_compound_empty_parens() {
        // `test ( )` has an empty primary inside the parens; `primary`
        // collects a zero-length run and diagnoses with "expected
        // expression".
        let msg = diag(b"test: syntax error: expected expression");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"(".to_vec(), b")".to_vec()],
                )
                .expect("test ( )");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_compound_dangling_or_operator() {
        // `test a -o` has an `-o` with no RHS primary; the precondition
        // check in `or_expr` right after consuming `-o` fires.
        let msg = diag(b"test: syntax error: expected operand after -o");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"a".to_vec(), b"-o".to_vec()],
                )
                .expect("test a -o");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_compound_unparseable_still_errors() {
        // `test a b c d e` has no compound operators, so the dispatcher
        // routes it to the compound parser via the fallback arm.
        // `primary` greedily consumes `a b c`, `test_three_args` errors
        // with the existing "unknown operator: b" wording (deterministic
        // "first bad operator wins" style from the Issue 7 basic
        // grammar).
        let msg = diag(b"test: unknown operator: b");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"test".to_vec(),
                        b"a".to_vec(),
                        b"b".to_vec(),
                        b"c".to_vec(),
                        b"d".to_vec(),
                        b"e".to_vec(),
                    ],
                )
                .expect("test a b c d e");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_bang_empty_operand_is_true() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_two_args(&shell, b"!", b"");
            assert_eq!(result, Ok(true));
        });
    }

    #[test]
    fn test_bang_nonempty_operand_is_false() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_two_args(&shell, b"!", b"hello");
            assert_eq!(result, Ok(false));
        });
    }

    #[test]
    fn test_string_not_equal_operator() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_three_args(&shell, b"abc", b"!=", b"def");
            assert_eq!(result, Ok(true));

            let result = test_three_args(&shell, b"abc", b"!=", b"abc");
            assert_eq!(result, Ok(false));
        });
    }

    #[test]
    fn test_unary_z_empty_string() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_unary(&shell, b"-z", b"");
            assert_eq!(result, Ok(true));
        });
    }

    #[test]
    fn test_unary_z_nonempty_string() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let result = test_unary(&shell, b"-z", b"x");
            assert_eq!(result, Ok(false));
        });
    }

    #[test]
    fn test_unary_block_special() {
        run_trace(
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT)],
            || {
                let shell = test_shell();
                let result = test_unary(&shell, b"-b", b"/dev/sda");
                assert_eq!(result, Ok(false));
            },
        );
    }

    #[test]
    fn test_unary_char_special() {
        run_trace(
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT)],
            || {
                let shell = test_shell();
                let result = test_unary(&shell, b"-c", b"/dev/null");
                assert_eq!(result, Ok(false));
            },
        );
    }

    #[test]
    fn test_unary_directory() {
        run_trace(trace_entries![stat(any, any) -> stat_dir], || {
            let shell = test_shell();
            let result = test_unary(&shell, b"-d", b"/tmp");
            assert_eq!(result, Ok(true));
        });
    }

    #[test]
    fn test_unary_directory_not_found() {
        run_trace(
            trace_entries![stat(any, any) -> err(sys::constants::ENOENT)],
            || {
                let shell = test_shell();
                let result = test_unary(&shell, b"-d", b"/nosuch");
                assert_eq!(result, Ok(false));
            },
        );
    }

    #[test]
    fn test_unary_tty_negative_fd() {
        let msg = diag(b"test: -1: not a valid fd");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"test".to_vec(), b"-t".to_vec(), b"-1".to_vec()],
                )
                .expect("test -t -1");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn test_file_binary_nt_both_missing() {
        run_trace(
            trace_entries![
                stat(any, any) -> err(sys::constants::ENOENT),
                stat(any, any) -> err(sys::constants::ENOENT),
            ],
            || {
                let result = test_file_binary(b"/no1", b"-nt", b"/no2");
                assert_eq!(result, Some(Ok(false)));
            },
        );
    }

    #[test]
    fn test_file_binary_nt_first_missing_second_exists() {
        run_trace(
            trace_entries![
                stat(any, any) -> err(sys::constants::ENOENT),
                stat(any, any) -> stat_file(0o644),
            ],
            || {
                let result = test_file_binary(b"/no", b"-nt", b"/exists");
                assert_eq!(result, Some(Ok(false)));
            },
        );
    }
}
