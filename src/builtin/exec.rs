use super::*;

pub(super) fn exec_builtin(
    shell: &Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    let args = if argv.get(1).map(|s| s.as_slice()) == Some(b"--") {
        &argv[2..]
    } else {
        &argv[1..]
    };
    if args.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }
    if args.iter().any(|s| s.contains_byte(0)) {
        return Err(shell.diagnostic(1, b"exec: invalid argument"));
    }
    let Some(program_path) = which_in_path(&args[0], shell, true) else {
        let msg = ByteWriter::new()
            .bytes(b"exec: ")
            .bytes(&args[0])
            .bytes(b": not found")
            .finish();
        return Err(shell.diagnostic(127, &msg));
    };
    let env = shell.env_for_exec_utility(cmd_assignments);
    sys::exec_replace_with_env(&program_path, &args.to_vec(), &env)
        .map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    Ok(BuiltinOutcome::Status(0))
}

#[cfg(test)]
mod tests {
    use crate::builtin::test_support::*;

    #[test]
    fn exec_nul_byte_arg_error() {
        let msg = diag(b"exec: invalid argument");
        run_trace(vec![trace_write_stderr(&msg)], || {
            let mut shell = test_shell();
            let _ = invoke(&mut shell, &[b"exec".to_vec(), b"foo\x00bar".to_vec()]);
        });
    }

    #[test]
    fn exec_not_found_error() {
        let msg = diag(b"exec: totally_missing: not found");
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
                t(
                    "access",
                    vec![ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Err(libc::ENOENT),
                ),
                trace_write_stderr(&msg),
            ],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"exec".to_vec(), b"totally_missing".to_vec()]);
            },
        );
    }
}
