use crate::bstr;
use crate::expand::word;
use crate::shell::options::CompatMode;
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
/// pass runs and whether the escape pass runs in `bash_compat` mode.
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
/// Dispatches on the captured value of `compat_mode`:
///
/// - `Posix`: parameter expansion only; no backslash-escape decoding;
///   no history `!` substitution.
/// - `Bash`: escape decoder, then parameter expansion, then (for
///   `Ps1Or2`) history `!` substitution.
pub(crate) fn expand_full_prompt(
    shell: &mut Shell,
    var: &[u8],
    default: &[u8],
    kind: PromptKind,
) -> Prompt {
    let raw = shell.get_var(var).map(|v| v.to_vec()).unwrap_or_else(|| {
        // If unset and bash_compat is on, PS1 falls back to "\s-\v\$ ".
        // See spec § 3.2. Other slots use their static default.
        if var == b"PS1" && matches!(shell.options.compat_mode, CompatMode::Bash) {
            b"\\s-\\v\\$ ".to_vec()
        } else {
            default.to_vec()
        }
    });
    let compat = shell.options.compat_mode;
    let histnum = shell.history_number();

    match compat {
        CompatMode::Posix => {
            // No escape pass; parameter expansion only; no history.
            let expanded = word::expand_parameter_text(shell, &raw).unwrap_or_else(|_| raw.clone());
            Prompt::new(expanded)
        }
        CompatMode::Bash => {
            let run_escape_pass = !matches!(kind, PromptKind::Ps3);
            let stage1 = if run_escape_pass {
                let env = build_prompt_env(shell, histnum);
                let tm = sys::time::local_time_now();
                prompt_expand::decode(&raw, &env, &tm)
            } else {
                Prompt::new(raw.clone())
            };
            // Pass 2: parameter expansion on the rendered bytes. The
            // invisible mask is preserved as-is; in practice mask
            // ranges wrap literal bytes (color escapes) which
            // parameter expansion leaves alone. Mask alignment under
            // substitution-driven byte shifts is a known limitation
            // covered by spec § 8.3 ("implementation-defined
            // representation"), and is outside the scope of this
            // initial implementation.
            let after_param =
                word::expand_parameter_text(shell, &stage1.bytes).unwrap_or(stage1.bytes);
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
    let short = match (
        full.iter().position(|b| *b == b'.'),
        full.iter().rposition(|b| *b == b'.'),
    ) {
        (Some(first), Some(last)) if first < last => full[..last].to_vec(),
        _ => full.clone(),
    };

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
}
