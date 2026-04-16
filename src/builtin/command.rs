use super::alias::{format_alias_definition, shell_quote};
use super::{BuiltinOutcome, diag_status, is_builtin, is_special_builtin, run, write_stdout_line};
use crate::bstr::{BStrExt, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) const DEFAULT_COMMAND_PATH: &[u8] = b"/usr/bin:/bin";

#[cfg(test)]
pub(super) fn which(name: &[u8], shell: &Shell) -> Option<Vec<u8>> {
    which_in_path(name, shell, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CommandMode {
    Execute,
    QueryShort,
    QueryVerbose,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CommandDescription {
    Alias(Vec<u8>),
    Function,
    SpecialBuiltin,
    RegularBuiltin,
    ReservedWord,
    External(Vec<u8>),
}

pub(super) fn parse_command_options(argv: &[Vec<u8>]) -> (bool, CommandMode, usize) {
    let mut use_default_path = false;
    let mut mode = CommandMode::Execute;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"-p" => {
                use_default_path = true;
                index += 1;
            }
            b"-v" => {
                mode = CommandMode::QueryShort;
                index += 1;
            }
            b"-V" => {
                mode = CommandMode::QueryVerbose;
                index += 1;
            }
            b"--" => {
                index += 1;
                break;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => break,
            _ => break,
        }
    }
    (use_default_path, mode, index)
}

pub(super) fn command_usage_status(mode: CommandMode) -> i32 {
    match mode {
        CommandMode::Execute => 127,
        CommandMode::QueryShort | CommandMode::QueryVerbose => 1,
    }
}

pub(super) fn command_short_description(
    shell: &Shell,
    name: &[u8],
    use_default_path: bool,
) -> Option<Vec<u8>> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => {
            let mut out = b"alias ".to_vec();
            out.extend_from_slice(&format_alias_definition(name, &value));
            Some(out)
        }
        CommandDescription::Function
        | CommandDescription::SpecialBuiltin
        | CommandDescription::RegularBuiltin
        | CommandDescription::ReservedWord => Some(name.to_vec()),
        CommandDescription::External(path) => Some(path),
    }
}

pub(super) fn command_verbose_description(
    shell: &Shell,
    name: &[u8],
    use_default_path: bool,
) -> Option<Vec<u8>> {
    match describe_command(shell, name, use_default_path)? {
        CommandDescription::Alias(value) => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is an alias for ");
            out.extend_from_slice(&shell_quote(&value));
            Some(out)
        }
        CommandDescription::Function => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a function");
            Some(out)
        }
        CommandDescription::SpecialBuiltin => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a special built-in utility");
            Some(out)
        }
        CommandDescription::RegularBuiltin => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a regular built-in utility");
            Some(out)
        }
        CommandDescription::ReservedWord => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is a reserved word");
            Some(out)
        }
        CommandDescription::External(path) => {
            let mut out = name.to_vec();
            out.extend_from_slice(b" is ");
            out.extend_from_slice(&path);
            Some(out)
        }
    }
}

pub(super) fn describe_command(
    shell: &Shell,
    name: &[u8],
    use_default_path: bool,
) -> Option<CommandDescription> {
    if let Some(value) = shell.aliases().get(name) {
        return Some(CommandDescription::Alias(value.to_vec()));
    }
    if shell.functions().contains_key(name) {
        return Some(CommandDescription::Function);
    }
    if is_special_builtin(name) {
        return Some(CommandDescription::SpecialBuiltin);
    }
    if is_builtin(name) {
        return Some(CommandDescription::RegularBuiltin);
    }
    if is_reserved_word_name(name) {
        return Some(CommandDescription::ReservedWord);
    }
    which_in_path(name, shell, use_default_path).map(CommandDescription::External)
}

pub(super) fn command(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (use_default_path, mode, index) = parse_command_options(argv);
    let Some(name) = argv.get(index) else {
        return Ok(diag_status(
            shell,
            command_usage_status(mode),
            b"command: utility name required",
        ));
    };

    if mode != CommandMode::Execute && index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, b"command: too many arguments"));
    }

    match mode {
        CommandMode::QueryShort => {
            let Some(line) = command_short_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            write_stdout_line(&line);
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::QueryVerbose => {
            let Some(line) = command_verbose_description(shell, name, use_default_path) else {
                return Ok(BuiltinOutcome::Status(1));
            };
            write_stdout_line(&line);
            Ok(BuiltinOutcome::Status(0))
        }
        CommandMode::Execute => execute_command_utility(shell, &argv[index..], use_default_path),
    }
}

pub(super) fn type_builtin(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut status = 0;
    for name in &argv[1..] {
        match command_verbose_description(shell, name, false) {
            Some(desc) => write_stdout_line(&desc),
            None => {
                let msg = ByteWriter::new().bytes(name).bytes(b": not found").finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn hash(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() >= 2 && argv[1] == b"-r" {
        shell.path_cache_mut().clear();
        return Ok(BuiltinOutcome::Status(0));
    }
    if argv.len() == 1 {
        if shell.path_cache().is_empty() {
            return Ok(BuiltinOutcome::Status(0));
        }
        for (name, path) in shell.path_cache() {
            let line = ByteWriter::new()
                .bytes(name)
                .byte(b'\t')
                .bytes(path)
                .finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    let mut status = 0;
    for name in &argv[1..] {
        if is_builtin(name) || shell.functions().contains_key(name.as_slice()) {
            continue;
        }
        match search_path(name, shell, false, |p| {
            sys::fs::access_path(p, sys::constants::X_OK).is_ok()
        }) {
            Some(path) => {
                shell.path_cache_mut().insert(name.as_slice().into(), path);
            }
            None => {
                let msg = ByteWriter::new()
                    .bytes(b"hash: ")
                    .bytes(name)
                    .bytes(b": not found")
                    .finish();
                shell.diagnostic(1, &msg);
                status = 1;
            }
        }
    }
    Ok(BuiltinOutcome::Status(status))
}

pub(super) fn execute_command_utility(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    use_default_path: bool,
) -> Result<BuiltinOutcome, ShellError> {
    let name = &argv[0];
    if is_builtin(name) {
        return match run(shell, argv, &[]) {
            Ok(outcome) => Ok(outcome),
            Err(error) => Ok(BuiltinOutcome::Status(error.exit_status())),
        };
    }

    let Some(path) = which_in_path(name, shell, use_default_path) else {
        let msg = ByteWriter::new()
            .bytes(b"command: ")
            .bytes(name)
            .bytes(b": not found")
            .finish();
        return Ok(diag_status(shell, 127, &msg));
    };

    if sys::fs::access_path(&path, sys::constants::X_OK).is_err() {
        let msg = ByteWriter::new()
            .bytes(b"command: ")
            .bytes(name)
            .bytes(b": Permission denied")
            .finish();
        return Ok(diag_status(shell, 126, &msg));
    }

    let mut child_env = shell.env_for_child();
    if use_default_path {
        child_env.retain(|(k, _)| k != b"PATH");
        child_env.push((b"PATH".to_vec(), DEFAULT_COMMAND_PATH.to_vec()));
    }
    let env_pairs: Vec<(&[u8], &[u8])> = child_env
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();
    let argv_strs: Vec<&[u8]> = argv.iter().map(|v| v.as_slice()).collect();

    match sys::process::spawn_child(&path, &argv_strs, Some(&env_pairs), &[], None, false, None) {
        Ok(handle) => {
            let ws = sys::process::wait_pid(handle.pid, false)
                .map_err(|e| shell.diagnostic(1, &e.strerror()))?
                .expect("child status");
            Ok(BuiltinOutcome::Status(sys::process::decode_wait_status(
                ws.status,
            )))
        }
        Err(error) if error.is_enoent() => {
            let msg = ByteWriter::new()
                .bytes(b"command: ")
                .bytes(name)
                .bytes(b": not found")
                .finish();
            Ok(diag_status(shell, 127, &msg))
        }
        Err(error) => {
            let msg = ByteWriter::new()
                .bytes(b"command: ")
                .bytes(name)
                .bytes(b": ")
                .bytes(&error.strerror())
                .finish();
            Ok(diag_status(shell, 126, &msg))
        }
    }
}

pub(super) fn which_in_path(name: &[u8], shell: &Shell, use_default_path: bool) -> Option<Vec<u8>> {
    search_path(name, shell, use_default_path, path_exists)
}

pub(super) fn search_path(
    name: &[u8],
    shell: &Shell,
    use_default_path: bool,
    predicate: fn(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    if name.contains_byte(b'/') {
        if predicate(name) {
            return absolute_path(name);
        }
        return None;
    }

    let path_env_owned;
    let path_env: &[u8] = if use_default_path {
        DEFAULT_COMMAND_PATH
    } else {
        path_env_owned = shell
            .get_var(b"PATH")
            .map(|s| s.to_vec())
            .or_else(|| sys::env::env_var(b"PATH"))
            .unwrap_or_default();
        &path_env_owned
    };

    for dir in path_env.split(|&b| b == b':') {
        let candidate = if dir.is_empty() {
            let mut c = b"./".to_vec();
            c.extend_from_slice(name);
            c
        } else {
            let mut c = dir.to_vec();
            c.push(b'/');
            c.extend_from_slice(name);
            c
        };
        if predicate(&candidate) {
            return absolute_path(&candidate);
        }
    }
    None
}

pub(super) fn path_exists(path: &[u8]) -> bool {
    sys::fs::file_exists(path)
}

pub(super) fn readable_regular_file(path: &[u8]) -> bool {
    sys::fs::is_regular_file(path) && sys::fs::access_path(path, sys::constants::R_OK).is_ok()
}

pub(super) fn absolute_path(path: &[u8]) -> Option<Vec<u8>> {
    if path.first() == Some(&b'/') {
        return Some(path.to_vec());
    }
    sys::fs::get_cwd().ok().map(|cwd| {
        let mut result = cwd;
        result.push(b'/');
        result.extend_from_slice(path);
        result
    })
}

pub(super) fn is_reserved_word_name(word: &[u8]) -> bool {
    matches!(
        word,
        b"!" | b"{"
            | b"}"
            | b"case"
            | b"do"
            | b"done"
            | b"elif"
            | b"else"
            | b"esac"
            | b"fi"
            | b"for"
            | b"if"
            | b"in"
            | b"then"
            | b"until"
            | b"while"
    )
}

// ---------------------------------------------------------------------------
// test / [ builtin
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn command_runs_builtin() {
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"hello\n")) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"echo".to_vec(), b"hello".to_vec()],
                )
                .expect("command echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn which_in_path_with_slash_existing() {
        run_trace(
            trace_entries![
                access(str(b"./myscript"), int(libc::F_OK)) -> 0,
                getcwd() -> cwd("/home/user"),
            ],
            || {
                let shell = test_shell();
                let result = which(b"./myscript", &shell);
                assert_eq!(result, Some(b"/home/user/./myscript".to_vec()));
            },
        );
    }

    #[test]
    fn which_in_path_with_slash_not_found() {
        run_trace(
            trace_entries![access(str(b"./nosuch"), int(libc::F_OK)) -> err(libc::ENOENT),],
            || {
                let shell = test_shell();
                let result = which(b"./nosuch", &shell);
                assert!(result.is_none());
            },
        );
    }

    #[test]
    fn command_no_utility_name() {
        let msg = diag(b"command: utility name required");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"command".to_vec(), b"-v".to_vec()]).expect("command -v");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn type_not_found() {
        let msg = diag(b"totally_missing_cmd: not found");
        run_trace(
            trace_entries![
                access(any, any) -> err(libc::ENOENT),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"type".to_vec(), b"totally_missing_cmd".to_vec()],
                )
                .expect("type");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn hash_no_args_empty() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"hash".to_vec()]).expect("hash");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn hash_dash_r() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .path_cache_mut()
                .insert(b"foo"[..].into(), b"/usr/bin/foo".to_vec());
            let outcome = invoke(&mut shell, &[b"hash".to_vec(), b"-r".to_vec()]).expect("hash -r");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            assert!(shell.path_cache().is_empty());
        });
    }

    #[test]
    fn hash_command_not_found() {
        let msg = diag(b"hash: totally_missing: not found");
        run_trace(
            trace_entries![
                access(any, any) -> err(libc::ENOENT),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(&mut shell, &[b"hash".to_vec(), b"totally_missing".to_vec()])
                    .expect("hash missing");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // parse_command_options
    // -----------------------------------------------------------------------

    #[test]
    fn parse_options_dash_p() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> = vec![b"command".to_vec(), b"-p".to_vec(), b"ls".to_vec()];
            let (use_default, mode, idx) = parse_command_options(&argv);
            assert!(use_default);
            assert_eq!(mode, CommandMode::Execute);
            assert_eq!(idx, 2);
        });
    }

    #[test]
    fn parse_options_dash_v() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> = vec![b"command".to_vec(), b"-v".to_vec(), b"ls".to_vec()];
            let (_, mode, idx) = parse_command_options(&argv);
            assert_eq!(mode, CommandMode::QueryShort);
            assert_eq!(idx, 2);
        });
    }

    #[test]
    fn parse_options_dash_big_v() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> = vec![b"command".to_vec(), b"-V".to_vec(), b"ls".to_vec()];
            let (_, mode, idx) = parse_command_options(&argv);
            assert_eq!(mode, CommandMode::QueryVerbose);
            assert_eq!(idx, 2);
        });
    }

    #[test]
    fn parse_options_double_dash() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> =
                vec![b"command".to_vec(), b"--".to_vec(), b"something".to_vec()];
            let (use_default, mode, idx) = parse_command_options(&argv);
            assert!(!use_default);
            assert_eq!(mode, CommandMode::Execute);
            assert_eq!(idx, 2);
        });
    }

    #[test]
    fn parse_options_unknown_dash_flag_stops() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> = vec![b"command".to_vec(), b"-z".to_vec()];
            let (_, mode, idx) = parse_command_options(&argv);
            assert_eq!(mode, CommandMode::Execute);
            assert_eq!(idx, 1);
        });
    }

    #[test]
    fn parse_options_bare_dash_stops() {
        assert_no_syscalls(|| {
            let argv: Vec<Vec<u8>> = vec![b"command".to_vec(), b"-".to_vec()];
            let (_, _, idx) = parse_command_options(&argv);
            assert_eq!(idx, 1);
        });
    }

    // -----------------------------------------------------------------------
    // command_usage_status
    // -----------------------------------------------------------------------

    #[test]
    fn usage_status_execute_mode() {
        assert_no_syscalls(|| {
            assert_eq!(command_usage_status(CommandMode::Execute), 127);
            assert_eq!(command_usage_status(CommandMode::QueryShort), 1);
            assert_eq!(command_usage_status(CommandMode::QueryVerbose), 1);
        });
    }

    // -----------------------------------------------------------------------
    // command -v (short description)
    // -----------------------------------------------------------------------

    #[test]
    fn command_v_external_found() {
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/ls"), int(libc::F_OK)) -> 0,
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"/usr/bin/ls\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"ls".to_vec()],
                )
                .expect("command -v ls");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_v_external_not_found() {
        run_trace(
            trace_entries![access(any, any) -> err(libc::ENOENT)],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"nosuchcmd".to_vec()],
                )
                .expect("command -v nosuchcmd");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn command_v_special_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"export\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"export".to_vec()],
                )
                .expect("command -v export");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_v_regular_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"echo\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"echo".to_vec()],
                )
                .expect("command -v echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_v_function() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"myfunc\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.functions_mut().insert(
                    b"myfunc"[..].into(),
                    std::rc::Rc::new(crate::syntax::ast::Command::Simple(Default::default())),
                );
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"myfunc".to_vec()],
                )
                .expect("command -v myfunc");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_v_reserved_word() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"if\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"if".to_vec()],
                )
                .expect("command -v if");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_v_alias() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"alias ll='ls -la'\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .aliases_mut()
                    .insert(b"ll"[..].into(), b"ls -la"[..].into());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-v".to_vec(), b"ll".to_vec()],
                )
                .expect("command -v ll");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // command -V (verbose description)
    // -----------------------------------------------------------------------

    #[test]
    fn command_big_v_alias() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"ll is an alias for 'ls -la'\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .aliases_mut()
                    .insert(b"ll"[..].into(), b"ls -la"[..].into());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"ll".to_vec()],
                )
                .expect("command -V ll");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_function() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"myfunc is a function\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.functions_mut().insert(
                    b"myfunc"[..].into(),
                    std::rc::Rc::new(crate::syntax::ast::Command::Simple(Default::default())),
                );
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"myfunc".to_vec()],
                )
                .expect("command -V myfunc");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_special_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"export is a special built-in utility\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"export".to_vec()],
                )
                .expect("command -V export");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_regular_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"echo is a regular built-in utility\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"echo".to_vec()],
                )
                .expect("command -V echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_reserved_word() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"if is a reserved word\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"if".to_vec()],
                )
                .expect("command -V if");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_external() {
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/ls"), int(libc::F_OK)) -> 0,
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"ls is /usr/bin/ls\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"ls".to_vec()],
                )
                .expect("command -V ls");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn command_big_v_not_found() {
        run_trace(
            trace_entries![access(any, any) -> err(libc::ENOENT)],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-V".to_vec(), b"nosuchcmd".to_vec()],
                )
                .expect("command -V nosuchcmd");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // command: too many arguments for -v / -V
    // -----------------------------------------------------------------------

    #[test]
    fn command_v_too_many_args() {
        let msg = diag(b"command: too many arguments");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"command".to_vec(),
                        b"-v".to_vec(),
                        b"a".to_vec(),
                        b"b".to_vec(),
                    ],
                )
                .expect("command -v a b");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // command: no utility name in Execute mode -> 127
    // -----------------------------------------------------------------------

    #[test]
    fn command_no_utility_execute_mode() {
        let msg = diag(b"command: utility name required");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"command".to_vec()]).expect("command (bare)");
                assert!(matches!(outcome, BuiltinOutcome::Status(127)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // type builtin
    // -----------------------------------------------------------------------

    #[test]
    fn type_special_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"export is a special built-in utility\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"type".to_vec(), b"export".to_vec()])
                    .expect("type export");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_regular_builtin() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"echo is a regular built-in utility\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"type".to_vec(), b"echo".to_vec()]).expect("type echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_function() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"myfunc is a function\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.functions_mut().insert(
                    b"myfunc"[..].into(),
                    std::rc::Rc::new(crate::syntax::ast::Command::Simple(Default::default())),
                );
                let outcome = invoke(&mut shell, &[b"type".to_vec(), b"myfunc".to_vec()])
                    .expect("type myfunc");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_reserved_word() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"while is a reserved word\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"type".to_vec(), b"while".to_vec()]).expect("type while");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_alias() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"ll is an alias for 'ls -la'\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .aliases_mut()
                    .insert(b"ll"[..].into(), b"ls -la"[..].into());
                let outcome =
                    invoke(&mut shell, &[b"type".to_vec(), b"ll".to_vec()]).expect("type ll");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_external_command() {
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/ls"), int(libc::F_OK)) -> 0,
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"ls is /usr/bin/ls\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome =
                    invoke(&mut shell, &[b"type".to_vec(), b"ls".to_vec()]).expect("type ls");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn type_multiple_mixed() {
        let not_found_msg = diag(b"nosuchcmd: not found");
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"echo is a regular built-in utility\n")) -> auto,
                access(any, any) -> err(libc::ENOENT),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&not_found_msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"type".to_vec(), b"echo".to_vec(), b"nosuchcmd".to_vec()],
                )
                .expect("type echo nosuchcmd");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // execute_command_utility
    // -----------------------------------------------------------------------

    #[test]
    fn execute_command_runs_builtin_directly() {
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"hello\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"echo".to_vec(), b"hello".to_vec()],
                )
                .expect("command echo hello");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn execute_command_external_not_found() {
        let msg = diag(b"command: nosuchcmd: not found");
        run_trace(
            trace_entries![
                access(any, any) -> err(libc::ENOENT),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/nonexistent".to_vec());
                let outcome = invoke(&mut shell, &[b"command".to_vec(), b"nosuchcmd".to_vec()])
                    .expect("command nosuchcmd");
                assert!(matches!(outcome, BuiltinOutcome::Status(127)));
            },
        );
    }

    #[test]
    fn execute_command_external_permission_denied() {
        let msg = diag(b"command: noperm: Permission denied");
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/noperm"), int(libc::F_OK)) -> 0,
                access(str(b"/usr/bin/noperm"), int(libc::X_OK)) -> err(libc::EACCES),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(&mut shell, &[b"command".to_vec(), b"noperm".to_vec()])
                    .expect("command noperm");
                assert!(matches!(outcome, BuiltinOutcome::Status(126)));
            },
        );
    }

    #[test]
    fn execute_command_external_spawn_success() {
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/myext"), int(libc::F_OK)) -> 0,
                access(str(b"/usr/bin/myext"), int(libc::X_OK)) -> 0,
                fork() -> pid(42), child: [
                    execvp(str(b"/usr/bin/myext"), _) -> int(-1),
                ],
                waitpid(42, _) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(&mut shell, &[b"command".to_vec(), b"myext".to_vec()])
                    .expect("command myext");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn execute_command_external_spawn_enoent() {
        let msg = diag(b"command: myext: not found");
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/myext"), int(libc::F_OK)) -> 0,
                access(str(b"/usr/bin/myext"), int(libc::X_OK)) -> 0,
                fork() -> err(libc::ENOENT),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(&mut shell, &[b"command".to_vec(), b"myext".to_vec()])
                    .expect("command myext enoent");
                assert!(matches!(outcome, BuiltinOutcome::Status(127)));
            },
        );
    }

    #[test]
    fn execute_command_external_spawn_other_error() {
        let msg = diag(b"command: myext: Permission denied");
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/myext"), int(libc::F_OK)) -> 0,
                access(str(b"/usr/bin/myext"), int(libc::X_OK)) -> 0,
                fork() -> err(libc::EACCES),
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let outcome = invoke(&mut shell, &[b"command".to_vec(), b"myext".to_vec()])
                    .expect("command myext eacces");
                assert!(matches!(outcome, BuiltinOutcome::Status(126)));
            },
        );
    }

    #[test]
    fn execute_command_with_default_path() {
        run_trace(
            trace_entries![
                access(str(b"/usr/bin/myext"), int(libc::F_OK)) -> 0,
                access(str(b"/usr/bin/myext"), int(libc::X_OK)) -> 0,
                fork() -> pid(50), child: [
                    setenv(str(b"PATH"), str(b"/usr/bin:/bin")) -> 0,
                    execvp(str(b"/usr/bin/myext"), _) -> int(-1),
                ],
                waitpid(50, _) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/custom".to_vec());
                shell.exported_mut().insert(b"PATH".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"-p".to_vec(), b"myext".to_vec()],
                )
                .expect("command -p myext");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    // -----------------------------------------------------------------------
    // search_path: empty dir segment in PATH
    // -----------------------------------------------------------------------

    #[test]
    fn search_path_empty_dir_segment() {
        run_trace(
            trace_entries![
                access(str(b"/a/mybin"), int(libc::F_OK)) -> err(libc::ENOENT),
                access(str(b"./mybin"), int(libc::F_OK)) -> 0,
                getcwd() -> cwd("/home/user"),
            ],
            || {
                let mut shell = test_shell();
                shell.env_mut().insert(b"PATH".to_vec(), b"/a::".to_vec());
                let result = which(b"mybin", &shell);
                assert_eq!(result, Some(b"/home/user/./mybin".to_vec()));
            },
        );
    }

    // -----------------------------------------------------------------------
    // search_path: PATH from env_var fallback
    // -----------------------------------------------------------------------

    #[test]
    fn search_path_fallback_to_getenv() {
        run_trace(
            trace_entries![
                ..vec![crate::sys::test_support::t(
                    "getenv",
                    vec![crate::sys::test_support::ArgMatcher::Str(b"PATH".to_vec())],
                    crate::sys::test_support::TraceResult::StrVal(b"/from/env".to_vec()),
                )],
                access(str(b"/from/env/findme"), int(libc::F_OK)) -> 0,
            ],
            || {
                let shell = test_shell();
                let result = which(b"findme", &shell);
                assert_eq!(result, Some(b"/from/env/findme".to_vec()));
            },
        );
    }

    // -----------------------------------------------------------------------
    // describe_command coverage for all branches
    // -----------------------------------------------------------------------

    #[test]
    fn describe_command_alias() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell
                .aliases_mut()
                .insert(b"ll"[..].into(), b"ls -la"[..].into());
            let desc = describe_command(&shell, b"ll", false);
            assert!(matches!(desc, Some(CommandDescription::Alias(_))));
        });
    }

    #[test]
    fn describe_command_function() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.functions_mut().insert(
                b"myfunc"[..].into(),
                std::rc::Rc::new(crate::syntax::ast::Command::Simple(Default::default())),
            );
            let desc = describe_command(&shell, b"myfunc", false);
            assert_eq!(desc, Some(CommandDescription::Function));
        });
    }

    #[test]
    fn describe_command_special_builtin() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let desc = describe_command(&shell, b"export", false);
            assert_eq!(desc, Some(CommandDescription::SpecialBuiltin));
        });
    }

    #[test]
    fn describe_command_regular_builtin() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let desc = describe_command(&shell, b"echo", false);
            assert_eq!(desc, Some(CommandDescription::RegularBuiltin));
        });
    }

    #[test]
    fn describe_command_reserved_word() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let desc = describe_command(&shell, b"if", false);
            assert_eq!(desc, Some(CommandDescription::ReservedWord));
        });
    }

    #[test]
    fn execute_command_builtin_error_converts_to_status() {
        run_trace(
            trace_entries![
                write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: shift: numeric argument required\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"command".to_vec(), b"shift".to_vec(), b"bad".to_vec()],
                )
                .expect("command shift bad");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}
