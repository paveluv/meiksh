use crate::arena::ByteArena;
use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

pub(super) fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_value = shell.get_var(b"ENV").map(|s| s.to_vec());
    let arena = ByteArena::new();
    let env_file = env_value
        .map(|value| expand::expand_parameter_text(shell, &value, &arena).map(|s| s.to_vec()))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?;
    if let Some(path) = env_file {
        let is_absolute = !path.is_empty() && path[0] == b'/';
        if is_absolute && sys::file_exists(&path) {
            let _ = shell.source_path(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::*;

    #[test]
    fn load_env_file_ignores_relative_path() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.env.insert(b"ENV".to_vec(), b"relative.sh".to_vec());
            load_env_file(&mut shell).expect("relative ignored");
        });
    }

    #[test]
    fn load_env_file_ignores_missing_absolute_path() {
        run_trace(
            vec![t(
                "access",
                vec![
                    ArgMatcher::Str("/tmp/meiksh-missing-env.sh".into()),
                    ArgMatcher::Int(0),
                ],
                TraceResult::Err(sys::ENOENT),
            )],
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
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/env.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/env.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"FROM_ENV_FILE=1\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
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
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("/home/user/env.sh".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/home/user/env.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"FROM_EXPANDED_ENV=1\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
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
        run_trace(vec![], || {
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
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("/tmp/bad.sh".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/bad.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"echo 'unterminated\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: line 2: unterminated single quote\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
        run_trace(vec![], || {
            let mut shell = test_shell();
            load_env_file(&mut shell).expect("no env");
        });
    }
}
