use super::alias::shell_quote;
use super::{BuiltinOutcome, var_error_msg, write_stdout_line};
use crate::bstr::BStrExt;
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

pub(super) fn export(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, b"export", argv)?;
    if print || index == argv.len() {
        for line in exported_lines(shell) {
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    // Assignment-context expansion (including tilde-prefix handling)
    // is performed by the parser/expander at AST-build and word-expand
    // time; the builtin receives the already-expanded value verbatim.
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            shell.export_var(name, Some(value))?;
        } else {
            shell.export_var(item, None)?;
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn readonly(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (print, index) = parse_declaration_listing_flag(shell, b"readonly", argv)?;
    if print || index == argv.len() {
        for line in readonly_lines(shell) {
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    for item in &argv[index..] {
        if let Some((name, value)) = item.split_once_byte(b'=') {
            shell
                .set_var(name, value)
                .map_err(|e| shell.diagnostic(1, &var_error_msg(b"readonly", &e)))?;
            shell.mark_readonly(name);
        } else {
            shell.mark_readonly(item);
        }
    }
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn unset(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (target, index) = parse_unset_target(shell, argv)?;
    let mut status = 0;
    for item in &argv[index..] {
        match target {
            UnsetTarget::Variable => {
                if let Err(error) = shell.unset_var(item) {
                    shell.diagnostic(1, &var_error_msg(b"unset", &error));
                    status = 1;
                }
            }
            UnsetTarget::Function => {
                shell.unset_function(item.as_slice());
            }
        }
    }
    if status != 0 {
        Ok(BuiltinOutcome::UtilityError(status))
    } else {
        Ok(BuiltinOutcome::Status(status))
    }
}

pub(super) fn parse_declaration_listing_flag(
    shell: &Shell,
    name: &[u8],
    argv: &[Vec<u8>],
) -> Result<(bool, usize), ShellError> {
    if argv.len() >= 2 && argv[1] == b"-p" {
        if argv.len() > 2 {
            let msg = ByteWriter::new()
                .bytes(name)
                .bytes(b": -p does not accept operands")
                .finish();
            return Err(shell.diagnostic(1, &msg));
        }
        return Ok((true, 2));
    }
    if let Some(arg) = argv.get(1)
        && arg.first() == Some(&b'-')
        && arg != b"-"
        && arg != b"--"
    {
        let msg = ByteWriter::new()
            .bytes(name)
            .bytes(b": invalid option: ")
            .bytes(arg)
            .finish();
        return Err(shell.diagnostic(1, &msg));
    }
    Ok((false, 1))
}

pub(super) fn exported_lines(shell: &Shell) -> Vec<Vec<u8>> {
    shell
        .exported()
        .iter()
        .map(|name| declaration_line(b"export", name, shell.get_var(name)))
        .collect()
}

pub(super) fn readonly_lines(shell: &Shell) -> Vec<Vec<u8>> {
    shell
        .readonly()
        .iter()
        .map(|name| declaration_line(b"readonly", name, shell.get_var(name)))
        .collect()
}

pub(super) fn declaration_line(prefix: &[u8], name: &[u8], value: Option<&[u8]>) -> Vec<u8> {
    match value {
        Some(value) => {
            let mut out = prefix.to_vec();
            out.push(b' ');
            out.extend_from_slice(name);
            out.push(b'=');
            out.extend_from_slice(&shell_quote(value));
            out
        }
        None => {
            let mut out = prefix.to_vec();
            out.push(b' ');
            out.extend_from_slice(name);
            out
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnsetTarget {
    Variable,
    Function,
}

pub(super) fn parse_unset_target(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<(UnsetTarget, usize), ShellError> {
    let mut target = UnsetTarget::Variable;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        if arg.first() != Some(&b'-') || arg == b"-" {
            break;
        }
        if arg == b"--" {
            index += 1;
            break;
        }
        for &ch in &arg[1..] {
            match ch {
                b'v' => target = UnsetTarget::Variable,
                b'f' => target = UnsetTarget::Function,
                _ => {
                    let msg = ByteWriter::new()
                        .bytes(b"unset: invalid option: -")
                        .byte(ch)
                        .finish();
                    return Err(shell.diagnostic(1, &msg));
                }
            }
        }
        index += 1;
    }
    Ok((target, index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn export_updates_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"export".to_vec(), b"NAME=value".to_vec()]).expect("export");
            assert_eq!(shell.get_var(b"NAME"), Some(b"value" as &[u8]));
            assert!(shell.exported().contains(b"NAME" as &[u8]));
        });
    }

    #[test]
    fn unset_removes_variable_and_export_flag() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(&mut shell, &[b"export".to_vec(), b"NAME=value".to_vec()]).expect("export");

            invoke(&mut shell, &[b"unset".to_vec(), b"NAME".to_vec()]).expect("unset");
            assert_eq!(shell.get_var(b"NAME"), None);
            assert!(!shell.exported().contains(b"NAME" as &[u8]));
        });
    }

    #[test]
    fn readonly_marks_variable_readonly() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            invoke(
                &mut shell,
                &[b"readonly".to_vec(), b"LOCKED=value".to_vec()],
            )
            .expect("readonly");
            assert!(shell.readonly().contains(b"LOCKED" as &[u8]));
        });
    }

    #[test]
    fn export_preserves_literal_tilde_in_value() {
        // Assignment-context tilde expansion is done by the parser; the
        // builtin stores its operand verbatim. This test guards against
        // the legacy double-expansion bug where the builtin re-expanded
        // a tilde byte that had been preserved via quoting or escape.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"HOME".to_vec(), b"/home/user".to_vec());
            invoke(&mut shell, &[b"export".to_vec(), b"FOO=~/bin".to_vec()]).expect("export tilde");
            assert_eq!(shell.get_var(b"FOO"), Some(b"~/bin" as &[u8]));
        });
    }

    #[test]
    fn readonly_preserves_literal_tilde_in_value() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"HOME".to_vec(), b"/home/user".to_vec());
            invoke(&mut shell, &[b"readonly".to_vec(), b"FOO=~/bin".to_vec()])
                .expect("readonly tilde");
            assert_eq!(shell.get_var(b"FOO"), Some(b"~/bin" as &[u8]));
        });
    }

    #[test]
    fn unset_readonly_var_error() {
        let msg = diag(b"unset: readonly variable: RO");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                shell.set_var(b"RO", b"val").unwrap();
                shell.mark_readonly(b"RO");
                let outcome =
                    invoke(&mut shell, &[b"unset".to_vec(), b"RO".to_vec()]).expect("unset RO");
                assert!(matches!(outcome, BuiltinOutcome::UtilityError(1)));
            },
        );
    }

    #[test]
    fn export_dash_p_with_operands_errors() {
        let msg = diag(b"export: -p does not accept operands");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let error = invoke(
                    &mut shell,
                    &[b"export".to_vec(), b"-p".to_vec(), b"FOO".to_vec()],
                )
                .expect_err("export -p with operands");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn export_invalid_option_errors() {
        let msg = diag(b"export: invalid option: -z");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &[b"export".to_vec(), b"-z".to_vec()])
                    .expect_err("export -z");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn readonly_invalid_option_errors() {
        let msg = diag(b"readonly: invalid option: -x");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let error = invoke(&mut shell, &[b"readonly".to_vec(), b"-x".to_vec()])
                    .expect_err("readonly -x");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn unset_double_dash_stops_option_parsing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.set_var(b"-v", b"val").unwrap();
            let outcome = invoke(
                &mut shell,
                &[b"unset".to_vec(), b"--".to_vec(), b"-v".to_vec()],
            )
            .expect("unset --");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert_eq!(shell.get_var(b"-v"), None);
        });
    }

    #[test]
    fn unset_invalid_option_errors() {
        let msg = diag(b"unset: invalid option: -z");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let error =
                    invoke(&mut shell, &[b"unset".to_vec(), b"-z".to_vec()]).expect_err("unset -z");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }
}
