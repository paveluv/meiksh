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
use crate::interactive::inputrc::editline::{
    EditlineLookup, decode_editline_keyseq, translate_editline_function,
};
use crate::interactive::inputrc::{self, Conditions, Mode};
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
        // Positional form: either readline-style (one or more
        // `keyseq:function` strings) or editline-style (`keyseq
        // function-name`, two positional args, common on FreeBSD
        // `~/.shrc`). See `docs/features/emacs-editing-mode.md` § 14.5.
        _ => {
            let positional: Vec<&[u8]> = argv[1..].iter().map(|v| v.as_slice()).collect();
            dispatch_positional(shell, &positional)
        }
    }
}

/// Choose between the readline-style multi-arg form and the
/// editline/tcsh-style positional form, then delegate. See the
/// dispatch flowchart in `docs/features/emacs-editing-mode.md`
/// § 14.5.
fn dispatch_positional(
    shell: &mut Shell,
    positional: &[&[u8]],
) -> Result<BuiltinOutcome, ShellError> {
    // Caller guarantees at least one arg: the only call site is the
    // `_ =>` fallthrough in [`bind`], which has already consumed
    // `argv[1]` before delegating here.
    debug_assert!(
        !positional.is_empty(),
        "dispatch_positional called without any argument",
    );
    // Any `:` in a positional argument forces the readline-style
    // branch (bash handles each arg independently in this case).
    let any_colon = positional.iter().any(|a| a.contains(&b':'));
    if any_colon {
        return do_apply_lines(shell, positional);
    }
    // Without a colon, only the two-arg editline form is meaningful.
    if positional.len() == 2 {
        return do_apply_editline(shell, positional[0], positional[1]);
    }
    // Fallback: surface the error with readline's diagnostic wording
    // so we match bash's observable behaviour on malformed input.
    let line = positional[0].to_vec();
    do_apply_line(shell, &line)
}

/// Readline multi-arg form: `bind 'k1:f1' 'k2:f2' ...`. Each arg is
/// fed independently to [`inputrc::apply_line`]. Bash returns status
/// 0 here even when individual arguments produced diagnostics, so we
/// match that to keep parity with existing scripts.
fn do_apply_lines(shell: &Shell, args: &[&[u8]]) -> Result<BuiltinOutcome, ShellError> {
    let conditions = conditions_for(shell);
    let mut ctx = match inputrc::global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    for arg in args {
        let report = inputrc::apply_line(arg, &mut ctx, &conditions);
        inputrc::report_diagnostics(b"-", &report);
    }
    Ok(BuiltinOutcome::Status(0))
}

/// Editline/tcsh positional form: `bind <keyseq> <function-name>`.
/// Decodes the keyseq with editline's relaxed grammar (`^X`, `\E`
/// accepted alongside the standard readline escapes) and maps the
/// function name through the editline→readline translation table.
fn do_apply_editline(
    shell: &Shell,
    keyseq_arg: &[u8],
    func_arg: &[u8],
) -> Result<BuiltinOutcome, ShellError> {
    let seq = match decode_editline_keyseq(keyseq_arg) {
        Ok(bytes) => bytes,
        Err(msg) => {
            let mut full = b"bind: ".to_vec();
            full.extend_from_slice(msg.as_bytes());
            let _ = shell.diagnostic(1, &full);
            return Ok(BuiltinOutcome::Status(1));
        }
    };
    match translate_editline_function(func_arg) {
        EditlineLookup::Mapped(func) => {
            let mut ctx = match inputrc::global().lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            ctx.keymap.bind(&seq, KeymapEntry::Func(func));
            Ok(BuiltinOutcome::Status(0))
        }
        EditlineLookup::Unsupported => {
            let mut msg = b"bind: unsupported editline function: ".to_vec();
            msg.extend_from_slice(func_arg);
            let _ = shell.diagnostic(1, &msg);
            Ok(BuiltinOutcome::Status(1))
        }
        EditlineLookup::Unknown => {
            let mut msg = b"bind: unknown function: ".to_vec();
            msg.extend_from_slice(func_arg);
            let _ = shell.diagnostic(1, &msg);
            Ok(BuiltinOutcome::Status(1))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{invoke, test_shell};
    use crate::interactive::emacs_editing::keymap::Resolved;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    /// Snapshot + restore the global emacs context. Unit tests below
    /// mutate the global keymap through the real builtin path; resetting
    /// afterwards keeps subsequent tests in this module reading a
    /// deterministic table. A module-private mutex serialises the
    /// save/run/restore trio so parallel test threads don't stomp on
    /// each other's in-flight mutations.
    fn with_fresh_ctx<F: FnOnce()>(f: F) {
        use std::sync::Mutex;
        static SERIAL: Mutex<()> = Mutex::new(());
        let _lock = match SERIAL.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let saved = {
            let guard = match inputrc::global().lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            guard.keymap.clone()
        };
        f();
        let mut guard = match inputrc::global().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        guard.keymap = saved;
    }

    fn lookup(seq: &[u8]) -> Resolved {
        let guard = match inputrc::global().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        guard.keymap.resolve(seq)
    }

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

    #[test]
    fn editline_form_installs_binding() {
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"bind".to_vec(),
                        b"^[[A".to_vec(),
                        b"ed-search-prev-history".to_vec(),
                    ],
                )
                .expect("bind editline");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    lookup(&[0x1b, b'[', b'A']),
                    Resolved::Function(EmacsFn::HistorySearchBackward)
                ));
            });
        });
    }

    #[test]
    fn editline_form_accepts_mixed_readline_name() {
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"bind".to_vec(),
                        b"\\e[B".to_vec(),
                        b"history-search-forward".to_vec(),
                    ],
                )
                .expect("bind editline mixed");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    lookup(&[0x1b, b'[', b'B']),
                    Resolved::Function(EmacsFn::HistorySearchForward)
                ));
            });
        });
    }

    #[test]
    fn editline_unsupported_function_returns_status_1() {
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: unsupported editline function: vi-cmd-mode\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(
                        &mut shell,
                        &[b"bind".to_vec(), b"^[qz".to_vec(), b"vi-cmd-mode".to_vec()],
                    )
                    .expect("bind editline unsupported");
                    assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                    // Unsupported function must not install a binding
                    // (pick a sequence that is unbound by default).
                    assert!(matches!(lookup(&[0x1b, b'q', b'z']), Resolved::Unbound));
                },
            );
        });
    }

    #[test]
    fn editline_unknown_function_returns_status_1() {
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: unknown function: totally-made-up\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(
                        &mut shell,
                        &[
                            b"bind".to_vec(),
                            b"^[[A".to_vec(),
                            b"totally-made-up".to_vec(),
                        ],
                    )
                    .expect("bind editline unknown");
                    assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                },
            );
        });
    }

    #[test]
    fn trim_ws_strips_leading_and_trailing_spaces_and_tabs() {
        // `trim_ws` skips ASCII space / tab on both ends and keeps the
        // inner byte run verbatim.  The trailing-whitespace loop is
        // only exercised when `e > s && matches!(bytes[e-1], ' ' | '\t')`
        // — covered here by the trailing-tab input.
        assert_no_syscalls(|| {
            assert_eq!(trim_ws(b"  hello\t"), b"hello");
            assert_eq!(trim_ws(b"\t\t  "), b"");
            assert_eq!(trim_ws(b""), b"");
        });
    }

    #[test]
    fn parse_bind_x_spec_accepts_unquoted_keyname_form() {
        // `C-a:echo` uses the unquoted keyname branch (no leading `"`).
        // `decode_keyname` resolves `C-a` to `0x01`; the command bytes
        // are everything after the colon, trimmed.
        assert_no_syscalls(|| {
            let spec = parse_bind_x_spec(b"C-a:echo hello").expect("parse");
            assert_eq!(spec.keyseq, vec![0x01]);
            assert_eq!(spec.command, b"echo hello");
        });
    }

    #[test]
    fn parse_bind_x_spec_rejects_unterminated_quoted_key() {
        // An open quote with no closing `"` makes `decode_quoted`
        // return `Err`, which the spec parser must translate to `None`.
        assert_no_syscalls(|| {
            assert!(parse_bind_x_spec(b"\"unterminated").is_none());
        });
    }

    #[test]
    fn parse_bind_x_spec_missing_colon_in_unquoted_form_is_none() {
        // Unquoted path requires a `:` separator; without one, we must
        // return `None` so the caller surfaces status 1.
        assert_no_syscalls(|| {
            assert!(parse_bind_x_spec(b"C-a no-colon").is_none());
        });
    }

    #[test]
    fn bind_r_without_keyseq_reports_error() {
        // `bind -r` with no following argument must print a
        // `bind: -r requires a keyseq` diagnostic and return status 1
        // via the `ShellError::Status` wrapper.
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: -r requires a keyseq\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let err = invoke(&mut shell, &[b"bind".to_vec(), b"-r".to_vec()])
                        .expect_err("expected -r without key to error");
                    assert!(matches!(err, ShellError::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_f_without_filename_reports_error() {
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: -f requires a filename\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let err = invoke(&mut shell, &[b"bind".to_vec(), b"-f".to_vec()])
                        .expect_err("expected -f without file to error");
                    assert!(matches!(err, ShellError::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_x_without_argument_reports_error() {
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: -x requires an argument\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let err = invoke(&mut shell, &[b"bind".to_vec(), b"-x".to_vec()])
                        .expect_err("expected -x without spec to error");
                    assert!(matches!(err, ShellError::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_x_with_malformed_spec_returns_status_1() {
        // `parse_bind_x_spec` rejects input without a `:` separator;
        // `do_bind_x` must then surface status 1 without touching the
        // keymap.
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"bind".to_vec(), b"-x".to_vec(), b"no colon here".to_vec()],
                )
                .expect("bind -x malformed");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            });
        });
    }

    #[test]
    fn bind_r_with_invalid_key_argument_returns_status_1() {
        // `parse_key_argument` rejects an unknown keyname; the
        // `-r` branch must surface status 1 without a diagnostic.
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"bind".to_vec(), b"-r".to_vec(), b"not-a-key".to_vec()],
                )
                .expect("bind -r with bad key");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            });
        });
    }

    #[test]
    fn bind_positional_successful_readline_form_returns_status_0() {
        // A single positional arg with a valid `keyseq:function` line
        // reaches `do_apply_line`, which calls `inputrc::apply_line`.
        // A successful apply yields `applied_lines == 1` and the
        // builtin returns status 0 via the non-error branch.
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"bind".to_vec(), b"\"\\C-xq\": accept-line".to_vec()],
                )
                .expect("bind one-arg");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    lookup(&[0x18, b'q']),
                    Resolved::Function(EmacsFn::AcceptLine)
                ));
            });
        });
    }

    #[test]
    fn bind_f_with_missing_file_reports_status_1() {
        // `bind -f` on a nonexistent file runs the inputrc loader,
        // which emits an "unable to open" diagnostic and returns an
        // empty report with `applied_lines == 0`. The builtin must
        // surface status 1 in that case.
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![
                    realpath(_, _) -> err(crate::sys::constants::ENOENT),
                    open(_, _, _) -> err(crate::sys::constants::ENOENT),
                    write(
                        fd(crate::sys::constants::STDERR_FILENO),
                        bytes(b"meiksh: /nonexistent/meiksh-bind-test: cannot open: /nonexistent/meiksh-bind-test\n"),
                    ) -> auto,
                ],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(
                        &mut shell,
                        &[
                            b"bind".to_vec(),
                            b"-f".to_vec(),
                            b"/nonexistent/meiksh-bind-test".to_vec(),
                        ],
                    )
                    .expect("bind -f missing");
                    assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_editline_with_invalid_keyseq_returns_status_1() {
        // `decode_editline_keyseq` rejects a trailing `^` (dangling
        // control-escape). The `do_apply_editline` function emits a
        // `bind: ...` diagnostic and returns status 1.
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: bind: dangling `^` in key sequence\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(
                        &mut shell,
                        &[b"bind".to_vec(), b"^".to_vec(), b"accept-line".to_vec()],
                    )
                    .expect("bind editline bad keyseq");
                    assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_positional_single_colonless_arg_falls_through_to_apply_line() {
        // A lone positional arg without a colon can't match the
        // two-arg editline form, so `dispatch_positional` falls
        // through to `do_apply_line`. `inputrc::apply_line` cannot
        // parse a bare token (no colon), so `applied_lines == 0`
        // and the builtin reports status 1. The parser emits its
        // own "unrecognized command" diagnostic on stderr.
        with_fresh_ctx(|| {
            run_trace(
                trace_entries![write(
                    fd(crate::sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: -: line 1: missing `:` in binding\n"),
                ) -> auto,],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(
                        &mut shell,
                        &[b"bind".to_vec(), b"single-positional".to_vec()],
                    )
                    .expect("bind fallback");
                    assert!(matches!(outcome, BuiltinOutcome::Status(1)));
                },
            );
        });
    }

    #[test]
    fn bind_no_args_dumps_current_bindings() {
        // With no options or positional args, `bind` writes the current
        // inputrc dump to stdout (covers the None→dump_bindings path
        // in `bind` itself).
        with_fresh_ctx(|| {
            let expected = {
                let guard = match inputrc::global().lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let mut out = Vec::new();
                guard.keymap.dump_inputrc(&mut out);
                out
            };
            run_trace(
                trace_entries![write(fd(crate::sys::constants::STDOUT_FILENO), bytes(&expected)) -> auto],
                || {
                    let mut shell = test_shell();
                    let outcome = invoke(&mut shell, &[b"bind".to_vec()]).expect("bind");
                    assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                },
            );
        });
    }

    #[test]
    fn bind_single_colon_arg_applies_line_and_returns_zero() {
        // A single positional `keyseq:function` arg without other
        // arguments goes through dispatch_positional's readline-style
        // branch via the `any_colon` check, which leads to
        // do_apply_lines (status 0) — exercises the success exit at
        // line 250 of do_apply_line via the multi-line fallback and
        // also covers the readline-only path.
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"bind".to_vec(), b"\"\\C-xz\": accept-line".to_vec()],
                )
                .expect("bind single");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            });
        });
    }

    #[test]
    fn do_apply_line_success_returns_zero() {
        // Call do_apply_line directly with a well-formed readline line
        // so applied_lines > 0 → Status(0) path at line 250.
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let shell = test_shell();
                let outcome =
                    do_apply_line(&shell, b"\"\\C-xq\": accept-line").expect("apply_line");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            });
        });
    }

    #[test]
    fn multi_arg_readline_applies_each() {
        assert_no_syscalls(|| {
            with_fresh_ctx(|| {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"bind".to_vec(),
                        b"\"\\C-xa\": accept-line".to_vec(),
                        b"\"\\C-xb\": beginning-of-line".to_vec(),
                    ],
                )
                .expect("bind multi");
                // Bash returns 0 even on per-arg errors; we match.
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert!(matches!(
                    lookup(&[0x18, b'a']),
                    Resolved::Function(EmacsFn::AcceptLine)
                ));
                assert!(matches!(
                    lookup(&[0x18, b'b']),
                    Resolved::Function(EmacsFn::BeginningOfLine)
                ));
            });
        });
    }
}
