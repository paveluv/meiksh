use crate::sys;

use super::glob::pattern_matches;
use super::model::is_glob_byte;

pub(super) fn expand_pathname(pattern: &[u8]) -> Vec<Vec<u8>> {
    if !pattern.iter().any(|&b| is_glob_byte(b)) {
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
    let mut matches = Vec::new();
    expand_path_segments(&base, &segments, 0, absolute, &mut matches);
    matches.sort();
    matches
}

pub(super) fn expand_path_segments(
    base: &[u8],
    segments: &[&[u8]],
    index: usize,
    absolute: bool,
    matches: &mut Vec<Vec<u8>>,
) {
    if index == segments.len() {
        let text = if absolute {
            base.to_vec()
        } else {
            if base.starts_with(b"./") && base.len() > 2 {
                base[2..].to_vec()
            } else {
                base.to_vec()
            }
        };
        matches.push(if text.is_empty() { b".".to_vec() } else { text });
        return;
    }

    let segment = segments[index];

    if !segment.iter().any(|&b| is_glob_byte(b)) {
        let next = path_join(base, segment);
        if sys::fs::file_exists(&next) {
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
        return;
    }

    let Ok(mut names) = sys::fs::read_dir_entries(base) else {
        return;
    };
    names.sort();
    for name in names {
        if name.starts_with(b".") && !segment.starts_with(b".") {
            continue;
        }
        if pattern_matches(&name, segment) {
            let next = path_join(base, &name);
            expand_path_segments(&next, segments, index + 1, absolute, matches);
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
