use super::{BuiltinOutcome, parse_i32, parse_usize};
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

pub(super) fn return_builtin(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<BuiltinOutcome, ShellError> {
    if shell.function_depth == 0 && shell.source_depth == 0 {
        return Err(shell.diagnostic(1, b"return: not in a function"));
    }
    if argv.len() > 2 {
        return Err(shell.diagnostic(1, b"return: too many arguments"));
    }
    let status = argv
        .get(1)
        .map(|value| parse_i32(value).ok_or(()))
        .transpose()
        .map_err(|_| shell.diagnostic(1, b"return: numeric argument required"))?
        .unwrap_or(shell.last_status);
    Ok(BuiltinOutcome::Return(status))
}

pub(super) fn break_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, b"break: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, b"break", argv)?;
    Ok(BuiltinOutcome::Break(levels.min(shell.loop_depth)))
}

pub(super) fn continue_builtin(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<BuiltinOutcome, ShellError> {
    if shell.loop_depth == 0 {
        return Err(shell.diagnostic(1, b"continue: only meaningful in a loop"));
    }
    let levels = parse_loop_count(shell, b"continue", argv)?;
    Ok(BuiltinOutcome::Continue(levels.min(shell.loop_depth)))
}

pub(super) fn parse_loop_count(
    shell: &Shell,
    name: &[u8],
    argv: &[Vec<u8>],
) -> Result<usize, ShellError> {
    if argv.len() > 2 {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": too many arguments")
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    let levels = argv
        .get(1)
        .map(|value| parse_usize(value).ok_or(()))
        .transpose()
        .map_err(|_| {
            let msg = ByteWriter::new()
                .bytes(name)
                .bytes(b": numeric argument required")
                .finish();
            shell.diagnostic(1, &msg)
        })?
        .unwrap_or(1);
    if levels == 0 {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": numeric argument required")
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn return_too_many_args() {
        let msg = diag(b"return: too many arguments");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                shell.function_depth = 1;
                let _ = invoke(
                    &mut shell,
                    &[b"return".to_vec(), b"0".to_vec(), b"1".to_vec()],
                );
            },
        );
    }

    #[test]
    fn continue_not_in_loop() {
        let msg = diag(b"continue: only meaningful in a loop");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"continue".to_vec()]);
            },
        );
    }

    #[test]
    fn break_too_many_args() {
        let msg = diag(b"break: too many arguments");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                shell.loop_depth = 1;
                let _ = invoke(
                    &mut shell,
                    &[b"break".to_vec(), b"1".to_vec(), b"2".to_vec()],
                );
            },
        );
    }

    #[test]
    fn break_invalid_number() {
        let msg = diag(b"break: numeric argument required");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                shell.loop_depth = 1;
                let _ = invoke(&mut shell, &[b"break".to_vec(), b"abc".to_vec()]);
            },
        );
    }

    #[test]
    fn break_zero_level() {
        let msg = diag(b"break: numeric argument required");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                shell.loop_depth = 1;
                let _ = invoke(&mut shell, &[b"break".to_vec(), b"0".to_vec()]);
            },
        );
    }
}
