//! `$if` / `$else` / `$endif` / `$include` directive dispatching
//! (spec § 6).

#![allow(dead_code)]

use super::{Diagnostic, Mode, Report};

/// Per-file parser state: a stack of `$if` frames.
#[derive(Clone, Debug)]
pub(crate) struct ParserState {
    mode: Mode,
    stack: Vec<Frame>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Frame {
    IfActive,
    IfInactive,
    ElseActive,
    ElseInactive,
}

impl ParserState {
    pub(crate) fn new(mode: Mode) -> Self {
        Self {
            mode,
            stack: Vec::new(),
        }
    }

    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    /// True when directives on the current line should be applied
    /// (i.e. every frame on the stack is in its active branch).
    pub(crate) fn is_active(&self) -> bool {
        self.stack
            .iter()
            .all(|f| matches!(f, Frame::IfActive | Frame::ElseActive))
    }

    pub(crate) fn is_balanced(&self) -> bool {
        self.stack.is_empty()
    }
}

pub(crate) enum DirectiveOutcome {
    Handled,
    Include(Vec<u8>),
    NotRecognized,
}

pub(crate) fn dispatch_directive(
    rest_after_dollar: &[u8],
    lineno: usize,
    state: &mut ParserState,
    report: &mut Report,
) -> DirectiveOutcome {
    if let Some(test) = rest_after_dollar.strip_prefix(b"if ") {
        let test = trim_ws(test);
        let active = evaluate_if(test, state.mode);
        let active_in_context = state.is_active() && active;
        state.stack.push(if active_in_context {
            Frame::IfActive
        } else if !state.is_active() {
            Frame::IfInactive
        } else {
            Frame::IfInactive
        });
        if !active && state.is_active_parent() {
            // The test evaluated false at the top of its frame.
            // is_active() already folds the `Inactive` frames; just
            // emit a diagnostic if the test was unrecognized.
            if !is_recognized_test(test) {
                report.diagnostics.push(Diagnostic {
                    line: lineno,
                    message: format!("unknown $if test: {}", String::from_utf8_lossy(test)),
                });
            }
        }
        return DirectiveOutcome::Handled;
    }
    if rest_after_dollar == b"else" {
        match state.stack.last_mut() {
            Some(frame) => {
                *frame = match *frame {
                    Frame::IfActive => Frame::ElseInactive,
                    Frame::IfInactive => Frame::ElseActive,
                    Frame::ElseActive | Frame::ElseInactive => {
                        report.diagnostics.push(Diagnostic {
                            line: lineno,
                            message: "duplicate $else".to_string(),
                        });
                        *frame
                    }
                };
            }
            None => {
                report.diagnostics.push(Diagnostic {
                    line: lineno,
                    message: "$else without $if".to_string(),
                });
            }
        }
        return DirectiveOutcome::Handled;
    }
    if rest_after_dollar == b"endif" {
        if state.stack.pop().is_none() {
            report.diagnostics.push(Diagnostic {
                line: lineno,
                message: "$endif without $if".to_string(),
            });
        }
        return DirectiveOutcome::Handled;
    }
    if let Some(path) = rest_after_dollar.strip_prefix(b"include ") {
        return DirectiveOutcome::Include(trim_ws(path).to_vec());
    }
    if let Some(path) = rest_after_dollar.strip_prefix(b"include\t") {
        return DirectiveOutcome::Include(trim_ws(path).to_vec());
    }
    DirectiveOutcome::NotRecognized
}

impl ParserState {
    fn is_active_parent(&self) -> bool {
        // "active" including the current (just-pushed) frame ignored.
        if self.stack.len() <= 1 {
            return true;
        }
        self.stack[..self.stack.len() - 1]
            .iter()
            .all(|f| matches!(f, Frame::IfActive | Frame::ElseActive))
    }
}

fn evaluate_if(test: &[u8], mode: Mode) -> bool {
    match test {
        b"mode=emacs" => matches!(mode, Mode::Emacs),
        b"mode=vi" => matches!(mode, Mode::Vi),
        _ => false,
    }
}

fn is_recognized_test(test: &[u8]) -> bool {
    matches!(test, b"mode=emacs" | b"mode=vi")
}

fn trim_ws(bytes: &[u8]) -> &[u8] {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn if_emacs_active_in_emacs_mode() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            let r = dispatch_directive(b"if mode=emacs", 1, &mut state, &mut report);
            assert!(matches!(r, DirectiveOutcome::Handled));
            assert!(state.is_active());
            assert!(report.diagnostics.is_empty());
        });
    }

    #[test]
    fn if_vi_inactive_in_emacs_mode() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            dispatch_directive(b"if mode=vi", 1, &mut state, &mut report);
            assert!(!state.is_active());
        });
    }

    #[test]
    fn else_flips_active_branch() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            dispatch_directive(b"if mode=vi", 1, &mut state, &mut report);
            assert!(!state.is_active());
            dispatch_directive(b"else", 2, &mut state, &mut report);
            assert!(state.is_active());
            dispatch_directive(b"endif", 3, &mut state, &mut report);
            assert!(state.is_balanced());
        });
    }

    #[test]
    fn include_returns_path() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            match dispatch_directive(b"include /etc/xyz", 1, &mut state, &mut report) {
                DirectiveOutcome::Include(path) => assert_eq!(path, b"/etc/xyz"),
                _ => panic!("expected include"),
            }
        });
    }

    #[test]
    fn unknown_test_diagnosed_and_inactive() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            dispatch_directive(b"if term=xterm", 1, &mut state, &mut report);
            assert!(!state.is_active());
            assert_eq!(report.diagnostics.len(), 1);
        });
    }

    #[test]
    fn endif_without_if_is_diagnostic() {
        assert_no_syscalls(|| {
            let mut state = ParserState::new(Mode::Emacs);
            let mut report = Report::default();
            dispatch_directive(b"endif", 1, &mut state, &mut report);
            assert_eq!(report.diagnostics.len(), 1);
        });
    }
}
