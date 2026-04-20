#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuoteState {
    Quoted,
    Literal,
    Expanded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    Text(Vec<u8>, QuoteState),
}

pub(super) fn render_pattern_from_segments(segments: &[Segment]) -> Vec<u8> {
    let mut pattern = Vec::new();
    for seg in segments {
        let Segment::Text(text, state) = seg;
        if *state == QuoteState::Quoted {
            for &b in text.iter() {
                pattern.push(b'\\');
                pattern.push(b);
            }
        } else {
            pattern.extend_from_slice(text);
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
    use crate::expand::word::{expand_here_document, expand_redirect_word};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;
    #[test]
    fn field_and_pattern_helpers_cover_corner_cases() {
        run_trace(
            trace_entries![opendir(_) -> err(crate::sys::constants::ENOENT)],
            || {
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
                        Segment::Text(b"y".to_vec(), QuoteState::Expanded),
                    ]),
                    b"xy".to_vec()
                );
            },
        );
    }

    #[test]
    fn expands_here_documents_without_field_splitting() {
        use crate::syntax::build_heredoc_parts;
        let mut ctx = FakeContext::new();
        let body = b"hello $USER\n$(printf hi)\n$((1 + 2))\n";
        let parts = build_heredoc_parts(body);
        let expanded = expand_here_document(&mut ctx, body, &parts, 0).expect("expand heredoc");
        assert_eq!(expanded, b"hello meiksh\nprintf hi\n3\n");

        let body = b"\\$USER\nline\\\ncontinued\n";
        let parts = build_heredoc_parts(body);
        let escaped = expand_here_document(&mut ctx, body, &parts, 0).expect("expand heredoc");
        assert_eq!(escaped, b"$USER\nlinecontinued\n");

        let body = b"keep\\";
        let parts = build_heredoc_parts(body);
        let trailing = expand_here_document(&mut ctx, body, &parts, 0).expect("expand heredoc");
        assert_eq!(trailing, b"keep\\");

        let body = b"\\x";
        let parts = build_heredoc_parts(body);
        let literal = expand_here_document(&mut ctx, body, &parts, 0).expect("expand heredoc");
        assert_eq!(literal, b"\\x");

        let body = b"a\\\\b\n";
        let parts = build_heredoc_parts(body);
        let double_backslash = expand_here_document(&mut ctx, body, &parts, 0)
            .expect("expand heredoc double backslash");
        assert_eq!(double_backslash, b"a\\b\n");
    }
    #[test]
    fn redirect_word_with_expanded_field_splitting() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"V".to_vec(), b"a b".to_vec());
            let word = parsed_first_argv_word(b"cat $V\n");
            let result = expand_redirect_word(&mut ctx, &word).expect("redirect word split");
            assert_eq!(result, b"a b");
        });
    }

    fn parsed_first_argv_word(source: &[u8]) -> crate::syntax::ast::Word {
        let prog = crate::syntax::parse(source).expect("parse");
        let item = &prog.items[0];
        let cmd = &item.and_or.first.commands[0];
        match cmd {
            crate::syntax::ast::Command::Simple(sc) => sc.words[1].clone(),
            _ => panic!("expected simple command"),
        }
    }
}
