use super::{BuiltinOutcome, diag_status, parse_usize, var_error_msg, write_stderr};
use crate::bstr::{self, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

pub(super) fn getopts_set(
    shell: &mut Shell,
    name: &[u8],
    value: &[u8],
) -> Result<(), BuiltinOutcome> {
    shell
        .set_var(name, value)
        .map_err(|e| diag_status(shell, 2, &var_error_msg(b"getopts", &e)))
}

pub(super) fn getopts(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 3 {
        return Ok(diag_status(
            shell,
            2,
            b"getopts: usage: getopts optstring name [arg ...]",
        ));
    }
    let optstring = &argv[1];
    let name = &argv[2];
    let silent = optstring.first() == Some(&b':');
    let opts = if silent {
        &optstring[1..]
    } else {
        optstring.as_slice()
    };

    let params: Vec<Vec<u8>> = if argv.len() > 3 {
        argv[3..].to_vec()
    } else {
        shell.positional.clone()
    };

    let optind: usize = shell
        .get_var(b"OPTIND")
        .and_then(|s| parse_usize(s))
        .unwrap_or(1);

    let charind: usize = shell
        .get_var(b"_GETOPTS_CIND")
        .and_then(|s| parse_usize(s))
        .unwrap_or(0);

    match getopts_inner(shell, name, opts, silent, &params, optind, charind) {
        Ok(status) => Ok(status),
        Err(outcome) => Ok(outcome),
    }
}

pub(super) fn getopts_inner(
    shell: &mut Shell,
    name: &[u8],
    opts: &[u8],
    silent: bool,
    params: &[Vec<u8>],
    optind: usize,
    charind: usize,
) -> Result<BuiltinOutcome, BuiltinOutcome> {
    if optind < 1 || optind > params.len() {
        getopts_set(shell, name, b"?")?;
        let _ = shell.unset_var(b"OPTARG");
        let buf = bstr::u64_to_bytes((params.len() + 1) as u64);
        getopts_set(shell, b"OPTIND", &buf)?;
        return Ok(BuiltinOutcome::Status(1));
    }

    let arg = &params[optind - 1];
    let arg_bytes: &[u8] = arg.as_slice();

    if charind == 0 {
        if arg == b"--" {
            getopts_set(shell, name, b"?")?;
            let _ = shell.unset_var(b"OPTARG");
            let buf = bstr::u64_to_bytes((optind + 1) as u64);
            getopts_set(shell, b"OPTIND", &buf)?;
            shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
            return Ok(BuiltinOutcome::Status(1));
        }
        if arg_bytes.len() < 2 || arg_bytes[0] != b'-' {
            getopts_set(shell, name, b"?")?;
            let _ = shell.unset_var(b"OPTARG");
            let buf = bstr::u64_to_bytes(optind as u64);
            getopts_set(shell, b"OPTIND", &buf)?;
            shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
            return Ok(BuiltinOutcome::Status(1));
        }
    }

    let ci = if charind == 0 { 1 } else { charind };
    let opt_byte = arg_bytes[ci];
    let next_ci = ci + 1;

    if let Some(pos) = opts.iter().position(|&b| b == opt_byte) {
        let takes_arg = opts.get(pos + 1) == Some(&b':');

        if takes_arg {
            if next_ci < arg_bytes.len() {
                getopts_set(shell, b"OPTARG", &arg_bytes[next_ci..])?;
                let buf = bstr::u64_to_bytes((optind + 1) as u64);
                getopts_set(shell, b"OPTIND", &buf)?;
                shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
            } else if optind < params.len() {
                getopts_set(shell, b"OPTARG", &params[optind])?;
                let buf = bstr::u64_to_bytes((optind + 2) as u64);
                getopts_set(shell, b"OPTIND", &buf)?;
                shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
            } else {
                if silent {
                    getopts_set(shell, name, b":")?;
                    getopts_set(shell, b"OPTARG", &[opt_byte])?;
                } else {
                    let msg = ByteWriter::new()
                        .bytes(&shell.shell_name)
                        .bytes(b": option requires an argument -- ")
                        .byte(opt_byte)
                        .byte(b'\n')
                        .finish();
                    write_stderr(&msg);
                    getopts_set(shell, name, b"?")?;
                    let _ = shell.unset_var(b"OPTARG");
                }
                let buf = bstr::u64_to_bytes((optind + 1) as u64);
                getopts_set(shell, b"OPTIND", &buf)?;
                shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
                return Ok(BuiltinOutcome::Status(0));
            }
        } else {
            let _ = shell.unset_var(b"OPTARG");
            if next_ci < arg_bytes.len() {
                shell.env_mut().insert(
                    b"_GETOPTS_CIND".to_vec(),
                    bstr::u64_to_bytes(next_ci as u64),
                );
            } else {
                let buf = bstr::u64_to_bytes((optind + 1) as u64);
                getopts_set(shell, b"OPTIND", &buf)?;
                shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
            }
        }
        getopts_set(shell, name, &[opt_byte])?;
        Ok(BuiltinOutcome::Status(0))
    } else {
        if silent {
            getopts_set(shell, b"OPTARG", &[opt_byte])?;
        } else {
            let msg = ByteWriter::new()
                .bytes(&shell.shell_name)
                .bytes(b": illegal option -- ")
                .byte(opt_byte)
                .byte(b'\n')
                .finish();
            write_stderr(&msg);
            let _ = shell.unset_var(b"OPTARG");
        }
        getopts_set(shell, name, b"?")?;
        if next_ci < arg_bytes.len() {
            shell.env_mut().insert(
                b"_GETOPTS_CIND".to_vec(),
                bstr::u64_to_bytes(next_ci as u64),
            );
        } else {
            let buf = bstr::u64_to_bytes((optind + 1) as u64);
            getopts_set(shell, b"OPTIND", &buf)?;
            shell.env_mut().remove(b"_GETOPTS_CIND" as &[u8]);
        }
        Ok(BuiltinOutcome::Status(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn getopts_too_few_args() {
        let msg = diag(b"getopts: usage: getopts optstring name [arg ...]");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"getopts".to_vec()]).expect("getopts");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }

    #[test]
    fn getopts_optind_past_params() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_mut().insert(b"OPTIND".to_vec(), b"5".to_vec());
            let outcome = invoke(
                &mut shell,
                &[
                    b"getopts".to_vec(),
                    b"ab:".to_vec(),
                    b"opt".to_vec(),
                    b"-a".to_vec(),
                ],
            )
            .expect("getopts past end");
            assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            assert_eq!(shell.get_var(b"opt"), Some(b"?" as &[u8]));
            assert_eq!(shell.get_var(b"OPTIND"), Some(b"2" as &[u8]));
        });
    }

    #[test]
    fn getopts_optind_past_params_readonly_optind() {
        let msg = diag(b"getopts: readonly variable: OPTIND");
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env_mut().insert(b"OPTIND".to_vec(), b"5".to_vec());
                shell.readonly_mut().insert(b"OPTIND".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"getopts".to_vec(),
                        b"ab:".to_vec(),
                        b"opt".to_vec(),
                        b"-a".to_vec(),
                    ],
                )
                .expect("getopts readonly optind");
                assert!(matches!(outcome, BuiltinOutcome::Status(2)));
            },
        );
    }
}
