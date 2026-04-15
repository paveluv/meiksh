use super::BuiltinOutcome;
use super::command::{readable_regular_file, search_path};
use crate::bstr::{BStrExt, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

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
    use crate::builtin::BuiltinOutcome;
    use crate::builtin::test_support::{invoke, test_shell};
    use crate::sys::test_support::run_trace;
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

    #[test]
    fn dot_too_many_arguments() {
        let msg = crate::builtin::test_support::diag(b".: too many arguments");
        run_trace(trace_entries![write(fd(2), bytes(&msg)) -> auto,], || {
            let mut shell = test_shell();
            let error = invoke(
                &mut shell,
                &[b".".to_vec(), b"a.sh".to_vec(), b"b.sh".to_vec()],
            )
            .expect_err("too many args");
            assert_eq!(error.exit_status(), 2);
        });
    }

    #[test]
    fn dot_with_slash_path_readable_regular_file() {
        run_trace(
            trace_entries![
                stat(str(b"./script.sh"), any) -> stat_file(0o644),
                access(str(b"./script.sh"), _) -> 0,
                open("./script.sh", _, _) -> fd(10),
                read(fd(10), _) -> bytes(b"VAR=hello\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b".".to_vec(), b"./script.sh".to_vec()])
                    .expect("dot slash path");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var(b"VAR"), Some(b"hello" as &[u8]));
            },
        );
    }
}
