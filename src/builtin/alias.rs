use super::*;

pub(super) fn alias(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() == 1 {
        let mut items: Vec<_> = shell.aliases.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in items {
            let line = format_alias_definition(name, value);
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for item in &argv[1..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            shell.aliases.insert(name.into(), value.into());
        } else if let Some(value) = shell.aliases.get(item.as_slice()) {
            let line = format_alias_definition(item, value);
            write_stdout_line(&line);
        } else {
            let msg = ByteWriter::new()
                .bytes(b"alias: ")
                .bytes(item)
                .bytes(b": not found")
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn format_alias_definition(name: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    out.push(b'=');
    out.extend_from_slice(&shell_quote(value));
    out
}

pub(super) fn shell_quote(value: &[u8]) -> Vec<u8> {
    if value.is_empty() {
        return b"''".to_vec();
    }
    let mut out = Vec::new();
    out.push(b'\'');
    for &b in value {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
    out.push(b'\'');
    out
}

pub(super) fn needs_quoting(value: &[u8]) -> bool {
    value.is_empty()
        || value.iter().any(|&b| {
            !b.is_ascii_alphanumeric()
                && b != b'_'
                && b != b'/'
                && b != b'.'
                && b != b'-'
                && b != b'+'
                && b != b':'
                && b != b','
        })
}

pub(super) fn shell_quote_if_needed(value: &[u8]) -> Vec<u8> {
    if needs_quoting(value) {
        shell_quote(value)
    } else {
        value.to_vec()
    }
}

pub(super) fn unalias(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        return Err(shell.diagnostic(1, b"unalias: name required"));
    }
    if argv.len() == 2 && argv[1] == b"-a" {
        shell.aliases.clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv[1].first() == Some(&b'-') && argv[1] != b"-" && argv[1] != b"--" {
        let msg = ByteWriter::new()
            .bytes(b"unalias: invalid option: ")
            .bytes(&argv[1])
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    let start = usize::from(argv[1] == b"--") + 1;
    if start >= argv.len() {
        return Err(shell.diagnostic(1, b"unalias: name required"));
    }
    let mut status = 0;
    for item in &argv[start..] {
        if shell.aliases.remove(item.as_slice()).is_none() {
            let msg = ByteWriter::new()
                .bytes(b"unalias: ")
                .bytes(item)
                .bytes(b": not found")
                .finish();
            shell.diagnostic(1, &msg);
            status = 1;
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn alias_and_unalias_manage_alias_table() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"ll='ls -l'\n")) -> auto,
                write(
                    fd(crate::sys::STDERR_FILENO),
                    bytes(b"meiksh: alias: missing: not found\n"),
                ) -> auto,
                write(
                    fd(crate::sys::STDERR_FILENO),
                    bytes(b"meiksh: unalias: missing: not found\n"),
                ) -> auto,
                write(
                    fd(crate::sys::STDERR_FILENO),
                    bytes(b"meiksh: unalias: name required\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &[b"alias".to_vec(), b"ll=ls -l".to_vec()]).expect("alias");
                invoke(&mut shell, &[b"alias".to_vec(), b"la=ls -a".to_vec()]).expect("alias");
                assert_eq!(
                    shell.aliases.get(b"ll" as &[u8]).map(|s| &**s),
                    Some(b"ls -l" as &[u8])
                );

                let outcome =
                    invoke(&mut shell, &[b"alias".to_vec(), b"ll".to_vec()]).expect("alias query");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));

                let outcome = invoke(&mut shell, &[b"alias".to_vec(), b"missing".to_vec()])
                    .expect("missing alias");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));

                invoke(&mut shell, &[b"unalias".to_vec(), b"ll".to_vec()]).expect("unalias");
                assert!(!shell.aliases.contains_key(b"ll" as &[u8]));
                let outcome = invoke(&mut shell, &[b"unalias".to_vec(), b"missing".to_vec()])
                    .expect("unalias missing");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                let outcome = invoke(&mut shell, &[b"unalias".to_vec(), b"-a".to_vec()])
                    .expect("unalias all");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(shell.aliases.is_empty());

                let error = invoke(&mut shell, &[b"unalias".to_vec()]).expect_err("missing alias");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn alias_output_is_shell_quoted_for_reinput() {
        assert_no_syscalls(|| {
            assert_eq!(format_alias_definition(b"ll", b"ls -l"), b"ll='ls -l'");
            assert_eq!(format_alias_definition(b"sq", b"a'b"), b"sq='a'\\''b'");
            assert_eq!(format_alias_definition(b"empty", b""), b"empty=''");
        });
    }

    #[test]
    fn needs_quoting_and_shell_quote_if_needed_coverage() {
        assert!(!super::needs_quoting(b"simple"));
        assert!(!super::needs_quoting(b"path/to.file-1+2:3,4"));
        assert!(super::needs_quoting(b"has space"));
        assert!(super::needs_quoting(b""));
        assert!(super::needs_quoting(b"quo'te"));

        let result = super::shell_quote_if_needed(b"hello");
        assert_eq!(result, b"hello");

        let result = super::shell_quote_if_needed(b"has space");
        assert_eq!(result, b"'has space'");
    }

    #[test]
    fn alias_define_and_lookup() {
        run_trace(
            trace_entries![write(
                fd(crate::sys::STDOUT_FILENO),
                bytes(b"ll='ls -la'\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                shell.aliases.insert(b"ll"[..].into(), b"ls -la"[..].into());
                let outcome =
                    invoke(&mut shell, &[b"alias".to_vec(), b"ll".to_vec()]).expect("alias ll");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn alias_no_args_lists() {
        run_trace(
            trace_entries![write(
                fd(crate::sys::STDOUT_FILENO),
                bytes(b"ll='ls -la'\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                shell.aliases.insert(b"ll"[..].into(), b"ls -la"[..].into());
                let outcome = invoke(&mut shell, &[b"alias".to_vec()]).expect("alias");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn alias_single_name_lookup_missing() {
        let msg = diag(b"alias: nosuch: not found");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"alias".to_vec(), b"nosuch".to_vec()])
                    .expect("alias nosuch");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn unalias_dash_a() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.aliases.insert(b"ll"[..].into(), b"ls -la"[..].into());
            let outcome =
                invoke(&mut shell, &[b"unalias".to_vec(), b"-a".to_vec()]).expect("unalias -a");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.aliases.is_empty());
        });
    }

    #[test]
    fn unalias_missing_name() {
        let msg = diag(b"unalias: nosuch: not found");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"unalias".to_vec(), b"nosuch".to_vec()])
                    .expect("unalias");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}
