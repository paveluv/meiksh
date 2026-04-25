use crate::bstr;
use crate::expand::word;
use crate::shell::options::PromptsMode;
use crate::shell::state::Shell;
use crate::sys;

use super::prompt_expand::{self, Prompt, PromptEnv};

pub(super) fn write_prompt(prompt_str: &[u8]) -> sys::error::SysResult<()> {
    loop {
        match sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt_str) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn read_line() -> sys::error::SysResult<Option<Vec<u8>>> {
    let mut line = Vec::<u8>::new();
    let mut byte = [0u8; 1];
    loop {
        match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(line) }),
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    return Ok(Some(line));
                }
            }
            Err(e) if e.is_eintr() => {
                let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, b"\n");
                return Ok(Some(Vec::new()));
            }
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn expand_prompt(shell: &mut Shell, var: &[u8], default: &[u8]) -> Vec<u8> {
    expand_full_prompt(shell, var, default, PromptKind::Ps1Or2).bytes
}

/// PS4-specific expansion for the `xtrace` writer. Discards the
/// invisible-region mask (xtrace is not cursor-positioned) and skips
/// the history pass per spec § 3.5.
pub(crate) fn expand_ps4(shell: &mut Shell) -> Vec<u8> {
    expand_full_prompt(shell, b"PS4", b"+ ", PromptKind::Ps4).bytes
}

/// Which prompt slot is being expanded. Controls whether the history
/// pass runs and whether the escape pass runs in `bash_prompts` mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptKind {
    /// `PS1` or `PS2`: escape pass + history pass (in bash mode).
    Ps1Or2,
    /// `PS3`: parameter expansion only, no escape pass, no history
    /// pass — matches bash parity per the spec § 3.1. Wired in once
    /// `select` lands; retained here for API completeness.
    #[allow(dead_code)]
    Ps3,
    /// `PS4`: escape pass (bash mode), no history pass, invisible
    /// mask discarded by the caller.
    Ps4,
}

/// Full prompt expansion per ps1-prompt-extensions.md.
///
/// Dispatches on the captured value of `prompts_mode`:
///
/// - `Posix`: parameter expansion plus history (`!`/`!!`) substitution
///   for `Ps1Or2`. The history pass is **POSIX-mandated** for `PS1`
///   (POSIX.1-2024 § 2.5.3 *PS1*: "the value of this variable shall be
///   subjected to parameter expansion … and exclamation-mark
///   expansion"). meiksh applies it to `PS2` as well, matching bash /
///   ksh / dash and consistent with POSIX leaving extensions to
///   `PS2` unspecified.
/// - `Bash`: backslash-escape decoder, then parameter expansion, then
///   (for `Ps1Or2`) the same history `!` substitution.
pub(crate) fn expand_full_prompt(
    shell: &mut Shell,
    var: &[u8],
    default: &[u8],
    kind: PromptKind,
) -> Prompt {
    let raw = shell
        .get_var(var)
        .map(|v| v.to_vec())
        .unwrap_or_else(|| default.to_vec());
    let mode = shell.options.prompts_mode;
    let histnum = shell.history_number();

    let stage1 = match mode {
        // POSIX mode: no backslash-escape decoder; the raw bytes go
        // straight into the parameter pass.
        PromptsMode::Posix => Prompt::new(raw.clone()),
        PromptsMode::Bash => {
            let run_escape_pass = !matches!(kind, PromptKind::Ps3);
            if run_escape_pass {
                let env = build_prompt_env(shell, histnum);
                let tm = sys::time::local_time_now();
                prompt_expand::decode(&raw, &env, &tm)
            } else {
                Prompt::new(raw.clone())
            }
        }
    };

    // Pass 2: parameter expansion on the (possibly escape-decoded)
    // bytes. The invisible mask is preserved as-is; in practice mask
    // ranges wrap literal bytes (color escapes) which parameter
    // expansion leaves alone. Mask alignment under substitution-driven
    // byte shifts is a known limitation covered by
    // ps1-prompt-extensions.md § 8.3 ("implementation-defined
    // representation"), and is outside the scope of this initial
    // implementation.
    let after_param = word::expand_parameter_text(shell, &stage1.bytes).unwrap_or(stage1.bytes);

    // Pass 3: POSIX-mandated `!`/`!!` history substitution. Runs for
    // `Ps1Or2` in BOTH `Posix` and `Bash` modes. Earlier revisions
    // gated this on `bash_prompts`, which made meiksh non-conformant
    // with POSIX.1-2024 § 2.5.3 (`PS1` "shall be subjected to
    // parameter expansion … and exclamation-mark expansion") in its
    // default mode. `PS3` and `PS4` are excluded — the spec only
    // requires `!` expansion for `PS1`, and bash treats `PS3`/`PS4`
    // identically.
    let bytes = if matches!(kind, PromptKind::Ps1Or2) {
        expand_prompt_exclamation(&after_param, histnum)
    } else {
        after_param
    };

    Prompt {
        bytes,
        invisible: stage1.invisible,
    }
}

fn build_prompt_env(shell: &Shell, history_number: usize) -> PromptEnv {
    // `$USER` is our preferred source; fall back to
    // `getpwuid(geteuid())` per spec § 6.1 `\u`.
    let user = match shell.get_var(b"USER") {
        Some(v) if !v.is_empty() => Some(v.to_vec()),
        _ => {
            sys::process::getpwuid_name(sys::process::effective_uid_raw()).filter(|v| !v.is_empty())
        }
    };
    let home = shell
        .get_var(b"HOME")
        .filter(|v| !v.is_empty())
        .map(|v| v.to_vec());
    // `$PWD` is preferred; fall back to `getcwd(3)`.
    let cwd = match shell.get_var(b"PWD") {
        Some(v) if !v.is_empty() => Some(v.to_vec()),
        _ => sys::fs::get_cwd().ok(),
    };
    let hostname = sys::process::hostname_bytes().filter(|v| !v.is_empty());
    let tty_basename =
        sys::tty::tty_basename(sys::constants::STDIN_FILENO).filter(|v| !v.is_empty());

    // Major.minor short version is derived from CARGO_PKG_VERSION
    // (which is `MAJOR.MINOR.PATCH`). We use a tiny runtime split so
    // we don't need a build-time constant.
    let full = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    // `CARGO_PKG_VERSION` is always `MAJOR.MINOR.PATCH` (two dots), so
    // the rposition of `.` always points at the PATCH separator; the
    // short form drops the PATCH component.
    let last_dot = full.iter().rposition(|b| *b == b'.').unwrap_or(full.len());
    let short = full[..last_dot].to_vec();

    PromptEnv {
        user,
        hostname,
        cwd,
        home,
        tty_basename,
        euid_is_root: sys::process::effective_uid_is_root(),
        shell_name: shell.shell_name.to_vec(),
        jobs_count: shell.jobs.len(),
        history_number,
        session_counter: shell.session_command_counter,
        version_short: short,
        version_long: full,
    }
}

pub(super) fn expand_prompt_exclamation(s: &[u8], histnum: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'!' {
            i += 1;
            if i < s.len() && s[i] == b'!' {
                result.push(b'!');
                i += 1;
            } else if i < s.len() {
                bstr::push_u64(&mut result, histnum as u64);
                result.push(s[i]);
                i += 1;
            } else {
                bstr::push_u64(&mut result, histnum as u64);
            }
        } else {
            result.push(s[i]);
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn prompt_prefers_ps1() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"$ ");
            shell
                .env_mut()
                .insert(b"PS1".to_vec(), b"custom> ".to_vec());
            assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"custom> ");
        });
    }

    #[test]
    fn read_line_propagates_non_eintr_error() {
        run_trace(
            trace_entries![read(fd(sys::constants::STDIN_FILENO), _) -> err(sys::constants::EBADF)],
            || {
                let err = read_line().expect_err("should propagate EBADF");
                assert!(!err.is_eintr());
            },
        );
    }

    #[test]
    fn read_line_returns_empty_on_eintr() {
        run_trace(
            trace_entries![
                read(fd(sys::constants::STDIN_FILENO), _) -> err(sys::constants::EINTR),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"\n")) -> auto,
            ],
            || {
                let result = read_line().expect("should not fail on EINTR");
                assert_eq!(result, Some(Vec::new()));
            },
        );
    }

    #[test]
    fn expand_prompt_exclamation_covers_all_branches() {
        assert_no_syscalls(|| {
            assert_eq!(expand_prompt_exclamation(b"!!", 42), b"!");
            assert_eq!(expand_prompt_exclamation(b"!x", 42), b"42x");
            assert_eq!(expand_prompt_exclamation(b"!", 42), b"42");
            assert_eq!(expand_prompt_exclamation(b"no bang", 42), b"no bang");
        });
    }

    /// § 7.1: "The resulting digits shall be emitted verbatim; the
    /// history pass (Section 7.2) shall not re-scan those digits."
    ///
    /// We can't observe the full pipeline from a unit test without
    /// spinning up a Shell, but the helper contract is that
    /// `expand_prompt_exclamation` treats digits as plain bytes. The
    /// only thing that triggers substitution is the `!` byte itself.
    #[test]
    fn history_pass_does_not_scan_plain_digits() {
        assert_no_syscalls(|| {
            // Digits passed in verbatim survive unchanged.
            assert_eq!(expand_prompt_exclamation(b"12", 99), b"12");
            // Digits that happen to equal the history number are
            // still left alone — only `!` triggers substitution.
            assert_eq!(expand_prompt_exclamation(b"42-99", 99), b"42-99");
            // Digits adjacent to a `!` are still literal.
            assert_eq!(expand_prompt_exclamation(b"7!9", 99), b"7999");
        });
    }

    /// § 7.2: "A `!` introduced by parameter expansion shall be
    /// subject to this pass exactly like a `!` written directly in
    /// `PS1`". The helper sees whatever bytes pass 2 produced, so if
    /// those bytes include a `!`, it is substituted.
    #[test]
    fn history_pass_scans_bang_irrespective_of_origin() {
        assert_no_syscalls(|| {
            // A literal `!` in the middle of surrounding text — the
            // same shape that `expand_parameter_text` would produce
            // when $VAR resolves to bytes containing `!`.
            assert_eq!(expand_prompt_exclamation(b"x!y", 5), b"x5y");
        });
    }

    // === Full-pipeline behavior (expand_full_prompt) ==================

    /// POSIX.1-2024 § 2.5.3 *PS1*: in default (POSIX) mode, the
    /// escape pass does NOT run (so `\u`/`\h`/`\w` survive verbatim
    /// as backslash-letter pairs) but the **`!`/`!!` history pass
    /// is mandatory** ("the value of this variable shall be subjected
    /// to parameter expansion … and exclamation-mark expansion").
    /// `test_shell()` has a fresh history with `history_number() == 1`,
    /// so a bare `!foo` expands to `1foo`.
    #[test]
    fn expand_full_prompt_posix_mode_runs_history_pass_only() {
        let mut shell = test_shell();
        shell
            .env_mut()
            .insert(b"PS1".to_vec(), b"\\u@\\h:\\w!foo$ ".to_vec());
        let out = expand_full_prompt(&mut shell, b"PS1", b"", PromptKind::Ps1Or2);
        // Backslash-letter pairs survive (no bash escape pass);
        // literal `!` is replaced by the history number per POSIX.
        assert_eq!(out.bytes, b"\\u@\\h:\\w1foo$ ");
        assert!(out.invisible.is_empty());
    }

    /// POSIX.1-2024 § 2.5.3 also defines `!!` as the escape for a
    /// literal `!` ("An `<exclamation-mark>` character escaped by
    /// another `<exclamation-mark>` character (that is, `\"!!\"`)
    /// shall expand to a single `<exclamation-mark>` character"). In
    /// POSIX mode this rule must hold without `set -o bash_prompts`.
    #[test]
    fn expand_full_prompt_posix_mode_double_bang_emits_single_bang() {
        let mut shell = test_shell();
        shell
            .env_mut()
            .insert(b"PS1".to_vec(), b"cmd !!> ".to_vec());
        let out = expand_full_prompt(&mut shell, b"PS1", b"", PromptKind::Ps1Or2);
        assert_eq!(out.bytes, b"cmd !> ");
    }

    /// POSIX.1-2024 § 2.5.3 only requires `!` expansion for `PS1`.
    /// `PS4` (xtrace) shall NOT undergo the history pass; a literal
    /// `!` in `PS4` survives verbatim. Tested through `Ps4` so the
    /// `PromptKind` gate is exercised.
    #[test]
    fn expand_full_prompt_ps4_does_not_run_history_pass() {
        let mut shell = test_shell();
        shell.env_mut().insert(b"PS4".to_vec(), b"+!+ ".to_vec());
        let out = expand_full_prompt(&mut shell, b"PS4", b"+ ", PromptKind::Ps4);
        assert_eq!(out.bytes, b"+!+ ");
    }

    /// § 5 + § 7.1: `\!` decodes during the escape pass to the
    /// history number, and a subsequent literal `!` is independently
    /// substituted by the history pass. The two mechanisms compose
    /// without clobbering each other — the contract in § 7.1 third
    /// bullet.
    #[test]
    fn expand_full_prompt_bash_mode_runs_all_three_passes() {
        let mut shell = test_shell();
        shell
            .options
            .set_named_option(b"bash_prompts", true)
            .expect("toggle bash_prompts");
        // Pre-populate PWD so `build_prompt_env` does not fall back
        // to `getcwd(3)` (which would panic under the no-trace
        // assertion of the test harness).
        shell.env_mut().insert(b"PWD".to_vec(), b"/tmp".to_vec());
        shell.env_mut().insert(b"PS1".to_vec(), b"\\!-!".to_vec());
        // history_number() on a fresh shell equals 1.
        let out = expand_full_prompt(&mut shell, b"PS1", b"", PromptKind::Ps1Or2);
        assert_eq!(out.bytes, b"1-1");
    }

    /// § 3.6: "Prompt variables shall be re-expanded on every prompt
    /// write. Meiksh shall not cache the expanded value." Two
    /// consecutive expansions of `PS1='$TAG'` with the var mutated
    /// in between must observe the new value.
    #[test]
    fn expand_full_prompt_reruns_parameter_pass_each_call() {
        let mut shell = test_shell();
        shell.env_mut().insert(b"PS1".to_vec(), b"$TAG".to_vec());
        shell.env_mut().insert(b"TAG".to_vec(), b"first".to_vec());
        let first = expand_full_prompt(&mut shell, b"PS1", b"", PromptKind::Ps1Or2);
        assert_eq!(first.bytes, b"first");

        shell.env_mut().insert(b"TAG".to_vec(), b"second".to_vec());
        let second = expand_full_prompt(&mut shell, b"PS1", b"", PromptKind::Ps1Or2);
        assert_eq!(second.bytes, b"second");
    }

    /// § 3.2: The default `PS1` value is the caller-supplied literal
    /// regardless of `bash_prompts`. The `bash_prompts` option only
    /// toggles escape-sequence decoding on the *configured* `PS1`
    /// value; it never substitutes a different default. `PS1`'s
    /// POSIX-spec default (`"$ "` or `"# "` for root) is instead
    /// seeded by `load_startup_files`, so by the time `expand_full_prompt`
    /// is reached with an unset `PS1`, the caller's static default is
    /// the sole fallback.
    #[test]
    fn expand_full_prompt_default_ps1_in_bash_mode_is_caller_default() {
        let mut shell = test_shell();
        shell
            .options
            .set_named_option(b"bash_prompts", true)
            .expect("toggle bash_prompts");
        shell.env_mut().insert(b"PWD".to_vec(), b"/tmp".to_vec());
        shell.env_mut().remove(b"PS1".as_slice());
        let out = expand_full_prompt(&mut shell, b"PS1", b"$ ", PromptKind::Ps1Or2);
        // No `\s-\v\$` substitution, no transformation: the caller's
        // default is returned unchanged because it contains no escape
        // sequences or parameter expansions.
        assert_eq!(out.bytes, b"$ ");
    }

    /// § 3.2: When `bash_prompts` is off and `PS1` is unset, the
    /// default is the caller-supplied `"$ "` literal — no escape pass,
    /// no transformation.
    #[test]
    fn expand_full_prompt_default_ps1_in_posix_mode_is_caller_default() {
        let mut shell = test_shell();
        shell.env_mut().remove(b"PS1".as_slice());
        let out = expand_full_prompt(&mut shell, b"PS1", b"$ ", PromptKind::Ps1Or2);
        assert_eq!(out.bytes, b"$ ");
    }

    /// § 3.1 table: `PS3` in bash mode skips the escape pass entirely.
    #[test]
    fn build_prompt_env_falls_back_to_getcwd_when_pwd_unset() {
        // When `$PWD` is empty, `build_prompt_env` falls through to
        // `sys::fs::get_cwd()` to populate `cwd`.  Covers the
        // `_ => sys::fs::get_cwd().ok()` arm.
        run_trace(trace_entries![getcwd() -> cwd("/tmp/cov")], || {
            let mut shell = test_shell();
            shell.env_mut().remove(b"PWD".as_slice());
            let env = super::build_prompt_env(&shell, 1);
            assert_eq!(env.cwd.as_deref(), Some(b"/tmp/cov" as &[u8]));
        });
    }

    #[test]
    fn expand_full_prompt_ps3_skips_escape_pass_even_in_bash_mode() {
        let mut shell = test_shell();
        shell
            .options
            .set_named_option(b"bash_prompts", true)
            .expect("toggle bash_prompts");
        shell.env_mut().insert(b"PWD".to_vec(), b"/tmp".to_vec());
        shell
            .env_mut()
            .insert(b"PS3".to_vec(), b"\\u pick: ".to_vec());
        let out = expand_full_prompt(&mut shell, b"PS3", b"#? ", PromptKind::Ps3);
        // `\u` is NOT decoded because the escape pass is skipped.
        assert_eq!(out.bytes, b"\\u pick: ");
    }
}
