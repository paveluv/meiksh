use super::*;

pub(super) fn exit(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let status = match argv.get(1) {
        Some(value) => parse_i32(value)
            .ok_or_else(|| shell.diagnostic(2, b"exit: numeric argument required"))?,
        None => shell.last_status,
    };
    Ok(BuiltinOutcome::Exit(status))
}
