//! Inputrc parser (spec `docs/features/inputrc.md`).
//!
//! Entry points:
//!
//! * [`load_from_path`] — read a file, parse each line, apply the
//!   accepted directives to the given [`EmacsContext`]. Non-fatal
//!   diagnostics are written to `stderr` in the
//!   `meiksh: <file>: line <n>: <msg>` format required by spec § 7.
//! * [`apply_line`] — parse a single line (used by the `bind` builtin
//!   one-argument form).
//!
//! The parser is split into submodules that correspond to the spec
//! sections: [`escape`] handles § 4.5 escapes, [`vars`] handles § 5,
//! [`keybind`] handles § 4, [`conditional`] handles § 6.1, and
//! [`include`] handles § 6.2.

#![allow(dead_code)]

pub(crate) mod conditional;
pub(crate) mod editline;
pub(crate) mod escape;
pub(crate) mod include;
pub(crate) mod keybind;
pub(crate) mod vars;

use std::sync::{Mutex, OnceLock};

use crate::sys;

use super::emacs_editing::keymap::{Keymap, KeymapEntry};

/// Global emacs-editor state: keymap and tuned variables. Modified by
/// `bind`, by startup inputrc loading, and consumed by the read-line
/// loop.
pub(crate) fn global() -> &'static Mutex<EmacsContext> {
    static CTX: OnceLock<Mutex<EmacsContext>> = OnceLock::new();
    CTX.get_or_init(|| Mutex::new(EmacsContext::new()))
}

/// State threaded through the parser so [`keybind`] and [`vars`] can
/// mutate the live keymap / variable table.
#[derive(Debug)]
pub(crate) struct EmacsContext {
    pub keymap: Keymap,
    pub vars: vars::InputrcVars,
    /// True once a startup inputrc has been consulted, so repeat
    /// invocations of [`ensure_startup_loaded`] don't re-read the
    /// file.
    pub startup_loaded: bool,
}

impl EmacsContext {
    pub(crate) fn new() -> Self {
        Self {
            keymap: Keymap::default_emacs(),
            vars: vars::InputrcVars::default(),
            startup_loaded: false,
        }
    }
}

impl Default for EmacsContext {
    fn default() -> Self {
        Self::new()
    }
}

/// A single diagnostic (non-fatal) emitted during parsing.
#[derive(Clone, Debug)]
pub(crate) struct Diagnostic {
    pub line: usize,
    pub message: String,
}

/// The outcome of parsing one or many lines.
#[derive(Clone, Debug, Default)]
pub(crate) struct Report {
    pub applied_lines: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// Active mode for `$if` evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Emacs,
    Vi,
}

/// Runtime conditions consulted by `$if` tests (spec § 6.1).
///
/// * `mode` — the editing mode the parser is being driven in.
/// * `term` — the value of the `TERM` environment variable at the
///   time parsing started, used to evaluate `$if term=<name>`. An
///   empty slice means `TERM` was unset, in which case every
///   `term=...` test evaluates false.
#[derive(Clone, Debug)]
pub(crate) struct Conditions {
    pub mode: Mode,
    pub term: Vec<u8>,
}

impl Conditions {
    pub(crate) fn new(mode: Mode, term: Vec<u8>) -> Self {
        Self { mode, term }
    }

    /// Convenience constructor for sites that don't need `$if term=`
    /// resolution (unit tests, the bind single-arg form when no
    /// shell is available).
    pub(crate) fn mode_only(mode: Mode) -> Self {
        Self {
            mode,
            term: Vec::new(),
        }
    }
}

/// Read `path` and apply its directives to `ctx`. Diagnostics are
/// returned in the [`Report`], with a one-based `line` number. The
/// function takes care of writing the human-readable
/// `meiksh: <path>: line <n>: <msg>` lines to stderr itself.
pub(crate) fn load_from_path(
    path: &[u8],
    ctx: &mut EmacsContext,
    conditions: &Conditions,
) -> Report {
    let mut guard = include::IncludeGuard::default();
    let mut report = Report::default();
    load_with_guard(path, ctx, conditions, &mut guard, &mut report);
    report
}

/// Parse and apply a single inputrc line (used by `bind single-arg`).
pub(crate) fn apply_line(line: &[u8], ctx: &mut EmacsContext, conditions: &Conditions) -> Report {
    let mut report = Report::default();
    let mut state = conditional::ParserState::new(conditions);
    parse_line(line, 1, ctx, &mut state, &mut report, None);
    report
}

/// Emit every diagnostic in `report` to stderr using the spec § 7
/// format.
pub(crate) fn report_diagnostics(file: &[u8], report: &Report) {
    for diag in &report.diagnostics {
        emit_diagnostic(file, diag);
    }
}

/// Lazily consult the startup inputrc. On first call, check
/// `$INPUTRC` then `$HOME/.inputrc` then `/etc/inputrc`, and parse the
/// first one that opens. Subsequent calls are no-ops.
pub(crate) fn ensure_startup_loaded(shell: &crate::shell::state::Shell) {
    let mut ctx = match global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if ctx.startup_loaded {
        return;
    }
    ctx.startup_loaded = true;
    let term = shell
        .get_var(b"TERM")
        .map(|b| b.to_vec())
        .unwrap_or_default();
    let conditions = Conditions::new(Mode::Emacs, term);
    let mut load = |path: Vec<u8>| -> bool {
        if path.is_empty() {
            return false;
        }
        if !sys::fs::file_exists(&path) {
            return false;
        }
        let report = load_from_path(&path, &mut ctx, &conditions);
        report_diagnostics(&path, &report);
        true
    };
    if let Some(p) = shell.get_var(b"INPUTRC").map(|b| b.to_vec()) {
        if load(p) {
            return;
        }
    }
    if let Some(home) = shell.get_var(b"HOME").map(|b| b.to_vec()) {
        let mut path = home;
        if !path.ends_with(b"/") {
            path.push(b'/');
        }
        path.extend_from_slice(b".inputrc");
        if load(path) {
            return;
        }
    }
    let _ = load(b"/etc/inputrc".to_vec());
}

pub(crate) fn load_with_guard(
    path: &[u8],
    ctx: &mut EmacsContext,
    conditions: &Conditions,
    guard: &mut include::IncludeGuard,
    report: &mut Report,
) {
    let canonical = sys::fs::canonicalize(path).unwrap_or_else(|_| path.to_vec());
    if !guard.enter(&canonical) {
        report.diagnostics.push(Diagnostic {
            line: 0,
            message: format!(
                "recursive $include detected: {}",
                String::from_utf8_lossy(path)
            ),
        });
        return;
    }
    let content = match sys::fs::read_file(path) {
        Ok(c) => c,
        Err(_) => {
            report.diagnostics.push(Diagnostic {
                line: 0,
                message: format!("cannot open: {}", String::from_utf8_lossy(path)),
            });
            guard.leave(&canonical);
            return;
        }
    };
    let mut state = conditional::ParserState::new(conditions);
    for (lineno, raw) in content.split(|b| *b == b'\n').enumerate() {
        parse_line(
            raw,
            lineno + 1,
            ctx,
            &mut state,
            report,
            Some((path, guard)),
        );
    }
    if !state.is_balanced() {
        report.diagnostics.push(Diagnostic {
            line: 0,
            message: "unterminated $if block".to_string(),
        });
    }
    guard.leave(&canonical);
}

fn parse_line(
    raw: &[u8],
    lineno: usize,
    ctx: &mut EmacsContext,
    state: &mut conditional::ParserState,
    report: &mut Report,
    include_host: Option<(&[u8], &mut include::IncludeGuard)>,
) {
    let trimmed = trim_whitespace(raw);
    if trimmed.is_empty() || trimmed.first() == Some(&b'#') {
        return;
    }
    if let Some(rest) = trimmed.strip_prefix(b"$") {
        match conditional::dispatch_directive(rest, lineno, state, report) {
            conditional::DirectiveOutcome::Handled => return,
            conditional::DirectiveOutcome::Include(path) => {
                if !state.is_active() {
                    return;
                }
                let (host, guard) = match include_host {
                    Some(x) => x,
                    None => {
                        report.diagnostics.push(Diagnostic {
                            line: lineno,
                            message: "$include not allowed here".to_string(),
                        });
                        return;
                    }
                };
                let resolved = include::resolve(host, &path);
                let nested = Conditions::new(state.mode(), state.term().to_vec());
                load_with_guard(&resolved, ctx, &nested, guard, report);
                return;
            }
            conditional::DirectiveOutcome::NotRecognized => {
                report.diagnostics.push(Diagnostic {
                    line: lineno,
                    message: format!("unknown directive: ${}", String::from_utf8_lossy(rest)),
                });
                return;
            }
        }
    }
    if !state.is_active() {
        return;
    }
    if trimmed.starts_with(b"set ") || trimmed.starts_with(b"set\t") {
        let remainder = trim_whitespace(&trimmed[3..]);
        match vars::parse_assignment(remainder, &mut ctx.vars) {
            Ok(()) => report.applied_lines += 1,
            Err(msg) => report.diagnostics.push(Diagnostic {
                line: lineno,
                message: msg,
            }),
        }
        return;
    }
    match keybind::parse(trimmed) {
        Ok((seq, entry)) => {
            ctx.keymap.bind(&seq, entry);
            report.applied_lines += 1;
        }
        Err(msg) => report.diagnostics.push(Diagnostic {
            line: lineno,
            message: msg,
        }),
    }
}

pub(crate) fn emit_diagnostic(file: &[u8], diag: &Diagnostic) {
    let mut msg = b"meiksh: ".to_vec();
    msg.extend_from_slice(file);
    if diag.line > 0 {
        msg.extend_from_slice(b": line ");
        msg.extend_from_slice(diag.line.to_string().as_bytes());
    }
    msg.extend_from_slice(b": ");
    msg.extend_from_slice(diag.message.as_bytes());
    msg.push(b'\n');
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
}

fn trim_whitespace(bytes: &[u8]) -> &[u8] {
    let mut s = 0;
    let mut e = bytes.len();
    while s < e && matches!(bytes[s], b' ' | b'\t' | b'\r') {
        s += 1;
    }
    while e > s && matches!(bytes[e - 1], b' ' | b'\t' | b'\r') {
        e -= 1;
    }
    &bytes[s..e]
}

/// Helper used by [`keybind`] to re-interpret a key-sequence macro RHS
/// as a bindable [`KeymapEntry::Macro`]; exposed here so both parsers
/// and the builtin can share it.
pub(crate) fn macro_entry(bytes: Vec<u8>) -> KeymapEntry {
    KeymapEntry::Macro(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn trim_whitespace_strips_both_sides() {
        assert_no_syscalls(|| {
            assert_eq!(trim_whitespace(b"  hi\t"), b"hi");
            assert_eq!(trim_whitespace(b"hi"), b"hi");
            assert_eq!(trim_whitespace(b"   "), b"");
        });
    }

    #[test]
    fn apply_line_set_known_bool() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let _ = apply_line(
                b"set completion-ignore-case on",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            assert!(ctx.vars.completion_ignore_case);
        });
    }

    #[test]
    fn apply_line_unknown_var_diagnoses() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let report = apply_line(
                b"set who-knows hello",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            // emit_diagnostic was called with a write to stderr; we
            // can't see that, but the report should contain the
            // diagnostic.
            assert_eq!(report.diagnostics.len(), 1);
            assert!(report.diagnostics[0].message.contains("unknown variable"));
        });
    }

    #[test]
    fn apply_line_binds_function_form() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let _ = apply_line(
                b"\"\\C-a\": end-of-line",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            use crate::interactive::emacs_editing::keymap::{EmacsFn, Resolved};
            assert_eq!(
                ctx.keymap.resolve(b"\x01"),
                Resolved::Function(EmacsFn::EndOfLine)
            );
        });
    }

    /// Parse a multi-line inputrc source. This drives the same logic
    /// as [`load_from_path`] but without touching the filesystem, so
    /// the unit tests stay `assert_no_syscalls`-clean.
    fn parse_source(src: &[u8], ctx: &mut EmacsContext, mode: Mode) -> Report {
        parse_source_with(src, ctx, &Conditions::mode_only(mode))
    }

    fn parse_source_with(src: &[u8], ctx: &mut EmacsContext, conditions: &Conditions) -> Report {
        let mut report = Report::default();
        let mut state = conditional::ParserState::new(conditions);
        for (lineno, raw) in src.split(|b| *b == b'\n').enumerate() {
            parse_line(raw, lineno + 1, ctx, &mut state, &mut report, None);
        }
        if !state.is_balanced() {
            report.diagnostics.push(Diagnostic {
                line: 0,
                message: "unterminated $if block".to_string(),
            });
        }
        report
    }

    #[test]
    fn unknown_variable_does_not_halt_parser() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let src = b"set who-knows off\nset completion-ignore-case on\n";
            let report = parse_source(src, &mut ctx, Mode::Emacs);
            assert!(ctx.vars.completion_ignore_case);
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("unknown variable")),
                "expected unknown-variable diagnostic: {report:?}"
            );
            assert_eq!(report.applied_lines, 1);
        });
    }

    #[test]
    fn if_mode_emacs_applies_and_vi_block_skipped() {
        assert_no_syscalls(|| {
            use crate::interactive::emacs_editing::keymap::{EmacsFn, Resolved};
            let mut ctx = EmacsContext::default();
            let src = b"$if mode=emacs\n\"\\C-a\": end-of-line\n$else\n\"\\C-a\": beginning-of-line\n$endif\n";
            let _ = parse_source(src, &mut ctx, Mode::Emacs);
            assert_eq!(
                ctx.keymap.resolve(b"\x01"),
                Resolved::Function(EmacsFn::EndOfLine),
                "emacs branch must win under mode=emacs"
            );
        });
    }

    #[test]
    fn if_mode_vi_skips_emacs_branch() {
        assert_no_syscalls(|| {
            use crate::interactive::emacs_editing::keymap::{EmacsFn, Resolved};
            let mut ctx = EmacsContext::default();
            let src = b"$if mode=vi\n\"\\C-a\": end-of-line\n$endif\n";
            let _ = parse_source(src, &mut ctx, Mode::Emacs);
            // The default emacs binding for C-a (beginning-of-line)
            // must remain untouched because the $if branch does not
            // execute under mode=emacs.
            assert_eq!(
                ctx.keymap.resolve(b"\x01"),
                Resolved::Function(EmacsFn::BeginningOfLine),
            );
        });
    }

    #[test]
    fn macro_binding_is_installed() {
        assert_no_syscalls(|| {
            use crate::interactive::emacs_editing::keymap::Resolved;
            let mut ctx = EmacsContext::default();
            let _ = apply_line(
                b"\"\\C-xg\": \"git status\"",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            match ctx.keymap.resolve(b"\x18g") {
                Resolved::Macro(bytes) => assert_eq!(bytes, b"git status"),
                other => panic!("expected macro, got {other:?}"),
            }
        });
    }

    #[test]
    fn rebind_overrides_previous_binding() {
        assert_no_syscalls(|| {
            use crate::interactive::emacs_editing::keymap::{EmacsFn, Resolved};
            let mut ctx = EmacsContext::default();
            let src = b"\"\\C-a\": end-of-line\n\"\\C-a\": kill-line\n";
            let _ = parse_source(src, &mut ctx, Mode::Emacs);
            assert_eq!(
                ctx.keymap.resolve(b"\x01"),
                Resolved::Function(EmacsFn::KillLine),
            );
        });
    }

    #[test]
    fn unterminated_if_block_reports_diagnostic() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let src = b"$if mode=emacs\n\"\\C-a\": end-of-line\n";
            let report = parse_source(src, &mut ctx, Mode::Emacs);
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("unterminated $if")),
                "expected unterminated-$if diagnostic: {report:?}"
            );
        });
    }

    #[test]
    fn apply_line_include_without_host_is_diagnostic() {
        // `apply_line` is used by `bind -f <line>` and has no file
        // host, so `$include` must be rejected at that site rather
        // than silently recursing.
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let report = apply_line(
                b"$include /etc/xyz",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            assert_eq!(report.diagnostics.len(), 1);
            assert!(
                report.diagnostics[0].message.contains("not allowed"),
                "got: {:?}",
                report.diagnostics[0].message
            );
        });
    }

    #[test]
    fn include_inside_inactive_if_is_silently_skipped() {
        // An `$include` nested under an inactive `$if` must be a
        // no-op: neither a diagnostic nor a filesystem read.
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let src = b"$if mode=vi\n$include /etc/nope\n$endif\n";
            let report = parse_source(src, &mut ctx, Mode::Emacs);
            assert!(
                report.diagnostics.is_empty(),
                "expected silent skip: {report:?}"
            );
        });
    }

    #[test]
    fn unknown_directive_is_diagnosed() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let src = b"$wibble\n";
            let report = parse_source(src, &mut ctx, Mode::Emacs);
            assert_eq!(report.diagnostics.len(), 1);
            assert!(
                report.diagnostics[0].message.contains("unknown directive"),
                "got: {:?}",
                report.diagnostics[0].message
            );
        });
    }

    #[test]
    fn macro_entry_wraps_bytes_in_keymap_entry() {
        assert_no_syscalls(|| {
            let e = macro_entry(b"ls -la".to_vec());
            match e {
                KeymapEntry::Macro(bytes) => assert_eq!(bytes, b"ls -la"),
                other => panic!("expected macro, got {other:?}"),
            }
        });
    }

    #[test]
    fn comment_and_blank_lines_are_ignored() {
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let src = b"# a comment\n\n   \n# another\n";
            let report = parse_source(src, &mut ctx, Mode::Emacs);
            assert_eq!(report.applied_lines, 0);
            assert!(report.diagnostics.is_empty());
        });
    }

    #[test]
    fn set_with_tab_separator_is_accepted() {
        // `set\tname value` must parse the same as `set name value`.
        assert_no_syscalls(|| {
            let mut ctx = EmacsContext::default();
            let _ = apply_line(
                b"set\tcompletion-ignore-case on",
                &mut ctx,
                &Conditions::mode_only(Mode::Emacs),
            );
            assert!(ctx.vars.completion_ignore_case);
        });
    }
}

#[cfg(test)]
mod io_tests {
    //! File-IO-driven paths in [`load_with_guard`]. These use the
    //! `run_trace` syscall harness instead of `assert_no_syscalls`.

    use super::*;
    use crate::sys::constants::ENOENT;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn cannot_open_path_reports_diagnostic() {
        run_trace(
            trace_entries![
                realpath(_, _) -> err(ENOENT),
                open(_, _, _) -> err(ENOENT),
            ],
            || {
                let mut ctx = EmacsContext::default();
                let report =
                    load_from_path(b"/etc/nope", &mut ctx, &Conditions::mode_only(Mode::Emacs));
                assert!(
                    report
                        .diagnostics
                        .iter()
                        .any(|d| d.message.contains("cannot open")),
                    "expected cannot-open diagnostic: {report:?}"
                );
            },
        );
    }

    #[test]
    fn unterminated_if_block_through_load_from_path_reports_diagnostic() {
        // Content leaves the parser state unbalanced so the
        // load_with_guard tail emits the "unterminated $if" bullet.
        let content = b"$if mode=emacs\n\"\\C-a\": end-of-line\n";
        run_trace(
            trace_entries![
                realpath(_, _) -> realpath("/tmp/rc"),
                open(_, _, _) -> fd(3),
                read(fd(3), _) -> bytes(content),
                read(fd(3), _) -> 0,
                close(fd(3)) -> 0,
            ],
            || {
                let mut ctx = EmacsContext::default();
                let report =
                    load_from_path(b"/tmp/rc", &mut ctx, &Conditions::mode_only(Mode::Emacs));
                assert!(
                    report
                        .diagnostics
                        .iter()
                        .any(|d| d.message.contains("unterminated $if")),
                    "expected unterminated-$if diagnostic: {report:?}"
                );
            },
        );
    }
}
