use super::*;

pub(super) fn echo_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut out: Vec<u8> = Vec::new();
    for (i, arg) in argv[1..].iter().enumerate() {
        if i > 0 {
            out.push(b' ');
        }
        out.extend_from_slice(arg);
    }
    out.push(b'\n');
    if let Err(e) = sys::write_all_fd(sys::STDOUT_FILENO, &out) {
        return Ok(diag_status_syserr(shell, 1, b"echo: write error: ", &e));
    }
    Ok(BuiltinOutcome::Status(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn echo_write_error_returns_nonzero() {
        let msg = diag(b"echo: write error: Bad file descriptor");
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"hello\n")) -> err(libc::EBADF),
                write(fd(2), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"echo".to_vec(), b"hello".to_vec()]).expect("echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}

// ---------------------------------------------------------------------------
// printf builtin
// ---------------------------------------------------------------------------
