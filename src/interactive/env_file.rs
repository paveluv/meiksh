use crate::expand::word;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::process::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_value = shell.get_var(b"ENV").map(|s| s.to_vec());
    let env_file = env_value
        .map(|value| word::expand_parameter_text(shell, &value))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?;
    if let Some(path) = env_file {
        let is_absolute = !path.is_empty() && path[0] == b'/';
        if is_absolute && sys::fs::file_exists(&path) {
            let _ = shell.source_path(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn load_env_file_ignores_relative_path() {
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            shell.env.insert(b"ENV".to_vec(), b"relative.sh".to_vec());
            load_env_file(&mut shell).expect("relative ignored");
        });
    }

    #[test]
    fn load_env_file_ignores_missing_absolute_path() {
        run_trace(
            trace_entries![access(str("/tmp/meiksh-missing-env.sh"), int(0)) -> err(sys::constants::ENOENT),],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert(b"ENV".to_vec(), b"/tmp/meiksh-missing-env.sh".to_vec());
                load_env_file(&mut shell).expect("missing ignored");
            },
        );
    }

    #[test]
    fn load_env_file_sources_existing_absolute_path() {
        run_trace(
            trace_entries![
                access(str("/tmp/env.sh"), int(0)) -> 0,
                open(str("/tmp/env.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_ENV_FILE=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
                load_env_file(&mut shell).expect("source env file");
                assert_eq!(shell.get_var(b"FROM_ENV_FILE"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn load_env_file_expands_parameters() {
        run_trace(
            trace_entries![
                access(str("/home/user/env.sh"), int(0)) -> 0,
                open(str("/home/user/env.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_EXPANDED_ENV=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
                shell
                    .env
                    .insert(b"ENV".to_vec(), b"${HOME}/env.sh".to_vec());
                load_env_file(&mut shell).expect("expanded env file");
                assert_eq!(shell.get_var(b"FROM_EXPANDED_ENV"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn load_env_file_respects_identity_guard() {
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
            shell
                .env
                .insert(b"ENV".to_vec(), b"${HOME}/env.sh".to_vec());
            sys::test_support::with_process_ids_for_test((1, 2, 3, 3), || {
                load_env_file(&mut shell).expect("guarded env file");
            });
            assert_eq!(shell.get_var(b"FROM_EXPANDED_ENV"), None);
        });
    }

    #[test]
    fn load_env_file_propagates_source_errors() {
        run_trace(
            trace_entries![
                access(str("/tmp/bad.sh"), int(0)) -> 0,
                open(str("/tmp/bad.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"echo 'unterminated\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
                write(
                    fd(sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: line 2: unterminated single quote\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"ENV".to_vec(), b"/tmp/bad.sh".to_vec());
                let error = load_env_file(&mut shell).expect_err("invalid env file");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn load_env_file_noop_when_env_variable_unset() {
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            load_env_file(&mut shell).expect("no env");
        });
    }
}
