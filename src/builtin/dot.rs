use super::*;

pub(super) fn dot(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let path = argv
        .get(1)
        .ok_or_else(|| shell.diagnostic(2, b".: filename argument required"))?;
    if argv.len() > 2 {
        return Err(shell.diagnostic(2, b".: too many arguments"));
    }
    let resolved = match resolve_dot_path(shell, path) {
        Ok(p) => p,
        Err(_) => {
            let msg = ByteWriter::new()
                .bytes(b".: ")
                .bytes(path)
                .bytes(b": not found")
                .finish();
            return Err(shell.diagnostic(1, &msg));
        }
    };
    let status = shell.source_path(&resolved)?;
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn resolve_dot_path(shell: &Shell, path: &[u8]) -> Result<Vec<u8>, ()> {
    if path.contains_byte(b'/') {
        if readable_regular_file(path) {
            return Ok(path.to_vec());
        }
        return Err(());
    }
    search_path(path, shell, false, readable_regular_file).ok_or(())
}

#[cfg(test)]
mod tests {
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn dot_with_slash_path_not_regular_file() {
        run_trace(
            trace_entries![
                stat(str(b"./somedir"), any) -> stat_dir,
                write(fd(2), bytes(b"meiksh: .: ./somedir: not found\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b".".to_vec(), b"./somedir".to_vec()]);
            },
        );
    }
}
