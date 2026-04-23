//! The `bind` builtin: inspect and modify the emacs keymap.
//!
//! Supported forms (spec `docs/features/emacs-editing-mode.md` § 10
//! and `docs/features/inputrc.md`):
//!
//! * `bind` / `bind -p` — dump bindings in inputrc format to stdout.
//! * `bind -l` — list bindable function names.
//! * `bind -r <keyseq>` — remove a binding.
//! * `bind -f <file>` — load bindings from an inputrc file.
//! * `bind -x '<keyseq>':<command>` — bind to external shell command.
//! * `bind '<keyseq>: <target>'` — apply a single inputrc line.

use super::{BuiltinOutcome, write_stdout_line};
use crate::interactive::emacs_editing::keymap::{ALL_FUNCTIONS, EmacsFn, KeymapEntry};
use crate::interactive::inputrc::{self, Conditions, EmacsContext, Mode};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

/// Build the [`Conditions`] used when a `bind`-family call needs to
/// run the inputrc parser. Meiksh always targets the emacs keymap,
/// and `$if term=` tests resolve against the current `$TERM`.
fn conditions_for(shell: &Shell) -> Conditions {
    let term = shell
        .get_var(b"TERM")
        .map(|b| b.to_vec())
        .unwrap_or_default();
    Conditions::new(Mode::Emacs, term)
}

pub(super) fn bind(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut args = argv[1..].iter();
    let first = match args.next() {
        None => return dump_bindings(),
        Some(a) => a,
    };
    match first.as_slice() {
        b"-l" => do_list_functions(),
        b"-p" => dump_bindings(),
        b"-r" => {
            let key = match args.next() {
                Some(k) => k,
                None => return Err(shell.diagnostic(1, b"bind: -r requires a keyseq")),
            };
            do_remove(key)
        }
        b"-f" => {
            let file = match args.next() {
                Some(f) => f,
                None => return Err(shell.diagnostic(1, b"bind: -f requires a filename")),
            };
            do_load_file(shell, file)
        }
        b"-x" => {
            let spec = match args.next() {
                Some(s) => s,
                None => return Err(shell.diagnostic(1, b"bind: -x requires an argument")),
            };
            do_bind_x(shell, spec)
        }
        arg if arg.starts_with(b"-") && arg != b"-" => {
            let mut msg = b"bind: unknown option: ".to_vec();
            msg.extend_from_slice(arg);
            let _ = shell.diagnostic(2, &msg);
            Ok(BuiltinOutcome::Status(2))
        }
        // Single-argument form: treat as one inputrc line.
        _ => {
            let line = first.clone();
            do_apply_line(shell, &line)
        }
    }
}

fn do_list_functions() -> Result<BuiltinOutcome, ShellError> {
    for f in ALL_FUNCTIONS {
        let func: EmacsFn = *f;
        write_stdout_line(func.name());
    }
    Ok(BuiltinOutcome::Status(0))
}

fn dump_bindings() -> Result<BuiltinOutcome, ShellError> {
    let ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let mut out = Vec::new();
    ctx.keymap.dump_inputrc(&mut out);
    drop(ctx);
    use crate::sys;
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &out);
    Ok(BuiltinOutcome::Status(0))
}

fn do_remove(key: &[u8]) -> Result<BuiltinOutcome, ShellError> {
    let bytes = match parse_key_argument(key) {
        Ok(b) => b,
        Err(_) => return Ok(BuiltinOutcome::Status(1)),
    };
    let mut ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if ctx.keymap.unbind(&bytes) {
        Ok(BuiltinOutcome::Status(0))
    } else {
        Ok(BuiltinOutcome::Status(1))
    }
}

fn do_load_file(shell: &Shell, file: &[u8]) -> Result<BuiltinOutcome, ShellError> {
    let conditions = conditions_for(shell);
    let mut ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let report = inputrc::load_from_path(file, &mut ctx, &conditions);
    inputrc::report_diagnostics(file, &report);
    if report.applied_lines == 0 && !report.diagnostics.is_empty() {
        Ok(BuiltinOutcome::Status(1))
    } else {
        Ok(BuiltinOutcome::Status(0))
    }
}

fn do_bind_x(_shell: &mut Shell, spec: &[u8]) -> Result<BuiltinOutcome, ShellError> {
    // Spec: `"keyseq":shell-command`. The keyseq is a quoted inputrc
    // sequence; the shell-command runs via `execute_string` when the
    // key is pressed.
    let parsed = match parse_bind_x_spec(spec) {
        Some(p) => p,
        None => return Ok(BuiltinOutcome::Status(1)),
    };
    let mut ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    ctx.keymap
        .bind(&parsed.keyseq, KeymapEntry::ExecShell(parsed.command));
    Ok(BuiltinOutcome::Status(0))
}

fn do_apply_line(shell: &Shell, line: &[u8]) -> Result<BuiltinOutcome, ShellError> {
    let conditions = conditions_for(shell);
    let mut ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let report = inputrc::apply_line(line, &mut ctx, &conditions);
    inputrc::report_diagnostics(b"-", &report);
    if report.applied_lines == 0 {
        Ok(BuiltinOutcome::Status(1))
    } else {
        Ok(BuiltinOutcome::Status(0))
    }
}

struct BindXSpec {
    keyseq: Vec<u8>,
    command: Vec<u8>,
}

fn parse_bind_x_spec(spec: &[u8]) -> Option<BindXSpec> {
    let mut trimmed = trim_ws(spec);
    // Optional quoted LHS.
    let (keyseq, rest) = if trimmed.first() == Some(&b'"') {
        match crate::interactive::inputrc::escape::decode_quoted(&trimmed[1..]) {
            Ok((bytes, consumed)) => {
                trimmed = &trimmed[1 + consumed..];
                (bytes, trim_ws(trimmed))
            }
            Err(_) => return None,
        }
    } else {
        let colon = trimmed.iter().position(|&b| b == b':')?;
        let key_part =
            crate::interactive::inputrc::escape::decode_keyname(trim_ws(&trimmed[..colon])).ok()?;
        (key_part, &trimmed[colon..])
    };
    let colon_rest = rest.strip_prefix(b":")?;
    Some(BindXSpec {
        keyseq,
        command: trim_ws(colon_rest).to_vec(),
    })
}

fn parse_key_argument(key: &[u8]) -> Result<Vec<u8>, String> {
    use crate::interactive::inputrc::escape::{decode_escape, decode_keyname, decode_quoted};
    if key.first() == Some(&b'"') {
        let (bytes, _) = decode_quoted(&key[1..])?;
        return Ok(bytes);
    }
    // Unquoted arg: accept either a bash-style escape sequence
    // (`\C-x\C-r`) or a plain keyname (`C-a`, `Return`).
    if key.contains(&b'\\') {
        let mut out = Vec::with_capacity(key.len());
        let mut i = 0;
        while i < key.len() {
            if key[i] == b'\\' {
                let (bytes, step) = decode_escape(&key[i + 1..])?;
                out.extend_from_slice(&bytes);
                i += 1 + step;
            } else {
                out.push(key[i]);
                i += 1;
            }
        }
        Ok(out)
    } else {
        decode_keyname(key)
    }
}

fn trim_ws(bytes: &[u8]) -> &[u8] {
    let mut s = 0;
    let mut e = bytes.len();
    while s < e && matches!(bytes[s], b' ' | b'\t') {
        s += 1;
    }
    while e > s && matches!(bytes[e - 1], b' ' | b'\t') {
        e -= 1;
    }
    &bytes[s..e]
}

// Silence unused-code warnings when the file compiles into
// non-interactive builds.
#[allow(dead_code)]
const _REF: EmacsFn = EmacsFn::SelfInsert;
#[allow(dead_code)]
fn _keep_emacs_context_live(_: &EmacsContext) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn parse_bind_x_spec_accepts_quoted_key() {
        assert_no_syscalls(|| {
            let spec = parse_bind_x_spec(b"\"\\C-xg\": git status").unwrap();
            assert_eq!(spec.keyseq, vec![0x18, b'g']);
            assert_eq!(spec.command, b"git status");
        });
    }

    #[test]
    fn parse_bind_x_spec_rejects_malformed() {
        assert_no_syscalls(|| {
            assert!(parse_bind_x_spec(b"no colon").is_none());
        });
    }

    #[test]
    fn parse_key_argument_quoted_and_keyname() {
        assert_no_syscalls(|| {
            assert_eq!(parse_key_argument(b"\"\\C-a\"").unwrap(), vec![0x01]);
            assert_eq!(parse_key_argument(b"C-a").unwrap(), vec![0x01]);
        });
    }
}
