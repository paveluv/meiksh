//! AST-build-time assignment-expansion logic.
//!
//! At parse time, `SimpleCommand` construction routes each argv word
//! through this module when the command name is lexically a POSIX
//! declaration utility (`export`, `readonly`) or a `command`-prefixed
//! form thereof. `apply_assignment_context_to_argv_word`
//! rewrites the word's parts in place to:
//!
//! 1. Mark the `NAME=` prefix's `Literal` with `assignment: true`, so the
//!    declaration-utility expander can split argv tokens without
//!    re-parsing.
//! 2. Emit `TildeLiteral`s at assignment-context positions (after the `=`
//!    and after each unquoted unescaped `:` in the value).
//!
//! `build_assignment_value_parts` is the matching emitter for real
//! assignments (`NAME=VALUE cmd`), producing value parts with the same
//! tilde rules.

use super::ast::Word;
use super::byte_class::{is_glob_char, is_name_cont, is_name_start};
use super::token;
use super::word_part::WordPart;

/// POSIX special built-ins that take assignment-word arguments. This
/// is the single source of truth for the declaration-utility set;
/// both the argv-rewrite (this module) and the
/// `SimpleCommand::declaration_context` flag (set by
/// `apply_declaration_utility_rewrite` in `super::ast`) derive from
/// it. The executor reads only the flag and never re-walks the argv.
/// Non-POSIX shells commonly extend the set with `local`, `declare`,
/// and `typeset`; add them here only if/when the exec side implements
/// them.
const DECLARATION_UTILITIES: &[&[u8]] = &[b"export", b"readonly"];

pub(super) fn is_declaration_utility(name_word: &Word) -> bool {
    let Some(bytes) = literal_only_bytes(name_word) else {
        return false;
    };
    DECLARATION_UTILITIES.iter().any(|&n| n == bytes)
}

pub(super) fn is_command_utility(name_word: &Word) -> bool {
    matches!(literal_only_bytes(name_word), Some(b) if b == b"command")
}

/// Walk past a (possibly nested) `command` utility prefix in a simple
/// command's argv. Returns the index of the first argument to rewrite
/// if a declaration-utility name sits immediately after the last
/// `command` token. Returns `None` for any non-literal argv (e.g.
/// `command $var`), `--` (POSIX-unspecified), an unrecognized trailing
/// name, or empty argv after `command`.
pub(super) fn find_command_decl_util_boundary(words: &[Word]) -> Option<usize> {
    let mut i = 1;
    loop {
        let arg = words.get(i)?;
        let bytes = literal_only_bytes(arg)?;
        if bytes == b"--" {
            return None;
        }
        if bytes == b"command" {
            i += 1;
            continue;
        }
        if DECLARATION_UTILITIES.iter().any(|&n| n == bytes) {
            return Some(i + 1);
        }
        return None;
    }
}

/// Build the `WordPart` list for the VALUE portion of a real assignment
/// `NAME=VALUE cmd`. Emits `TildeLiteral` at slice start and after each
/// unquoted unescaped `:`.
pub(super) fn build_assignment_value_parts(value_raw: &[u8]) -> Vec<WordPart> {
    token::build_word_parts_for_slice(value_raw, 0, value_raw.len(), 0, true)
}

/// Rewrite `word.parts` in place to encode assignment-context expansion
/// for a declaration-utility argv token. No-op if `word` isn't
/// `NAME=...`-shaped.
pub(super) fn apply_assignment_context_to_argv_word(word: &mut Word) {
    let Some(after_eq) = detect_name_equals_prefix(word) else {
        return;
    };
    // detect_name_equals_prefix guarantees parts[0] is a Literal whose
    // byte range covers `[start..=after_eq-1]` at minimum.
    let (lit_start, lit_end) = match word.parts[0] {
        WordPart::Literal { start, end, .. } => (start, end),
        _ => unreachable!("detect_name_equals_prefix ensures parts[0] is Literal"),
    };

    let left_span = &word.raw[lit_start..after_eq];
    let left_glob = left_span.iter().any(|&b| is_glob_char(b));
    let left_nl = left_span.iter().filter(|&&b| b == b'\n').count() as u16;
    let left = WordPart::Literal {
        start: lit_start,
        end: after_eq,
        has_glob: left_glob,
        newlines: left_nl,
        assignment: true,
    };

    // Re-tokenize the byte range after the `=` with assignment-context
    // tilde rules. We re-tokenize from `word.raw[after_eq..]` (not from
    // `lit_end..`) so that any literal bytes between `=` and the end of
    // the original Literal become the head of the new tail stream, and
    // subsequent parts (Expansion / QuotedLiteral / ...) are rebuilt
    // verbatim by the same tokenizer that produced them originally.
    let tail = token::build_word_parts_for_slice(&word.raw, after_eq, word.raw.len(), 0, true);
    debug_assert!(lit_end <= word.raw.len());

    let mut new_parts = Vec::with_capacity(1 + tail.len());
    new_parts.push(left);
    new_parts.extend(tail);
    word.parts = new_parts;
}

/// Return the byte offset in `word.raw` immediately past the `=` iff
/// `word` is `NAME=...`-shaped: parts[0] is a `Literal` whose bytes
/// contain a non-empty POSIX-valid NAME followed by `=`.
fn detect_name_equals_prefix(word: &Word) -> Option<usize> {
    let first = word.parts.first()?;
    let WordPart::Literal { start, end, .. } = *first else {
        return None;
    };
    let span = &word.raw[start..end];
    let eq = span.iter().position(|&b| b == b'=')?;
    if eq == 0 {
        return None;
    }
    let name = &span[..eq];
    if !is_posix_name(name) {
        return None;
    }
    Some(start + eq + 1)
}

fn is_posix_name(bytes: &[u8]) -> bool {
    !bytes.is_empty() && is_name_start(bytes[0]) && bytes[1..].iter().all(|&b| is_name_cont(b))
}

/// Return the full `word.raw` slice iff `word` consists of a single
/// plain `Literal` that spans `0..raw.len()` with no glob bytes. Used
/// by the declaration-utility and `command`-utility recognition
/// helpers: only lexically unambiguous command names trigger the
/// argv rewrite.
fn literal_only_bytes(word: &Word) -> Option<&[u8]> {
    let [
        WordPart::Literal {
            start,
            end,
            has_glob: false,
            ..
        },
    ] = word.parts.as_slice()
    else {
        return None;
    };
    if *start != 0 || *end != word.raw.len() {
        return None;
    }
    Some(&word.raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::syntax::word_part::ExpansionKind;
    use crate::sys::test_support::assert_no_syscalls;

    fn word(raw: &[u8]) -> Word {
        let prog = crate::syntax::parse(&[b"cmd ".as_ref(), raw, b"\n"].concat())
            .expect("parse test word");
        match &prog.items[0].and_or.first.commands[0] {
            crate::syntax::ast::Command::Simple(sc) => sc.words[1].clone(),
            _ => panic!("expected simple command"),
        }
    }

    fn first_cmd(src: &[u8]) -> crate::syntax::ast::SimpleCommand {
        let prog = crate::syntax::parse(src).expect("parse");
        match &prog.items[0].and_or.first.commands[0] {
            crate::syntax::ast::Command::Simple(sc) => sc.clone(),
            _ => panic!("expected simple command"),
        }
    }

    #[test]
    fn is_declaration_utility_matches_canonical_set() {
        assert_no_syscalls(|| {
            for name in [&b"export"[..], b"readonly"] {
                assert!(is_declaration_utility(&word(name)));
            }
            // Non-POSIX extensions (`local`, `declare`, `typeset`) are
            // intentionally excluded — we don't implement them on the
            // exec side yet, so marking their argv as assignments would
            // desync the two sides.
            for name in [&b"local"[..], b"declare", b"typeset"] {
                assert!(!is_declaration_utility(&word(name)));
            }
            assert!(!is_declaration_utility(&word(b"exportx")));
            assert!(!is_declaration_utility(&word(b"Export")));
            assert!(!is_declaration_utility(&word(b"ls")));
            // Quoted name should not match.
            assert!(!is_declaration_utility(&word(b"\"export\"")));
        });
    }

    #[test]
    fn is_command_utility_matches_only_plain_command() {
        assert_no_syscalls(|| {
            assert!(is_command_utility(&word(b"command")));
            assert!(!is_command_utility(&word(b"Command")));
            assert!(!is_command_utility(&word(b"\"command\"")));
        });
    }

    #[test]
    fn find_command_decl_util_boundary_peeks_nested_command() {
        assert_no_syscalls(|| {
            let sc = first_cmd(b"command command export A=1\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), Some(3));
        });
    }

    #[test]
    fn find_command_decl_util_boundary_single_prefix() {
        assert_no_syscalls(|| {
            let sc = first_cmd(b"command export A=1\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), Some(2));
        });
    }

    #[test]
    fn find_command_decl_util_boundary_stops_at_double_dash() {
        assert_no_syscalls(|| {
            let sc = first_cmd(b"command -- export A=1\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), None);
        });
    }

    #[test]
    fn find_command_decl_util_boundary_non_decl_returns_none() {
        assert_no_syscalls(|| {
            let sc = first_cmd(b"command echo hi\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), None);
        });
    }

    #[test]
    fn find_command_decl_util_boundary_bare_command_returns_none() {
        assert_no_syscalls(|| {
            let sc = first_cmd(b"command\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), None);
        });
    }

    #[test]
    fn find_command_decl_util_boundary_non_literal_returns_none() {
        assert_no_syscalls(|| {
            // "$var" is an Expansion, not a plain Literal.
            let sc = first_cmd(b"command $var A=1\n");
            assert_eq!(find_command_decl_util_boundary(&sc.words), None);
        });
    }

    #[test]
    fn declaration_context_flag_matches_utility_shape() {
        assert_no_syscalls(|| {
            for src in [
                &b"export A=1\n"[..],
                b"readonly A=1\n",
                b"command export A=1\n",
                b"command command readonly A=1\n",
            ] {
                assert!(
                    first_cmd(src).declaration_context,
                    "expected declaration_context for {}",
                    String::from_utf8_lossy(src)
                );
            }
            // Negative cases: a non-declaration command name, a bare
            // `command`, `command --` (POSIX-unspecified), a non-
            // declaration target after `command`, a real leading
            // assignment, and any option flags between `command` and
            // the target — the parser is conservative and only rewrites
            // when the lexical shape is unambiguous.
            for src in [
                &b"echo A=1\n"[..],
                b"command\n",
                b"command -- export A=1\n",
                b"command echo hi\n",
                b"command -v export A=1\n",
                b"A=1 echo hi\n",
            ] {
                assert!(
                    !first_cmd(src).declaration_context,
                    "expected no declaration_context for {}",
                    String::from_utf8_lossy(src)
                );
            }
        });
    }

    #[test]
    fn apply_sets_assignment_flag_and_splits_prefix() {
        assert_no_syscalls(|| {
            let mut w = word(b"A=foo");
            apply_assignment_context_to_argv_word(&mut w);
            assert_eq!(w.parts.len(), 2);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    start: 0,
                    end: 2,
                    assignment: true,
                    ..
                }
            ));
            assert!(matches!(
                w.parts[1],
                WordPart::Literal {
                    start: 2,
                    end: 5,
                    assignment: false,
                    ..
                }
            ));
        });
    }

    #[test]
    fn apply_emits_tilde_literal_after_equals() {
        assert_no_syscalls(|| {
            let mut w = word(b"A=~/x");
            apply_assignment_context_to_argv_word(&mut w);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    assignment: true,
                    ..
                }
            ));
            assert!(matches!(w.parts[1], WordPart::TildeLiteral { .. }));
        });
    }

    #[test]
    fn apply_emits_tilde_after_unquoted_colon() {
        assert_no_syscalls(|| {
            let mut w = word(b"PATH=~/bin:~/scripts");
            apply_assignment_context_to_argv_word(&mut w);
            let tilde_count = w
                .parts
                .iter()
                .filter(|p| matches!(p, WordPart::TildeLiteral { .. }))
                .count();
            assert_eq!(tilde_count, 2);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    assignment: true,
                    ..
                }
            ));
        });
    }

    #[test]
    fn apply_does_not_touch_tilde_in_quoted_literal() {
        assert_no_syscalls(|| {
            let mut w = word(b"A=\"~\"/x");
            apply_assignment_context_to_argv_word(&mut w);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    assignment: true,
                    ..
                }
            ));
            // Tail must not contain a TildeLiteral.
            assert!(
                !w.parts
                    .iter()
                    .any(|p| matches!(p, WordPart::TildeLiteral { .. }))
            );
        });
    }

    #[test]
    fn apply_is_noop_when_name_is_invalid() {
        assert_no_syscalls(|| {
            // leading digit is not a valid NAME
            let mut w = word(b"1A=foo");
            let before = w.parts.clone();
            apply_assignment_context_to_argv_word(&mut w);
            assert_eq!(w.parts, before);
        });
    }

    #[test]
    fn apply_is_noop_when_no_equals() {
        assert_no_syscalls(|| {
            let mut w = word(b"foo");
            let before = w.parts.clone();
            apply_assignment_context_to_argv_word(&mut w);
            assert_eq!(w.parts, before);
        });
    }

    #[test]
    fn apply_is_noop_when_equals_is_quoted() {
        assert_no_syscalls(|| {
            // `A"="b` → parts[0] = Literal "A" only, "=" is QuotedLiteral.
            let mut w = word(b"A\"=\"b");
            let before = w.parts.clone();
            apply_assignment_context_to_argv_word(&mut w);
            assert_eq!(w.parts, before);
        });
    }

    #[test]
    fn apply_preserves_dollar_expansion_in_value() {
        assert_no_syscalls(|| {
            let mut w = word(b"A=$x:~/y");
            apply_assignment_context_to_argv_word(&mut w);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    assignment: true,
                    ..
                }
            ));
            assert!(matches!(
                w.parts[1],
                WordPart::Expansion {
                    kind: ExpansionKind::SimpleVar { .. },
                    ..
                }
            ));
            assert!(
                w.parts
                    .iter()
                    .any(|p| matches!(p, WordPart::TildeLiteral { .. }))
            );
        });
    }

    #[test]
    fn apply_keeps_value_equals_as_literal() {
        assert_no_syscalls(|| {
            // `A=B=C` → NAME is "A", value is "B=C".
            let mut w = word(b"A=B=C");
            apply_assignment_context_to_argv_word(&mut w);
            assert!(matches!(
                w.parts[0],
                WordPart::Literal {
                    start: 0,
                    end: 2,
                    assignment: true,
                    ..
                }
            ));
            // Exactly one `assignment: true` literal regardless of
            // trailing `=` in the value.
            let flagged = w
                .parts
                .iter()
                .filter(|p| {
                    matches!(
                        p,
                        WordPart::Literal {
                            assignment: true,
                            ..
                        }
                    )
                })
                .count();
            assert_eq!(flagged, 1);
        });
    }

    #[test]
    fn detect_name_equals_prefix_non_literal_returns_none() {
        // A word whose first part is not a `Literal` (e.g. starts with
        // a `$var` expansion) must not be picked up as `NAME=...`.
        assert_no_syscalls(|| {
            assert!(detect_name_equals_prefix(&word(b"$x=1")).is_none());
        });
    }

    #[test]
    fn detect_name_equals_prefix_empty_name_returns_none() {
        // `=foo` has `eq == 0` — the helper rejects it.
        assert_no_syscalls(|| {
            assert!(detect_name_equals_prefix(&word(b"=foo")).is_none());
        });
    }

    #[test]
    fn literal_only_bytes_rejects_partial_literal_span() {
        // `literal_only_bytes` requires the single-`Literal` slice
        // pattern *and* `start == 0 && end == raw.len()`.  The
        // latter guard is the one on line 160 — hit it directly by
        // constructing a Word whose lone Literal covers a strict
        // subset of `raw`.
        assert_no_syscalls(|| {
            let w = Word {
                raw: b"foobar".to_vec(),
                parts: vec![WordPart::Literal {
                    start: 0,
                    end: 3,
                    has_glob: false,
                    newlines: 0,
                    assignment: false,
                }],
                line: 1,
            };
            assert!(literal_only_bytes(&w).is_none());
        });
    }

    #[test]
    fn build_assignment_value_parts_produces_tilde_for_path_like() {
        assert_no_syscalls(|| {
            let parts = build_assignment_value_parts(b"~/bin:~/scripts");
            let tilde_count = parts
                .iter()
                .filter(|p| matches!(p, WordPart::TildeLiteral { .. }))
                .count();
            assert_eq!(tilde_count, 2);
        });
    }
}
