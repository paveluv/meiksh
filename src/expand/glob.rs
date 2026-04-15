pub fn pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
    pattern_matches_inner(text, 0, pattern, 0)
}

pub(super) fn pattern_matches_inner(text: &[u8], ti: usize, pattern: &[u8], pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    let pc = pattern[pi];
    match pc {
        b'*' => {
            let mut pos = ti;
            loop {
                if pattern_matches_inner(text, pos, pattern, pi + 1) {
                    return true;
                }
                if pos == text.len() {
                    break;
                }
                pos += 1;
            }
            false
        }
        b'?' => ti < text.len() && pattern_matches_inner(text, ti + 1, pattern, pi + 1),
        b'[' => {
            let tc = if ti < text.len() {
                Some(text[ti])
            } else {
                None
            };
            match match_bracket(tc, pattern, pi) {
                Some((matched, next_pi)) => {
                    matched
                        && ti < text.len()
                        && pattern_matches_inner(text, ti + 1, pattern, next_pi)
                }
                None => {
                    ti < text.len()
                        && text[ti] == b'['
                        && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
                }
            }
        }
        b'\\' if pi + 1 < pattern.len() => {
            let escaped = pattern[pi + 1];
            ti < text.len()
                && text[ti] == escaped
                && pattern_matches_inner(text, ti + 1, pattern, pi + 2)
        }
        ch => {
            ti < text.len()
                && text[ti] == ch
                && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
        }
    }
}

pub(super) fn match_charclass(class: &[u8], ch: u8) -> bool {
    crate::sys::classify_byte(class, ch)
}

pub(super) fn match_bracket(
    current: Option<u8>,
    pattern: &[u8],
    start: usize,
) -> Option<(bool, usize)> {
    let current = current?;
    let mut index = start + 1;
    if index >= pattern.len() {
        return None;
    }

    let mut negate = false;
    if index < pattern.len() && matches!(pattern[index], b'!' | b'^') {
        negate = true;
        index += 1;
    }

    let mut matched = false;
    let mut saw_closer = false;
    let mut first_elem = true;
    while index < pattern.len() {
        let pc = pattern[index];
        if pc == b']' && !first_elem {
            saw_closer = true;
            index += 1;
            break;
        }

        first_elem = false;

        if pc == b'[' && index + 1 < pattern.len() {
            if let Some(adv) = match_bracket_special(pattern, index, current, &mut matched) {
                index = adv;
                continue;
            }
        }

        let first = if pc == b'\\' && index + 1 < pattern.len() {
            index += 1;
            pattern[index]
        } else {
            pc
        };
        if try_consume_range(pattern, index + 1, current, first, &mut matched, &mut index) {
            continue;
        }
        matched |= current == first;
        index += 1;
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn try_collating_range_endpoint(pattern: &[u8], rhs: usize) -> Option<(u8, usize)> {
    if pattern[rhs] != b'[' || rhs + 1 >= pattern.len() || pattern[rhs + 1] != b'.' {
        return None;
    }
    let (end, elem) = scan_bracket_delimited(pattern, rhs + 2, b'.', b']')?;
    Some((elem[0], end))
}

fn match_bracket_special(
    pattern: &[u8],
    index: usize,
    current: u8,
    matched: &mut bool,
) -> Option<usize> {
    let delim = pattern[index + 1];
    match delim {
        b':' => {
            let (end, class_name) = scan_bracket_delimited(pattern, index + 2, b':', b']')?;
            *matched |= match_charclass(class_name, current);
            Some(end)
        }
        b'.' => {
            let (end, elem) = scan_bracket_delimited(pattern, index + 2, b'.', b']')?;
            let ch = elem[0];
            let mut dummy_index = end;
            if try_consume_range(pattern, end, current, ch, matched, &mut dummy_index) {
                return Some(dummy_index);
            }
            *matched |= current == ch;
            Some(end)
        }
        b'=' => {
            let (end, elem) = scan_bracket_delimited(pattern, index + 2, b'=', b']')?;
            *matched |= current == elem[0];
            Some(end)
        }
        _ => None,
    }
}

fn scan_bracket_delimited<'a>(
    pattern: &'a [u8],
    start: usize,
    close_char: u8,
    close_bracket: u8,
) -> Option<(usize, &'a [u8])> {
    let mut i = start;
    while i + 1 < pattern.len() {
        if pattern[i] == close_char && pattern[i + 1] == close_bracket {
            let content = &pattern[start..i];
            if content.is_empty() {
                return None;
            }
            return Some((i + 2, content));
        }
        i += 1;
    }
    None
}

fn try_consume_range(
    pattern: &[u8],
    after_first: usize,
    current: u8,
    first: u8,
    matched: &mut bool,
    index: &mut usize,
) -> bool {
    if after_first + 1 >= pattern.len() || pattern[after_first] != b'-' {
        return false;
    }
    let rhs = after_first + 1;
    if pattern[rhs] == b']' {
        return false;
    }
    if let Some((last, end)) = try_collating_range_endpoint(pattern, rhs) {
        *matched |= first <= current && current <= last;
        *index = end;
        return true;
    }
    let last = pattern[rhs];
    *matched |= first <= current && current <= last;
    *index = rhs + 1;
    true
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::arena::ByteArena;
    use crate::bstr;
    use crate::expand::arithmetic::*;
    use crate::expand::core::{Context, ExpandError};
    use crate::expand::glob::*;
    use crate::expand::model::*;
    use crate::expand::parameter::*;
    use crate::expand::pathname::*;
    use crate::expand::test_support::*;
    use crate::expand::word::*;
    use crate::syntax::Word;

    #[test]
    fn bracket_helpers_cover_missing_closer() {
        assert_eq!(match_bracket(Some(b'a'), b"[a", 0), None);
    }

    #[test]
    fn collating_symbol_matches_literal() {
        assert!(pattern_matches(b"a", b"[[.a.]]"));
        assert!(!pattern_matches(b"b", b"[[.a.]]"));
    }

    #[test]
    fn collating_symbol_in_bracket_with_others() {
        assert!(pattern_matches(b"a", b"[[.a.]bc]"));
        assert!(pattern_matches(b"b", b"[[.a.]bc]"));
        assert!(!pattern_matches(b"d", b"[[.a.]bc]"));
    }

    #[test]
    fn collating_symbol_as_range_endpoint() {
        assert!(pattern_matches(b"b", b"[[.a.]-[.c.]]"));
        assert!(pattern_matches(b"a", b"[[.a.]-[.c.]]"));
        assert!(pattern_matches(b"c", b"[[.a.]-[.c.]]"));
        assert!(!pattern_matches(b"d", b"[[.a.]-[.c.]]"));
    }

    #[test]
    fn equivalence_class_matches_literal() {
        assert!(pattern_matches(b"a", b"[[=a=]]"));
        assert!(!pattern_matches(b"b", b"[[=a=]]"));
    }

    #[test]
    fn equivalence_class_in_bracket_with_others() {
        assert!(pattern_matches(b"a", b"[[=a=]bc]"));
        assert!(pattern_matches(b"b", b"[[=a=]bc]"));
        assert!(!pattern_matches(b"d", b"[[=a=]bc]"));
    }

    #[test]
    fn equivalence_class_fallback_to_collating_symbol() {
        assert!(pattern_matches(b"z", b"[[=z=]]"));
    }

    #[test]
    fn malformed_collating_symbol_falls_through() {
        assert!(pattern_matches(b"[]", b"[[.a]]"));
        assert!(pattern_matches(b"a]", b"[[.a]]"));
        assert!(!pattern_matches(b"a", b"[[.a]]"));
    }

    #[test]
    fn malformed_equivalence_class_falls_through() {
        assert!(pattern_matches(b"[]", b"[[=a]]"));
        assert!(pattern_matches(b"a]", b"[[=a]]"));
        assert!(!pattern_matches(b"a", b"[[=a]]"));
    }

    #[test]
    fn empty_collating_symbol_falls_through() {
        assert!(pattern_matches(b".]", b"[[..]]"));
        assert!(!pattern_matches(b"a]", b"[[..]]"));
    }

    #[test]
    fn bracket_with_nested_open_bracket_literal() {
        assert!(pattern_matches(b"[", b"[[]"));
        assert!(!pattern_matches(b"a", b"[[]"));
    }

    #[test]
    fn range_with_hyphen_before_closer() {
        assert!(pattern_matches(b"-", b"[a-]"));
        assert!(pattern_matches(b"a", b"[a-]"));
        assert!(!pattern_matches(b"b", b"[a-]"));
    }
}
