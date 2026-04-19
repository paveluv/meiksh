#![allow(unused_imports)]

use crate::shell::state::Shell;
use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
use crate::syntax::word_parts::WordPart;
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

/// Construct a `Word` whose `raw` is treated as a single literal
/// `WordPart` with no glob metacharacters and no embedded newlines. Use
/// in exec-path tests that need a valid AST-shape `Word` without going
/// through `syntax::parse`. The caller is responsible for only passing
/// plain bytes (no `*`, `?`, `[`, quoting, or expansion metacharacters).
pub(super) fn literal_word(raw: &[u8]) -> Word {
    let parts: Vec<WordPart> = vec![WordPart::Literal {
        start: 0,
        end: raw.len(),
        has_glob: false,
        newlines: 0,
    }];
    Word {
        raw: raw.to_vec(),
        parts,
        line: 0,
    }
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
