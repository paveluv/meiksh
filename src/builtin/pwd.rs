use super::*;

pub(super) fn pwd(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut logical = true;
    for arg in &argv[1..] {
        match arg.as_slice() {
            b"-L" => logical = true,
            b"-P" => logical = false,
            _ if arg.first() == Some(&b'-') => {
                let msg = ByteWriter::new()
                    .bytes(b"pwd: invalid option: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
            _ => {
                return Ok(diag_status(shell, 1, b"pwd: too many arguments"));
            }
        }
    }

    let output = pwd_output(shell, logical)?;
    write_stdout_line(&output);
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn pwd_output(shell: &Shell, logical: bool) -> Result<Vec<u8>, ShellError> {
    if logical {
        return current_logical_pwd(shell);
    }
    sys::get_cwd().map_err(|e| shell.diagnostic(1, &e.strerror()))
}

pub(super) fn current_logical_pwd(shell: &Shell) -> Result<Vec<u8>, ShellError> {
    let cwd = sys::get_cwd().map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    if let Some(pwd) = shell.get_var(b"PWD")
        && logical_pwd_is_valid(pwd)
        && paths_match_logically(pwd, &cwd)
    {
        return Ok(pwd.to_vec());
    }
    Ok(cwd)
}

pub(super) fn logical_pwd_is_valid(path: &[u8]) -> bool {
    if path.first() != Some(&b'/') {
        return false;
    }
    for component in path.split(|&b| b == b'/') {
        if component == b"." || component == b".." {
            return false;
        }
    }
    true
}

pub(super) fn paths_match_logically(lhs: &[u8], rhs: &[u8]) -> bool {
    sys::canonicalize(lhs).ok() == sys::canonicalize(rhs).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn pwd_invalid_option() {
        let msg = diag(b"pwd: invalid option: -z");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"pwd".to_vec(), b"-z".to_vec()]).expect("pwd -z");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn pwd_too_many_args() {
        let msg = diag(b"pwd: too many arguments");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"pwd".to_vec(), b"extra".to_vec()]).expect("pwd extra");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn pwd_logical_returns_pwd_when_valid() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/home/user"),
                realpath(any, any) -> realpath("/home/user"),
                realpath(any, any) -> realpath("/home/user"),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"/home/user\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home/user".to_vec());
                let outcome = invoke(&mut shell, &[b"pwd".to_vec()]).expect("pwd");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn logical_pwd_is_valid_rejects_non_absolute() {
        assert_no_syscalls(|| {
            assert!(!logical_pwd_is_valid(b"relative/path"));
        });
    }

    #[test]
    fn logical_pwd_is_valid_rejects_dot_components() {
        assert_no_syscalls(|| {
            assert!(!logical_pwd_is_valid(b"/a/./b"));
            assert!(!logical_pwd_is_valid(b"/a/../b"));
        });
    }

    #[test]
    fn pwd_logical_falls_back_to_cwd_when_paths_differ() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/real/path"),
                realpath(any, any) -> realpath("/home/link"),
                realpath(any, any) -> realpath("/real/path"),
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"/real/path\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home/link".to_vec());
                let outcome = invoke(&mut shell, &[b"pwd".to_vec()]).expect("pwd");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }
}
