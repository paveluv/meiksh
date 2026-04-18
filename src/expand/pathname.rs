use std::ffi::CString;

use crate::sys;

use super::glob::pattern_matches;

/// Sort `entries` in the current locale's collating sequence, without
/// allocating on every comparison.
///
/// `readdir` already hands back NUL-terminated `d_name` buffers, which we
/// keep in `CString` form end-to-end through the glob pipeline. That lets
/// the comparator call `strcoll(3)` directly on the prebuilt C strings.
/// glibc's own `__strcoll_l` has a fast path that degrades to `strcmp` in
/// C-category locales (including `C.UTF-8`), so we do not need a separate
/// bytewise short-circuit here — the libc call is already cheap, as long
/// as we are not wrapping it in a fresh `CString` on every invocation.
///
/// Under `cfg(test)` we bypass libc entirely to keep the unit-test suite
/// deterministic and host-locale-independent; the byte order is identical
/// to `strcoll` for all ASCII test fixtures.
#[inline]
fn sort_cstrings(entries: &mut [CString]) {
    #[cfg(not(test))]
    {
        entries.sort_by(|a, b| {
            let r = unsafe { libc::strcoll(a.as_ptr(), b.as_ptr()) };
            r.cmp(&0)
        });
    }
    #[cfg(test)]
    {
        entries.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    }
}

/// True if `segment` contains at least one unquoted glob metacharacter that
/// POSIX 2.13.1 treats as active: `*`, `?`, or a well-formed `[...]`
/// bracket expression.
///
/// A lone `[` with no matching `]` in the same segment is NOT a valid
/// bracket expression under POSIX and must be treated as literal. The
/// previous byte-level `is_glob_char` unconditionally marked `[` as a
/// glob char, which caused a bare `[` command name (as used by the `test`
/// builtin's alternate form) to trigger a full `opendir`/`readdir` scan
/// of the current directory on every invocation. Profiling showed this
/// accounting for 61% of the deep-parse benchmark self-time.
///
/// The pairing is segment-local: a `/` separates path segments and a `]`
/// on the other side of `/` does not close a bracket expression on this
/// side. Callers pass each individual path segment into this function.
pub(crate) fn has_active_glob_meta(segment: &[u8]) -> bool {
    let mut i = 0;
    while i < segment.len() {
        match segment[i] {
            b'*' | b'?' => return true,
            b'[' => {
                // POSIX requires at least one character inside the bracket
                // expression (2.13.1: "[<range>]"). A well-formed form is
                // `[...]` with at least one byte between the brackets.
                // `[]` alone is not a bracket expression.
                if let Some(rel) = segment[i + 1..].iter().position(|&b| b == b']') {
                    if rel >= 1 {
                        return true;
                    }
                }
                // No matching `]` in this segment, or empty content —
                // treat this `[` as literal and keep scanning.
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}

pub(crate) fn expand_pathname(pattern: &[u8]) -> Vec<Vec<u8>> {
    if !has_active_glob_meta(pattern) {
        return vec![pattern.to_vec()];
    }
    let absolute = pattern.first() == Some(&b'/');
    let segments: Vec<&[u8]> = pattern
        .split(|&b| b == b'/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let base: Vec<u8> = if absolute {
        b"/".to_vec()
    } else {
        b".".to_vec()
    };
    // `matches` is collected as `CString` so that the final sort can call
    // `strcoll(3)` directly on the owned C strings with no per-comparison
    // allocation. We convert back to `Vec<u8>` at the very end; that step
    // is free because `CString::into_bytes` just drops the trailing NUL
    // without reallocating.
    let mut matches: Vec<CString> = Vec::new();
    // One scratch buffer shared across every recursive call; each use writes
    // a NUL-terminated path via `bstr::write_cstring_into`, eliminating the
    // per-candidate `CString` allocation that dominated the glob profile.
    let mut scratch: Vec<u8> = Vec::with_capacity(256);
    expand_path_segments(&base, &segments, 0, absolute, &mut matches, &mut scratch);
    sort_cstrings(&mut matches);
    matches.into_iter().map(|c| c.into_bytes()).collect()
}

pub(super) fn expand_path_segments(
    base: &[u8],
    segments: &[&[u8]],
    index: usize,
    absolute: bool,
    matches: &mut Vec<CString>,
    scratch: &mut Vec<u8>,
) {
    if index == segments.len() {
        let text = if absolute {
            base.to_vec()
        } else if base.starts_with(b"./") && base.len() > 2 {
            base[2..].to_vec()
        } else {
            base.to_vec()
        };
        let text = if text.is_empty() { b".".to_vec() } else { text };
        // SAFETY: `text` is a path we just assembled from `path_join`
        // results — it never contains interior NUL bytes. Constructing
        // the `CString` via `from_vec_unchecked` reuses `text`'s buffer
        // (pushing a single NUL) and avoids the extra scan + allocation
        // of the checked constructor.
        debug_assert!(!text.contains(&0));
        matches.push(unsafe { CString::from_vec_unchecked(text) });
        return;
    }

    let segment = segments[index];

    if !has_active_glob_meta(segment) {
        let next = path_join(base, segment);
        let exists = match crate::bstr::write_cstring_into(&next, scratch) {
            Ok(cstr) => sys::fs::file_exists_cstr(cstr),
            Err(_) => return,
        };
        if exists {
            expand_path_segments(&next, segments, index + 1, absolute, matches, scratch);
        }
        return;
    }

    let names_result = match crate::bstr::write_cstring_into(base, scratch) {
        Ok(cstr) => sys::fs::read_dir_entries_cstr(cstr),
        Err(_) => return,
    };
    let Ok(mut names) = names_result else {
        return;
    };
    sort_cstrings(&mut names);
    for name in &names {
        let bytes = name.as_bytes();
        if bytes.starts_with(b".") && !segment.starts_with(b".") {
            continue;
        }
        if pattern_matches(bytes, segment) {
            let next = path_join(base, bytes);
            expand_path_segments(&next, segments, index + 1, absolute, matches, scratch);
        }
    }
}

pub(super) fn path_join(base: &[u8], name: &[u8]) -> Vec<u8> {
    let mut result = base.to_vec();
    if !result.is_empty() && *result.last().unwrap() != b'/' {
        result.push(b'/');
    }
    result.extend_from_slice(name);
    result
}

#[cfg(test)]
mod tests {
    use crate::expand::core::Context;
    use crate::expand::test_support::{DefaultPathContext, FakeContext};
    use crate::expand::word::{expand_redirect_word, expand_word, expand_word_text};
    use crate::syntax::ast::Word;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn expands_text_without_field_splitting_or_pathname_expansion() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"WORDS".to_vec(), b"one two".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"$WORDS".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            b"one two"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"*".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .expect("expand"),
            b"*"
        );
    }

    #[test]
    fn performs_pathname_expansion() {
        let dir_entries = || {
            trace_entries![
                readdir(_) -> dir_entry(b"a.txt"),
                readdir(_) -> dir_entry(b"b.txt"),
                readdir(_) -> dir_entry(b".hidden.txt"),
                readdir(_) -> 0,
            ]
        };
        run_trace(
            trace_entries![
                access(str("/testdir"), _) -> 0,
                opendir(str("/testdir")) -> 1,
                ..dir_entries(),
                closedir(_) -> 0,
                access(str("/testdir"), _) -> 0,
                opendir(str("/testdir")) -> 1,
                ..dir_entries(),
                closedir(_) -> 0,
            ],
            || {
                let mut ctx = FakeContext::new();
                assert_eq!(
                    expand_word(
                        &mut ctx,
                        &Word {
                            raw: b"/testdir/*.txt".as_ref().into(),
                            parts: Box::new([]),
                            line: 0
                        },
                    )
                    .expect("glob"),
                    vec![b"/testdir/a.txt".as_ref(), b"/testdir/b.txt".as_ref()]
                );
                assert_eq!(
                    expand_word(
                        &mut ctx,
                        &Word {
                            raw: b"\\*.txt".as_ref().into(),
                            parts: Box::new([]),
                            line: 0
                        },
                    )
                    .expect("escaped glob"),
                    vec![b"*.txt".as_ref()]
                );
                assert_eq!(
                    expand_word(
                        &mut ctx,
                        &Word {
                            raw: b"/testdir/.*.txt".as_ref().into(),
                            parts: Box::new([]),
                            line: 0
                        },
                    )
                    .expect("hidden glob"),
                    vec![b"/testdir/.hidden.txt".as_ref()]
                );
            },
        );
    }

    #[test]
    fn can_disable_pathname_expansion_via_context() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let pattern = b"/testdir/*.txt";
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: pattern.as_ref().into(),
                        parts: Box::new([]),
                        line: 0
                    },
                )
                .expect("noglob"),
                vec![pattern.as_ref()]
            );
        });
    }

    #[test]
    fn default_pathname_context_trait_impl() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(ctx.special_param(b'?'), None);
        assert_eq!(ctx.positional_param(0).as_deref(), Some(b"meiksh".as_ref()));
        assert_eq!(ctx.positional_param(1), None);
        assert!(ctx.positional_params().is_empty());
        assert!(ctx.home_dir_for_user(b"nobody").is_none());
        assert!(!ctx.nounset_enabled());
        ctx.set_var(b"NAME", b"value").expect("set var");
        assert_eq!(ctx.env_var(b"NAME").as_deref(), Some(b"value".as_ref()));
        assert_eq!(ctx.shell_name(), b"meiksh");
        assert_eq!(
            ctx.command_substitute_raw(b"printf ok")
                .expect("substitute"),
            b"printf ok\n"
        );
    }

    #[test]
    fn unmatched_glob_returns_pattern_literally() {
        run_trace(
            trace_entries![opendir(_) -> err(crate::sys::constants::ENOENT)],
            || {
                let mut ctx = DefaultPathContext::new();
                assert_eq!(
                    expand_word(
                        &mut ctx,
                        &Word {
                            raw: b"*.definitely-no-match".as_ref().into(),
                            parts: Box::new([]),
                            line: 0
                        },
                    )
                    .expect("unmatched glob"),
                    vec![b"*.definitely-no-match".as_ref()]
                );
            },
        );
    }

    #[test]
    fn has_active_glob_meta_matches_posix_bracket_rules() {
        // The base star/question cases are unchanged from the previous
        // byte-level helper.
        assert!(super::has_active_glob_meta(b"*.txt"));
        assert!(super::has_active_glob_meta(b"a?c"));
        assert!(!super::has_active_glob_meta(b"plain.txt"));
        // Well-formed bracket expression: active.
        assert!(super::has_active_glob_meta(b"[abc]"));
        assert!(super::has_active_glob_meta(b"[a-z]"));
        assert!(super::has_active_glob_meta(b"foo[0-9]bar"));
        // Lone `[` must not trigger a directory scan. This is the regression
        // fix: before this change, bare `[` (as used by the `test` builtin's
        // alternate form) triggered a full readdir sweep on every call.
        assert!(!super::has_active_glob_meta(b"["));
        // Empty bracket `[]` is not a valid bracket expression.
        assert!(!super::has_active_glob_meta(b"[]"));
        // `]` without a `[` is literal.
        assert!(!super::has_active_glob_meta(b"]"));
        // `[` before a `]` later in the string still pairs; even if the
        // bracket contents are unusual, the cheap check errs on the side
        // of delegating to `pattern_matches` for correctness.
        assert!(super::has_active_glob_meta(b"[x]y"));
    }

    #[test]
    fn lone_bracket_command_does_not_opendir() {
        // This mirrors the hot path from the `test` builtin's alternate
        // form: the command name is a bare `[`, which was erroneously
        // triggering a full directory scan per invocation. With the new
        // helper, `expand_pathname(b"[")` must NOT call opendir/readdir.
        assert_no_syscalls(|| {
            let out = super::expand_pathname(b"[");
            assert_eq!(out, vec![b"[".to_vec()]);
        });
    }

    #[test]
    fn redirect_word_no_pathname_expansion() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"file_*.txt".as_ref().into(),
                    parts: Box::new([]),
                    line: 0,
                },
            )
            .expect("redirect word");
            assert_eq!(result, b"file_*.txt");
        });
    }
}
