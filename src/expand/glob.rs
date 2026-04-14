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

        if pc == b'[' && index + 1 < pattern.len() && pattern[index + 1] == b':' {
            let class_start = index + 2;
            let mut found_end = None;
            let mut ci = class_start;
            while ci + 1 < pattern.len() {
                if pattern[ci] == b':' && pattern[ci + 1] == b']' {
                    found_end = Some(ci);
                    break;
                }
                ci += 1;
            }
            if let Some(end) = found_end {
                let class_name = &pattern[class_start..end];
                matched |= match_charclass(class_name, current);
                index = end + 2;
                continue;
            }
        }

        let first = if pc == b'\\' && index + 1 < pattern.len() {
            index += 1;
            pattern[index]
        } else {
            pc
        };
        if index + 2 < pattern.len() && pattern[index + 1] == b'-' && pattern[index + 2] != b']' {
            let last = pattern[index + 2];
            matched |= first <= current && current <= last;
            index += 3;
        } else {
            matched |= current == first;
            index += 1;
        }
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
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
}
