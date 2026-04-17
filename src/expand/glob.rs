use crate::sys::locale;

pub(crate) fn pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
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
                let (_, clen) = locale::decode_char(&text[pos..]);
                pos += if clen == 0 { 1 } else { clen };
            }
            false
        }
        b'?' => {
            if ti >= text.len() {
                return false;
            }
            let (_, clen) = locale::decode_char(&text[ti..]);
            let step = if clen == 0 { 1 } else { clen };
            pattern_matches_inner(text, ti + step, pattern, pi + 1)
        }
        b'[' => {
            if ti >= text.len() {
                return match_bracket_invalid(text, ti, pattern, pi);
            }
            let (wc, clen) = locale::decode_char(&text[ti..]);
            let char_len = if clen == 0 { 1 } else { clen };
            match match_bracket(Some(wc), char_len, text, ti, pattern, pi) {
                Some((matched, next_pi)) => {
                    matched && pattern_matches_inner(text, ti + char_len, pattern, next_pi)
                }
                None => text[ti] == b'[' && pattern_matches_inner(text, ti + 1, pattern, pi + 1),
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

fn match_bracket_invalid(text: &[u8], ti: usize, pattern: &[u8], pi: usize) -> bool {
    match match_bracket(None, 0, text, ti, pattern, pi) {
        Some((_, _)) => false,
        None => {
            ti < text.len()
                && text[ti] == b'['
                && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
        }
    }
}

pub(super) fn match_charclass(class: &[u8], ch: u32) -> bool {
    locale::classify_char(class, ch)
}

pub(super) fn match_bracket(
    current: Option<u32>,
    char_len: usize,
    text: &[u8],
    ti: usize,
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
            if let Some(adv) =
                match_bracket_special(pattern, index, current, text, ti, char_len, &mut matched)
            {
                index = adv;
                continue;
            }
        }

        let (first_wc, first_end) = if pc == b'\\' && index + 1 < pattern.len() {
            index += 1;
            let (wc, len) = decode_pattern_char(pattern, index);
            (wc, index + len)
        } else {
            let (wc, len) = decode_pattern_char(pattern, index);
            (wc, index + len)
        };
        if try_consume_range(
            pattern,
            first_end,
            current,
            first_wc,
            &mut matched,
            &mut index,
        ) {
            continue;
        }
        matched |= current == first_wc;
        index = first_end;
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn decode_pattern_char(pattern: &[u8], index: usize) -> (u32, usize) {
    let (wc, len) = locale::decode_char(&pattern[index..]);
    if len == 0 {
        (pattern[index] as u32, 1)
    } else {
        (wc, len)
    }
}

fn try_collating_range_endpoint(pattern: &[u8], rhs: usize) -> Option<(u32, usize)> {
    if pattern[rhs] != b'[' || rhs + 1 >= pattern.len() || pattern[rhs + 1] != b'.' {
        return None;
    }
    let (end, elem) = scan_bracket_delimited(pattern, rhs + 2, b'.', b']')?;
    let (wc, _) = locale::decode_char(elem);
    Some((wc, end))
}

fn match_bracket_special(
    pattern: &[u8],
    index: usize,
    current: u32,
    text: &[u8],
    ti: usize,
    _char_len: usize,
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
            if elem.len() > 1 {
                *matched |= text.get(ti..ti + elem.len()) == Some(elem);
                let mut dummy_index = end;
                if try_consume_range_collsym(pattern, end, current, elem, matched, &mut dummy_index)
                {
                    return Some(dummy_index);
                }
                return Some(end);
            }
            let (wc, _) = locale::decode_char(elem);
            let mut dummy_index = end;
            if try_consume_range(pattern, end, current, wc, matched, &mut dummy_index) {
                return Some(dummy_index);
            }
            *matched |= current == wc;
            Some(end)
        }
        b'=' => {
            let (end, elem) = scan_bracket_delimited(pattern, index + 2, b'=', b']')?;
            let (wc, _) = locale::decode_char(elem);
            *matched |= current == wc;
            Some(end)
        }
        _ => None,
    }
}

fn try_consume_range_collsym(
    pattern: &[u8],
    after_first: usize,
    current: u32,
    _elem: &[u8],
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
    if let Some((last_wc, end)) = try_collating_range_endpoint(pattern, rhs) {
        let (first_wc, _) = locale::decode_char(_elem);
        *matched |= first_wc <= current && current <= last_wc;
        *index = end;
        return true;
    }
    false
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
    current: u32,
    first: u32,
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
    let (last, last_len) = decode_pattern_char(pattern, rhs);
    *matched |= first <= current && current <= last;
    *index = rhs + last_len;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{assert_no_syscalls, set_test_locale_c, set_test_locale_utf8};

    #[test]
    fn question_mark_c_vs_utf8() {
        assert_no_syscalls(|| {
            // U+00E9 is 2 bytes; ? matches one character
            set_test_locale_c();
            assert!(!pattern_matches(b"\xc3\xa9", b"?"));
            assert!(pattern_matches(b"\xc3\xa9", b"??"));

            set_test_locale_utf8();
            assert!(pattern_matches(b"\xc3\xa9", b"?"));
            assert!(!pattern_matches(b"\xc3\xa9", b"??"));
        });
    }

    #[test]
    fn alpha_class_c_vs_utf8() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert!(!pattern_matches(b"\xc3\xa9", b"[[:alpha:]]"));

            set_test_locale_utf8();
            assert!(pattern_matches(b"\xc3\xa9", b"[[:alpha:]]"));
        });
    }

    #[test]
    fn bracket_helpers_cover_missing_closer() {
        assert_eq!(match_bracket(Some(b'a' as u32), 1, b"a", 0, b"[a", 0), None);
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
