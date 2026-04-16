#![allow(unused_imports)]

use crate::shell::state::Shell;
use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
use crate::sys;
use crate::sys::test_support::{ArgMatcher, TraceEntry, TraceResult, t};

pub(super) fn parse_test(
    source: &str,
) -> Result<crate::syntax::ast::Program, crate::syntax::ParseError> {
    crate::syntax::parse(source.as_bytes())
}

pub(super) fn test_shell() -> Shell {
    crate::shell::test_support::test_shell()
}

pub(super) fn t_stderr(msg: &str) -> TraceEntry {
    t(
        "write",
        vec![
            ArgMatcher::Fd(sys::constants::STDERR_FILENO),
            ArgMatcher::Bytes(format!("{msg}\n").into_bytes()),
        ],
        TraceResult::Auto,
    )
}
