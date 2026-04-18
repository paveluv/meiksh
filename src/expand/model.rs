#[cfg(test)]
use crate::syntax::byte_class::{is_ascii_ws, is_glob_char};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuoteState {
    Quoted,
    Literal,
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Segment {
    Text(Vec<u8>, QuoteState),
    AtBreak,
    AtEmpty,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Expansion {
    One(Vec<u8>),
    Static(&'static [u8]),
    AtFields(Vec<Vec<u8>>),
}

#[derive(Debug)]
pub(super) struct ExpandedWord {
    pub(super) segments: Vec<Segment>,
    pub(super) had_quoted_content: bool,
    pub(super) had_quoted_null_outside_at: bool,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Field {
    pub(super) text: Vec<u8>,
    pub(super) has_unquoted_glob: bool,
}

#[cfg(test)]
pub(super) fn segment_bytes(segments: &[Segment]) -> impl Iterator<Item = (u8, QuoteState)> + '_ {
    segments
        .iter()
        .flat_map(|seg| match seg {
            Segment::Text(text, state) => {
                let s = *state;
                Some(text.iter().map(move |&b| (b, s)))
            }
            _ => None,
        })
        .flatten()
}

#[cfg(test)]
pub(super) fn split_fields_from_segments(segments: &[Segment], ifs: &[u8]) -> Vec<Field> {
    if ifs.is_empty() {
        return vec![Field {
            text: flatten_segments(segments),
            has_unquoted_glob: segments.iter().any(|seg| {
                matches!(seg, Segment::Text(text, state) if *state != QuoteState::Quoted && text.iter().any(|&b| is_glob_char(b)))
            }),
        }];
    }

    let ifs_ws: Vec<u8> = ifs.iter().copied().filter(|b| is_ascii_ws(*b)).collect();
    let ifs_other: Vec<u8> = ifs.iter().copied().filter(|b| !is_ascii_ws(*b)).collect();
    let mut iter = segment_bytes(segments).peekable();

    let mut fields = Vec::new();
    let mut current = Vec::new();
    let mut current_glob = false;

    while let Some((b, state)) = iter.next() {
        let splittable = state == QuoteState::Expanded;
        if splittable && ifs_other.contains(&b) {
            fields.push(Field {
                text: std::mem::take(&mut current),
                has_unquoted_glob: current_glob,
            });
            current_glob = false;
            while iter
                .peek()
                .is_some_and(|&(pb, ps)| ps == QuoteState::Expanded && ifs_ws.contains(&pb))
            {
                iter.next();
            }
            continue;
        }
        if splittable && ifs_ws.contains(&b) {
            if !current.is_empty() {
                fields.push(Field {
                    text: std::mem::take(&mut current),
                    has_unquoted_glob: current_glob,
                });
                current_glob = false;
            }
            while iter
                .peek()
                .is_some_and(|&(pb, ps)| ps == QuoteState::Expanded && ifs_ws.contains(&pb))
            {
                iter.next();
            }
            continue;
        }
        current_glob |= state != QuoteState::Quoted && is_glob_char(b);
        current.push(b);
    }

    if !current.is_empty() {
        fields.push(Field {
            text: current,
            has_unquoted_glob: current_glob,
        });
    }

    fields
}

pub(super) fn push_segment(segments: &mut Vec<Segment>, text: Vec<u8>, state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.extend_from_slice(&text);
            return;
        }
    }
    segments.push(Segment::Text(text, state));
}

pub(super) fn push_segment_slice(segments: &mut Vec<Segment>, text: &[u8], state: QuoteState) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_state)) = segments.last_mut() {
        if *last_state == state {
            last.extend_from_slice(text);
            return;
        }
    }
    segments.push(Segment::Text(text.to_vec(), state));
}

pub(super) fn flatten_segments(segments: &[Segment]) -> Vec<u8> {
    let total: usize = segments
        .iter()
        .map(|s| match s {
            Segment::Text(part, _) => part.len(),
            _ => 0,
        })
        .sum();
    let mut result = Vec::with_capacity(total);
    for seg in segments {
        if let Segment::Text(part, _) = seg {
            result.extend_from_slice(part);
        }
    }
    result
}

pub(super) fn render_pattern_from_segments(segments: &[Segment]) -> Vec<u8> {
    let mut pattern = Vec::new();
    for seg in segments {
        if let Segment::Text(text, state) = seg {
            if *state == QuoteState::Quoted {
                for &b in text.iter() {
                    pattern.push(b'\\');
                    pattern.push(b);
                }
            } else {
                pattern.extend_from_slice(text);
            }
        }
    }
    pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::glob::{match_bracket, pattern_matches};
    use crate::expand::pathname::{expand_path_segments, expand_pathname};
    use crate::expand::test_support::FakeContext;
    use crate::expand::word::{
        expand_here_document, expand_redirect_word, expand_word, flatten_expansion,
    };
    use crate::syntax::ast::Word;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn performs_field_splitting_more_like_posix() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$WORDS".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"one".as_ref(), b"two".as_ref(), b"three".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$DELIMS".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            vec![b"".as_ref() as &[u8], b"", b""]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$EMPTY".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                },
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert!(split_fields_from_segments(&[], b" \t\n").is_empty());
    }

    #[test]
    fn field_and_pattern_helpers_cover_corner_cases() {
        run_trace(
            trace_entries![opendir(_) -> err(crate::sys::constants::ENOENT)],
            || {
                let segs = vec![Segment::Text(b"*.txt".to_vec(), QuoteState::Expanded)];
                assert_eq!(
                    split_fields_from_segments(&segs, b""),
                    vec![Field {
                        text: b"*.txt".to_vec(),
                        has_unquoted_glob: true,
                    }]
                );

                assert_eq!(
                    split_fields_from_segments(
                        &[Segment::Text(
                            b"alpha,  beta".to_vec(),
                            QuoteState::Expanded
                        )],
                        b" ,"
                    ),
                    vec![
                        Field {
                            text: b"alpha".to_vec(),
                            has_unquoted_glob: false,
                        },
                        Field {
                            text: b"beta".to_vec(),
                            has_unquoted_glob: false,
                        },
                    ]
                );

                assert_eq!(expand_pathname(b"plain.txt"), vec![b"plain.txt".to_vec()]);

                let mut matches: Vec<std::ffi::CString> = Vec::new();
                let mut scratch = Vec::new();
                expand_path_segments(
                    b"/definitely/not/a/real/dir",
                    &[b"*.txt".as_ref()],
                    0,
                    false,
                    &mut matches,
                    &mut scratch,
                );
                assert!(matches.is_empty());

                let mut matches: Vec<std::ffi::CString> = Vec::new();
                expand_path_segments(b".", &[], 0, false, &mut matches, &mut scratch);
                assert_eq!(
                    matches,
                    vec![std::ffi::CString::new(b".".to_vec()).unwrap()]
                );

                assert!(pattern_matches(b"x", b"?"));
                assert!(pattern_matches(b"[", b"["));
                assert!(pattern_matches(b"]", b"\\]"));
                assert!(pattern_matches(b"b", b"[a-c]"));
                assert!(pattern_matches(b"d", b"[!a-c]"));

                assert!(pattern_matches(b"a", b"[[:alpha:]]"));
                assert!(pattern_matches(b"Z", b"[[:alpha:]]"));
                assert!(!pattern_matches(b"5", b"[[:alpha:]]"));
                assert!(pattern_matches(b"3", b"[[:alnum:]]"));
                assert!(pattern_matches(b"z", b"[[:alnum:]]"));
                assert!(!pattern_matches(b"!", b"[[:alnum:]]"));
                assert!(pattern_matches(b" ", b"[[:blank:]]"));
                assert!(pattern_matches(b"\t", b"[[:blank:]]"));
                assert!(!pattern_matches(b"a", b"[[:blank:]]"));
                assert!(pattern_matches(b"\x01", b"[[:cntrl:]]"));
                assert!(!pattern_matches(b"a", b"[[:cntrl:]]"));
                assert!(pattern_matches(b"9", b"[[:digit:]]"));
                assert!(!pattern_matches(b"a", b"[[:digit:]]"));
                assert!(pattern_matches(b"!", b"[[:graph:]]"));
                assert!(!pattern_matches(b" ", b"[[:graph:]]"));
                assert!(pattern_matches(b"a", b"[[:lower:]]"));
                assert!(!pattern_matches(b"A", b"[[:lower:]]"));
                assert!(pattern_matches(b" ", b"[[:print:]]"));
                assert!(pattern_matches(b"a", b"[[:print:]]"));
                assert!(!pattern_matches(b"\x01", b"[[:print:]]"));
                assert!(pattern_matches(b".", b"[[:punct:]]"));
                assert!(!pattern_matches(b"a", b"[[:punct:]]"));
                assert!(pattern_matches(b"\n", b"[[:space:]]"));
                assert!(!pattern_matches(b"a", b"[[:space:]]"));
                assert!(pattern_matches(b"A", b"[[:upper:]]"));
                assert!(!pattern_matches(b"a", b"[[:upper:]]"));
                assert!(pattern_matches(b"f", b"[[:xdigit:]]"));
                assert!(pattern_matches(b"F", b"[[:xdigit:]]"));
                assert!(!pattern_matches(b"g", b"[[:xdigit:]]"));
                assert!(!pattern_matches(b"a", b"[[:bogus:]]"));
                assert!(pattern_matches(b"x", b"[[:x]"));
                assert!(!pattern_matches(b"", b"[a-z]"));

                assert_eq!(match_bracket(None, 0, b"", 0, b"[a]", 0), None);
                assert_eq!(match_bracket(Some(b'a' as u32), 1, b"a", 0, b"[", 0), None);
                assert_eq!(
                    match_bracket(Some(b']' as u32), 1, b"]", 0, b"[\\]]", 0),
                    Some((true, 4))
                );
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        b"*".to_vec(),
                        QuoteState::Quoted
                    )]),
                    b"\\*".to_vec()
                );
                assert_eq!(
                    render_pattern_from_segments(&[Segment::Text(
                        b"ab".to_vec(),
                        QuoteState::Literal
                    )]),
                    b"ab".to_vec()
                );
                assert_eq!(
                    render_pattern_from_segments(&[
                        Segment::Text(b"x".to_vec(), QuoteState::Literal),
                        Segment::AtBreak,
                        Segment::Text(b"y".to_vec(), QuoteState::Expanded),
                    ]),
                    b"xy".to_vec()
                );
            },
        );
    }

    #[test]
    fn expands_here_documents_without_field_splitting() {
        let mut ctx = FakeContext::new();
        let expanded =
            expand_here_document(&mut ctx, b"hello $USER\n$(printf hi)\n$((1 + 2))\n", 0)
                .expect("expand heredoc");
        assert_eq!(expanded, b"hello meiksh\nprintf hi\n3\n");

        let escaped = expand_here_document(&mut ctx, b"\\$USER\nline\\\ncontinued\n", 0)
            .expect("expand heredoc");
        assert_eq!(escaped, b"$USER\nlinecontinued\n");

        let trailing = expand_here_document(&mut ctx, b"keep\\", 0).expect("expand heredoc");
        assert_eq!(trailing, b"keep\\");

        let literal = expand_here_document(&mut ctx, b"\\x", 0).expect("expand heredoc");
        assert_eq!(literal, b"\\x");

        let double_backslash = expand_here_document(&mut ctx, b"a\\\\b\n", 0)
            .expect("expand heredoc double backslash");
        assert_eq!(double_backslash, b"a\\b\n");
    }

    #[test]
    fn unquoted_at_undergoes_field_splitting() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"a b".to_vec(), b"c".to_vec()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$@".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .expect("unquoted at"),
            vec![b"a".as_ref(), b"b", b"c"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$@".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .expect("unquoted at empty"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn segment_bytes_skips_at_break() {
        let segs = vec![
            Segment::Text(b"a".to_vec(), QuoteState::Expanded),
            Segment::AtBreak,
            Segment::Text(b"b".to_vec(), QuoteState::Quoted),
        ];
        let chars: Vec<_> = segment_bytes(&segs).collect();
        assert_eq!(
            chars,
            vec![(b'a', QuoteState::Expanded), (b'b', QuoteState::Quoted)]
        );
    }

    #[test]
    fn field_splitting_empty_result_returns_empty_vec() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"WS".to_vec(), b"   ".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$WS".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .expect("whitespace only"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn at_break_with_glob_in_at_fields() {
        let mut ctx = FakeContext::new();
        ctx.pathname_expansion_enabled = false;
        ctx.positional = vec![b"*.txt".to_vec(), b"b".to_vec()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("at with glob-like");
        assert_eq!(result, vec![b"*.txt".as_ref(), b"b"]);
    }

    #[test]
    fn flatten_expansion_covers_at_fields() {
        assert_eq!(
            flatten_expansion(Expansion::One(b"hello".to_vec())),
            b"hello"
        );
        assert_eq!(
            flatten_expansion(Expansion::AtFields(vec![b"a".to_vec(), b"b".to_vec()])),
            b"a b"
        );
    }

    #[test]
    fn at_empty_combined_with_at_break() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"x".to_vec()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("at one param");
        assert_eq!(result, vec![b"x".as_ref()]);

        ctx.positional = Vec::new();
        let result2 = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"$@\"".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("at empty");
        assert_eq!(result2, Vec::<&[u8]>::new());
    }

    #[test]
    fn redirect_word_with_expanded_field_splitting() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"V".to_vec(), b"a b".to_vec());
            let result = expand_redirect_word(
                &mut ctx,
                &Word {
                    raw: b"$V".as_ref().into(),
                    parts: Box::new([]),
                    line: 0,
                },
            )
            .expect("redirect word split");
            assert_eq!(result, b"a b");
        });
    }
}
