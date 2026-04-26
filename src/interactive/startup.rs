//! Interactive-shell startup-file loader.
//!
//! Implements the meiksh startup sequence defined in
//! `docs/features/startup-files.md`: set the exported `MEIKSH_VERSION`
//! marker, then — under an identity guard — source `/etc/profile`,
//! `$HOME/.profile`, and `$ENV`, in that order, skipping each file
//! that does not exist.

use crate::expand::word;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

/// System-wide profile sourced first. See spec § 3.1.
const SYSTEM_PROFILE_PATH: &[u8] = b"/etc/profile";

/// Name of the marker variable exported before any startup file is
/// sourced (spec § 5). Mirrors the `BASH_VERSION` / `ZSH_VERSION` /
/// `KSH_VERSION` convention other shells use. Scripts branch on its
/// presence with `[ -n "${MEIKSH_VERSION:-}" ]`.
const MEIKSH_MARKER_NAME: &[u8] = b"MEIKSH_VERSION";

/// Value assigned to [`MEIKSH_MARKER_NAME`]. The crate's SemVer
/// string — e.g. `"0.1.1"` — sourced at compile time from Cargo so
/// the exported value stays in lockstep with every release. See
/// spec § 5 for the forward-compatibility contract.
const MEIKSH_MARKER_VALUE: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();

/// Internal test-isolation opt-out (spec § 7). When set to `1` in the
/// environment, every file from Section 3 is skipped; the
/// `MEIKSH_VERSION` marker is still established so scripts can tell
/// they are running under a meiksh-managed test harness. This knob is read from the
/// shell variable table and therefore honors `export` from within the
/// shell; it is intentionally *not* exposed in public documentation
/// because it is not part of the user-facing contract.
const STARTUP_SKIP_ENV_NAME: &[u8] = b"MEIKSH_SKIP_STARTUP_FILES";
const STARTUP_SKIP_ENV_VALUE: &[u8] = b"1";

pub(super) fn load_startup_files(shell: &mut Shell) -> Result<(), ShellError> {
    // Spec § 5: the `MEIKSH_VERSION` marker is established *before*
    // any file is sourced and *before* the identity guard is
    // evaluated, so that privileged shells which skip all sourcing
    // still announce their identity to child processes (e.g. a
    // `printenv` run from a locked-down setuid meiksh).
    let _ = shell.set_var(MEIKSH_MARKER_NAME, MEIKSH_MARKER_VALUE);
    shell.mark_exported(MEIKSH_MARKER_NAME);

    // Seed `PS1` with the POSIX-spec default before sourcing any
    // startup file, so that scripts like `/etc/profile` whose outer
    // gate tests `[ "${PS1-}" ]` observe the variable as set and
    // proceed with their interactive-shell configuration. Per POSIX
    // XCU §2 <https://pubs.opengroup.org/onlinepubs/9799919799/>,
    // the default `PS1` value is `"$ "`; a privileged user (effective
    // UID 0) gets the implementation-defined alternative `"# "`,
    // matching the historical-superuser convention codified in
    // `docs/features/ps1-prompt-extensions.md` §3.2. The seed is
    // intentionally **not exported** — bash does not export `PS1`
    // either, and the module's `is_exported` assertions for
    // `MEIKSH_VERSION` depend on that invariant. Users who already
    // set `PS1` (e.g. via inherited environment) keep their value.
    if shell.get_var(b"PS1").is_none() {
        let default: &[u8] = if sys::process::effective_uid_is_root() {
            b"# "
        } else {
            b"$ "
        };
        let _ = shell.set_var(b"PS1", default);
    }

    // Spec § 4: setuid / setgid shells skip every file under § 3.
    if !sys::process::has_same_real_and_effective_ids() {
        return Ok(());
    }

    // Spec § 7 internal test hook: honoring `MEIKSH_SKIP_STARTUP_FILES=1`
    // lets the PTY harness spawn a `meiksh -i` process without the
    // developer's `/etc/profile`, `~/.profile`, or `$ENV` perturbing the
    // tests. The marker (set above) is still exported, matching what a
    // real interactive meiksh would expose to any child process.
    if shell.get_var(STARTUP_SKIP_ENV_NAME) == Some(STARTUP_SKIP_ENV_VALUE) {
        return Ok(());
    }

    // Spec § 3.1: /etc/profile.
    if sys::fs::file_exists(SYSTEM_PROFILE_PATH) {
        let _ = shell.source_path(SYSTEM_PROFILE_PATH)?;
    }

    // Spec § 3.2: $HOME/.profile. Resolved by byte concatenation (no
    // tilde expansion, no `getpwuid(3)` fallback — POSIX parameter
    // semantics only).
    if let Some(profile) = home_profile_path(shell)
        && sys::fs::file_exists(&profile)
    {
        let _ = shell.source_path(&profile)?;
    }

    // Spec § 3.3: $ENV, after the two profile files so that they may
    // set or redirect it. Parameter expansion uses the shell's current
    // state, which already reflects assignments performed by § 3.1 and
    // § 3.2.
    let env_value = shell.get_var(b"ENV").map(|s| s.to_vec());
    let env_file = env_value
        .map(|value| word::expand_parameter_text(shell, &value))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?;
    if let Some(path) = env_file
        && path.starts_with(b"/")
        && sys::fs::file_exists(&path)
    {
        let _ = shell.source_path(&path)?;
    }

    Ok(())
}

fn home_profile_path(shell: &Shell) -> Option<Vec<u8>> {
    let home = shell.get_var(b"HOME")?;
    if home.is_empty() || !home.starts_with(b"/") {
        return None;
    }
    let mut path = home.to_vec();
    if !path.ends_with(b"/") {
        path.push(b'/');
    }
    path.extend_from_slice(b".profile");
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn sets_meiksh_version_marker_before_sourcing() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                load_startup_files(&mut shell).expect("startup");
                let expected = env!("CARGO_PKG_VERSION").as_bytes();
                assert_eq!(shell.get_var(b"MEIKSH_VERSION"), Some(expected));
                assert!(shell.is_exported(b"MEIKSH_VERSION"));
                // A non-empty SemVer string — mirrors BASH_VERSION /
                // ZSH_VERSION / KSH_VERSION semantics. Scripts that
                // merely test presence (`[ -n "${MEIKSH_VERSION:-}" ]`)
                // are forward-compatible; the specific value is the
                // crate's Cargo version and will change across
                // releases.
                assert!(!expected.is_empty());
            },
        );
    }

    #[test]
    fn sources_etc_profile_when_present() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> 0,
                open(str("/etc/profile"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_ETC_PROFILE=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"FROM_ETC_PROFILE"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn sources_home_profile_when_present() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/home/u/.profile"), int(0)) -> 0,
                open(str("/home/u/.profile"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_HOME_PROFILE=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/u".to_vec());
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"FROM_HOME_PROFILE"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn home_profile_appends_slash_when_home_has_no_trailing_slash() {
        // /home/u -> /home/u/.profile
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/home/u/.profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/u".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn home_profile_does_not_duplicate_trailing_slash() {
        // /home/u/ -> /home/u/.profile (only one slash)
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/home/u/.profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/u/".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn home_profile_skipped_when_home_unset() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn home_profile_skipped_when_home_empty() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell.env_mut().insert(b"HOME".to_vec(), b"".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn home_profile_skipped_when_home_relative() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"relative/home".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn env_file_sourced_when_absolute_and_present() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/tmp/env.sh"), int(0)) -> 0,
                open(str("/tmp/env.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_ENV_FILE=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"FROM_ENV_FILE"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn env_file_skipped_when_relative() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"relative.sh".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn env_file_skipped_when_absolute_but_missing() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/tmp/missing.sh"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"/tmp/missing.sh".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn env_file_expands_parameters() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/home/u/.profile"), int(0)) -> err(sys::constants::ENOENT),
                access(str("/home/u/env.sh"), int(0)) -> 0,
                open(str("/home/u/env.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"FROM_EXPANDED=1\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/u".to_vec());
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"${HOME}/env.sh".to_vec());
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"FROM_EXPANDED"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn skip_env_var_bypasses_all_files_but_keeps_marker() {
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"HOME".to_vec(), b"/home/u".to_vec());
            shell
                .env_mut()
                .insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
            shell
                .env_mut()
                .insert(b"MEIKSH_SKIP_STARTUP_FILES".to_vec(), b"1".to_vec());
            load_startup_files(&mut shell).expect("startup");
            assert_eq!(
                shell.get_var(b"MEIKSH_VERSION"),
                Some(env!("CARGO_PKG_VERSION").as_bytes())
            );
            assert!(shell.is_exported(b"MEIKSH_VERSION"));
            assert_eq!(shell.get_var(b"FROM_ENV_FILE"), None);
            assert_eq!(shell.get_var(b"FROM_HOME_PROFILE"), None);
        });
    }

    #[test]
    fn skip_env_var_only_honors_exact_value_one() {
        // A value other than b"1" (e.g. "yes", "true", empty) must
        // not activate the skip path. This guards against accidental
        // semantic drift — the knob is a presence-plus-value signal,
        // distinct from the version-string-valued `MEIKSH_VERSION`
        // marker itself.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"MEIKSH_SKIP_STARTUP_FILES".to_vec(), b"yes".to_vec());
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn setuid_guard_skips_all_files_but_keeps_marker() {
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"HOME".to_vec(), b"/home/u".to_vec());
            shell
                .env_mut()
                .insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
            sys::test_support::with_process_ids_for_test((1, 2, 3, 3), || {
                load_startup_files(&mut shell).expect("guarded startup");
            });
            assert_eq!(
                shell.get_var(b"MEIKSH_VERSION"),
                Some(env!("CARGO_PKG_VERSION").as_bytes())
            );
            assert!(shell.is_exported(b"MEIKSH_VERSION"));
            assert_eq!(shell.get_var(b"FROM_ENV_FILE"), None);
            assert_eq!(shell.get_var(b"FROM_HOME_PROFILE"), None);
            // Even under the setuid guard, `PS1` is still seeded: a
            // locked-down privileged shell still needs a functional
            // prompt, and the seed runs before the guard bail-out by
            // design (see the comment in `load_startup_files`).
            assert_eq!(shell.get_var(b"PS1"), Some(b"$ ".as_ref()));
            assert!(!shell.is_exported(b"PS1"));
        });
    }

    #[test]
    fn seeds_default_ps1_when_unset_and_non_root() {
        // Non-root effective UID → POSIX default `"$ "`. Trace verifies
        // that the seed happens before the `/etc/profile` access() so
        // the profile's `[ "${PS1-}" ]` gate sees `PS1` as set.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                sys::test_support::with_process_ids_for_test((1000, 1000, 1000, 1000), || {
                    load_startup_files(&mut shell).expect("startup");
                });
                assert_eq!(shell.get_var(b"PS1"), Some(b"$ ".as_ref()));
                // PS1 must **not** be exported — bash does not export
                // it either, and the interactive shell's assertions
                // about which variables cross `exec` rely on this.
                assert!(!shell.is_exported(b"PS1"));
            },
        );
    }

    #[test]
    fn seeds_default_ps1_when_unset_and_root() {
        // Effective UID 0 → implementation-defined alternative `"# "`
        // per POSIX XCU §2 and `docs/features/ps1-prompt-extensions.md`
        // §3.2.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                sys::test_support::with_process_ids_for_test((0, 0, 0, 0), || {
                    load_startup_files(&mut shell).expect("startup");
                });
                assert_eq!(shell.get_var(b"PS1"), Some(b"# ".as_ref()));
                assert!(!shell.is_exported(b"PS1"));
            },
        );
    }

    #[test]
    fn preserves_user_ps1_when_already_set() {
        // A `PS1` inherited from the parent environment (or set by the
        // user at startup via command-line assignment) must not be
        // overwritten by the seed. The seed is strictly "unset → POSIX
        // default"; any value — including empty — takes precedence.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PS1".to_vec(), b"custom> ".to_vec());
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"PS1"), Some(b"custom> ".as_ref()));
            },
        );
    }

    #[test]
    fn seeds_ps1_even_when_startup_files_are_skipped() {
        // The MEIKSH_SKIP_STARTUP_FILES early-return happens *after*
        // the `PS1` seed: a shell that skips startup sourcing still
        // needs a sensible prompt, and the seed is free of any
        // side effects that would conflict with the skip contract.
        run_trace(trace_entries![], || {
            let mut shell = test_shell();
            shell
                .env_mut()
                .insert(b"MEIKSH_SKIP_STARTUP_FILES".to_vec(), b"1".to_vec());
            load_startup_files(&mut shell).expect("startup");
            assert_eq!(shell.get_var(b"PS1"), Some(b"$ ".as_ref()));
            assert!(!shell.is_exported(b"PS1"));
        });
    }

    #[test]
    fn sources_all_three_files_in_order() {
        // /etc/profile sets STEP=etc; ~/.profile re-assigns STEP=home;
        // $ENV re-assigns STEP=env. The final value shall be `env`,
        // proving that sourcing order is etc -> home -> env.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> 0,
                open(str("/etc/profile"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"STEP=etc\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
                access(str("/home/u/.profile"), int(0)) -> 0,
                open(str("/home/u/.profile"), any, any) -> fd(11),
                read(fd(11), _) -> bytes(b"STEP=home\n"),
                read(fd(11), _) -> 0,
                close(fd(11)) -> 0,
                access(str("/tmp/env.sh"), int(0)) -> 0,
                open(str("/tmp/env.sh"), any, any) -> fd(12),
                read(fd(12), _) -> bytes(b"STEP=env\n"),
                read(fd(12), _) -> 0,
                close(fd(12)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/u".to_vec());
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"/tmp/env.sh".to_vec());
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"STEP"), Some(b"env".as_ref()));
            },
        );
    }

    #[test]
    fn etc_profile_can_set_env_for_later_expansion() {
        // Verifies spec § 3.3: /etc/profile may set $ENV and the
        // later expansion step shall see that value.
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> 0,
                open(str("/etc/profile"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"ENV=/tmp/late.sh\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
                access(str("/tmp/late.sh"), int(0)) -> 0,
                open(str("/tmp/late.sh"), any, any) -> fd(11),
                read(fd(11), _) -> bytes(b"FROM_LATE=1\n"),
                read(fd(11), _) -> 0,
                close(fd(11)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                load_startup_files(&mut shell).expect("startup");
                assert_eq!(shell.get_var(b"FROM_LATE"), Some(b"1".as_ref()));
            },
        );
    }

    #[test]
    fn env_variable_unset_is_a_no_op() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                load_startup_files(&mut shell).expect("startup");
            },
        );
    }

    #[test]
    fn source_error_propagates() {
        run_trace(
            trace_entries![
                access(str("/etc/profile"), int(0)) -> err(sys::constants::ENOENT),
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
                shell
                    .env_mut()
                    .insert(b"ENV".to_vec(), b"/tmp/bad.sh".to_vec());
                let err = load_startup_files(&mut shell).expect_err("bad env");
                assert_ne!(err.exit_status(), 0);
            },
        );
    }
}
