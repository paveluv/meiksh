use super::expand_parts::char_boundary_offsets;
use crate::sys::locale;

/// Match `pattern` against `text` (top-level entry, POSIX-compliant byte-level
/// engine ported from dash's `pmatch` in `src/expand.c:1550-1646` and extended
/// to preserve character-aware semantics for `?` and `[...]`).
///
/// Computes the per-character boundary table on each call; prefer
/// [`pattern_matches_with_offsets`] when the caller already has one.
pub(crate) fn pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
    let offsets = char_boundary_offsets(text);
    pattern_matches_inner(text, 0, &offsets, 0, pattern, 0)
}

/// Same as [`pattern_matches`] but reuses a caller-computed boundary table.
///
/// `text_offsets[k] - text_base` is the byte index within `text` of the k-th
/// character boundary. Callers typically compute a single `offsets` vector
/// over a full string, then pass sub-slices of that vector (plus the
/// corresponding `text_base`) for each candidate prefix/suffix, so no
/// reallocation or rebasing is required.
pub(crate) fn pattern_matches_with_offsets(
    text: &[u8],
    text_offsets: &[usize],
    text_base: usize,
    pattern: &[u8],
) -> bool {
    pattern_matches_inner(text, 0, text_offsets, text_base, pattern, 0)
}

/// Core fnmatch engine. Iterative main loop, modelled after dash's `pmatch`;
/// `*` recurses, everything else state-machines in place.
///
/// For POSIX compliance, `?` and `[...]` still advance by one *character*,
/// using `text_offsets` for the common case and falling back to
/// `locale::decode_char` for positions that are not on a character boundary
/// (which can only happen for malformed inputs, since `*`'s byte-level
/// backtracking never causes a literal match to resume off-boundary in
/// well-formed UTF-8 text).
pub(super) fn pattern_matches_inner(
    text: &[u8],
    ti0: usize,
    text_offsets: &[usize],
    text_base: usize,
    pattern: &[u8],
    pi0: usize,
) -> bool {
    let mut pi = pi0;
    let mut ti = ti0;
    loop {
        if pi == pattern.len() {
            return ti == text.len();
        }
        let pc = pattern[pi];
        match pc {
            b'*' => {
                pi += 1;
                while pi < pattern.len() && pattern[pi] == b'*' {
                    pi += 1;
                }
                if pi == pattern.len() {
                    return true;
                }
                let next = pattern[pi];
                if next != b'\\' && next != b'?' && next != b'[' && next != b'*' {
                    match text[ti..].iter().position(|&b| b == next) {
                        Some(off) => ti += off,
                        None => return false,
                    }
                }
                loop {
                    if pattern_matches_inner(text, ti, text_offsets, text_base, pattern, pi) {
                        return true;
                    }
                    if ti == text.len() {
                        return false;
                    }
                    ti += 1;
                }
            }
            b'?' => {
                pi += 1;
                if ti >= text.len() {
                    return false;
                }
                ti = match next_char_end(text_offsets, text_base, ti) {
                    Some(end) => end,
                    None => {
                        let (_, clen) = locale::decode_char(&text[ti..]);
                        ti + if clen == 0 { 1 } else { clen }
                    }
                };
            }
            b'[' => {
                if ti >= text.len() {
                    return false;
                }
                let (wc, clen) = locale::decode_char(&text[ti..]);
                let char_len = if clen == 0 { 1 } else { clen };
                match match_bracket(Some(wc), char_len, text, ti, pattern, pi) {
                    Some((matched, next_pi)) => {
                        if !matched {
                            return false;
                        }
                        pi = next_pi;
                        ti += char_len;
                    }
                    None => {
                        if text[ti] != b'[' {
                            return false;
                        }
                        ti += 1;
                        pi += 1;
                    }
                }
            }
            b'\\' if pi + 1 < pattern.len() => {
                let c = pattern[pi + 1];
                if ti >= text.len() || text[ti] != c {
                    return false;
                }
                ti += 1;
                pi += 2;
            }
            _ => {
                if ti >= text.len() || text[ti] != pc {
                    return false;
                }
                ti += 1;
                pi += 1;
            }
        }
    }
}

#[inline]
fn next_char_end(text_offsets: &[usize], text_base: usize, ti: usize) -> Option<usize> {
    let target = ti + text_base;
    match text_offsets.binary_search(&target) {
        Ok(k) if k + 1 < text_offsets.len() => Some(text_offsets[k + 1] - text_base),
        _ => None,
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

    #[test]
    fn invalid_bracket_at_end_of_text() {
        assert!(!pattern_matches(b"", b"["));
        assert!(!pattern_matches(b"", b"[a]"));
    }

    #[test]
    fn multi_char_collating_element_in_bracket() {
        assert!(!pattern_matches(b"x", b"[[.ab.]c]"));
        assert!(pattern_matches(b"c", b"[[.ab.]c]"));
    }

    #[test]
    fn multi_char_collsym_with_dash_falls_through() {
        assert!(!pattern_matches(b"c", b"[[.ab.]-z]"));
        assert!(pattern_matches(b"z", b"[[.ab.]-z]"));
        assert!(pattern_matches(b"-", b"[[.ab.]-z]"));
    }

    #[test]
    fn multi_char_collsym_range_both_endpoints() {
        assert!(pattern_matches(b"c", b"[[.ab.]-[.z.]]"));
        assert!(pattern_matches(b"z", b"[[.ab.]-[.z.]]"));
    }

    #[test]
    fn multi_char_collsym_dash_closer() {
        assert!(pattern_matches(b"-", b"[[.ab.]-]"));
    }

    #[test]
    fn decode_pattern_char_single_byte_fallback() {
        set_test_locale_c();
        assert!(pattern_matches(b"\xff", b"[\xff]"));
    }

    #[test]
    fn decode_pattern_char_nul_byte_fallback() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            let (wc, len) = decode_pattern_char(b"\x00rest", 0);
            assert_eq!(wc, 0);
            assert_eq!(len, 1);
        });
    }

    #[test]
    fn decode_pattern_char_lone_continuation_fallback() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // `\xa9` alone is an invalid UTF-8 sequence; `decode_char`
            // returns `len == 0` and `decode_pattern_char` falls back to a
            // literal byte.
            let (wc, len) = decode_pattern_char(b"\xa9", 0);
            assert_eq!(wc, 0xa9);
            assert_eq!(len, 1);
        });
    }

    #[test]
    fn question_mark_mid_char_fallback() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // Pattern `\xc3?` on text `\xc3\xa9`: the literal `\xc3` byte
            // matches the lead byte of `é`, leaving `ti == 1`, which is
            // inside the multi-byte character. `?` then hits the
            // `next_char_end -> None` branch and falls back to
            // `decode_char` on the continuation byte (returns `len == 0`),
            // advancing one byte to ti == 2 == text.len().
            assert!(pattern_matches(b"\xc3\xa9", b"\xc3?"));
        });
    }

    /// Simple char-advance recursive reference implementation. Semantically
    /// equivalent to the new engine on well-formed inputs (where `*`'s
    /// byte-level backtracking cannot land a literal match at a mid-character
    /// position in the text).
    fn reference_matches(text: &[u8], pattern: &[u8]) -> bool {
        reference_inner(text, 0, pattern, 0)
    }

    fn reference_inner(text: &[u8], ti: usize, pattern: &[u8], pi: usize) -> bool {
        if pi == pattern.len() {
            return ti == text.len();
        }
        let pc = pattern[pi];
        match pc {
            b'*' => {
                let mut pos = ti;
                loop {
                    if reference_inner(text, pos, pattern, pi + 1) {
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
                reference_inner(text, ti + step, pattern, pi + 1)
            }
            b'[' => {
                if ti >= text.len() {
                    return false;
                }
                let (wc, clen) = locale::decode_char(&text[ti..]);
                let char_len = if clen == 0 { 1 } else { clen };
                match match_bracket(Some(wc), char_len, text, ti, pattern, pi) {
                    Some((matched, next_pi)) => {
                        matched && reference_inner(text, ti + char_len, pattern, next_pi)
                    }
                    None => text[ti] == b'[' && reference_inner(text, ti + 1, pattern, pi + 1),
                }
            }
            b'\\' if pi + 1 < pattern.len() => {
                let escaped = pattern[pi + 1];
                ti < text.len()
                    && text[ti] == escaped
                    && reference_inner(text, ti + 1, pattern, pi + 2)
            }
            ch => {
                ti < text.len() && text[ti] == ch && reference_inner(text, ti + 1, pattern, pi + 1)
            }
        }
    }

    fn check_matches(text: &[u8], pattern: &[u8], expected: bool) {
        let actual = pattern_matches(text, pattern);
        let reference = reference_matches(text, pattern);
        assert_eq!(
            actual, expected,
            "pattern_matches disagreed with expected for text={:?} pat={:?}",
            text, pattern
        );
        assert_eq!(
            reference, expected,
            "reference_matches disagreed with expected for text={:?} pat={:?}",
            text, pattern
        );
    }

    #[test]
    fn correctness_matrix_ascii() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            let cases: &[(&[u8], &[u8], bool)] = &[
                (b"", b"", true),
                (b"", b"*", true),
                (b"", b"**", true),
                (b"", b"?", false),
                (b"a", b"", false),
                (b"abc", b"abc", true),
                (b"abc", b"ab", false),
                (b"abc", b"abcd", false),
                (b"abc", b"*", true),
                (b"abc", b"???", true),
                (b"abc", b"????", false),
                (b"abc", b"a*c", true),
                (b"abc", b"a*C", false),
                (b"abcdef", b"*def", true),
                (b"abcdef", b"abc*", true),
                (b"abab", b"*a*b*", true),
                (b"xxx", b"***x***", true),
                (b"aaaaa", b"*a*a*a*", true),
                (b"aa", b"*a*a*a*", false),
                (b"foo/bar.txt", b"foo/*", true),
                (b"foo/bar.txt", b"*/*.txt", true),
                (b"foo/bar.txt", b"*.tar.gz", false),
                (b"file_3.txt", b"*_[0-9].txt", true),
                (b"file_x.txt", b"*_[0-9].txt", false),
                (b"foobar", b"*bar", true),
                (b"foo-bar", b"foo-*", true),
                (b"abc", b"\\*", false),
                (b"*", b"\\*", true),
                (b"a*c", b"a\\*c", true),
                (b"abc", b"a\\*c", false),
                (b"[", b"\\[", true),
                (b"?", b"\\?", true),
                (b"abc", b"\\?bc", false),
                (b"\\", b"\\", true),
                (b"?", b"\\", false),
                (b"[", b"[", true),
                (b"a", b"[", false),
                (b"x", b"[abc]", false),
                (b"b", b"[abc]", true),
            ];
            for (text, pat, expected) in cases {
                check_matches(text, pat, *expected);
            }
        });
    }

    #[test]
    fn correctness_matrix_utf8() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // café = c a f \xc3\xa9, 4 chars / 5 bytes.
            // π = \xcf\x80 (U+03C0), 1 char / 2 bytes.
            // é = \xc3\xa9, 1 char / 2 bytes.
            let cafe: &[u8] = b"caf\xc3\xa9";
            let pi_bytes: &[u8] = b"\xcf\x80";
            let e_acute: &[u8] = b"\xc3\xa9";

            let cases: Vec<(Vec<u8>, Vec<u8>, bool)> = vec![
                (cafe.to_vec(), cafe.to_vec(), true),
                (cafe.to_vec(), b"*".to_vec(), true),
                (cafe.to_vec(), b"caf?".to_vec(), true),
                (cafe.to_vec(), b"caf??".to_vec(), false),
                (cafe.to_vec(), b"????".to_vec(), true),
                (cafe.to_vec(), b"?????".to_vec(), false),
                (cafe.to_vec(), b"*\xc3\xa9".to_vec(), true),
                (
                    cafe.to_vec(),
                    [b"foo/".to_vec(), cafe.to_vec()].concat(),
                    false,
                ),
                (
                    [b"foo/".to_vec(), cafe.to_vec()].concat(),
                    [b"*".to_vec(), cafe.to_vec()].concat(),
                    true,
                ),
                (b"x\xc3\xa9".to_vec(), b"?\xc3\xa9".to_vec(), true),
                (b"xy\xc3\xa9".to_vec(), b"?\xc3\xa9".to_vec(), false),
                (b"xy\xc3\xa9".to_vec(), b"??\xc3\xa9".to_vec(), true),
                (e_acute.to_vec(), b"[\xc3\xa9\xc3\xa8]".to_vec(), true),
                (b"a".to_vec(), b"[\xc3\xa9\xc3\xa8]".to_vec(), false),
                (
                    [
                        cafe.to_vec(),
                        b"/".to_vec(),
                        pi_bytes.to_vec(),
                        b"/".to_vec(),
                    ]
                    .concat(),
                    [b"*/".to_vec(), pi_bytes.to_vec(), b"/".to_vec()].concat(),
                    true,
                ),
                (cafe.to_vec(), b"caf\\\xc3\xa9".to_vec(), true),
                (b"[".to_vec(), b"\\[".to_vec(), true),
            ];
            for (text, pat, expected) in &cases {
                check_matches(text, pat, *expected);
            }
        });
    }

    /// Deterministic property check: enumerate short (text, pattern) pairs
    /// over a small alphabet containing meta-characters and verify the new
    /// engine agrees with the reference on every pair.
    #[test]
    fn property_new_vs_reference_small_alphabet() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            let text_alphabet: &[u8] = b"ab";
            let pattern_alphabet: &[u8] = b"ab*?[]\\";
            let mut checked = 0u32;
            for text_len in 0..=4 {
                for text_idx in 0..(text_alphabet.len().pow(text_len as u32) as u32) {
                    let text = build_word(text_alphabet, text_len, text_idx as usize);
                    for pat_len in 0..=3 {
                        for pat_idx in 0..(pattern_alphabet.len().pow(pat_len as u32) as u32) {
                            let pat = build_word(pattern_alphabet, pat_len, pat_idx as usize);
                            let actual = pattern_matches(&text, &pat);
                            let expected = reference_matches(&text, &pat);
                            assert_eq!(
                                actual, expected,
                                "new/ref diverge: text={:?} pat={:?}",
                                text, pat
                            );
                            checked += 1;
                        }
                    }
                }
            }
            assert!(checked > 1000, "property test must cover >1000 pairs");
        });
    }

    fn build_word(alphabet: &[u8], len: usize, mut idx: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(alphabet[idx % alphabet.len()]);
            idx /= alphabet.len();
        }
        out
    }

    #[test]
    fn with_offsets_matches_top_level() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            let value: &[u8] = b"caf\xc3\xa9-\xc3\xa8";
            let offsets = super::super::expand_parts::char_boundary_offsets(value);
            let pat: &[u8] = b"?????";
            assert_eq!(
                pattern_matches_with_offsets(value, &offsets, 0, pat),
                pattern_matches(value, pat),
            );
            // Sub-slice: start=3 (before é).
            let start = 3;
            let k = offsets.iter().position(|&x| x == start).unwrap();
            let sub = &value[start..];
            let sub_pat: &[u8] = b"?-?";
            assert_eq!(
                pattern_matches_with_offsets(sub, &offsets[k..], start, sub_pat),
                pattern_matches(sub, sub_pat),
            );
        });
    }

    #[test]
    fn next_char_end_mid_char_returns_none() {
        assert_no_syscalls(|| {
            let offsets = [0usize, 2, 3];
            assert_eq!(next_char_end(&offsets, 0, 0), Some(2));
            assert_eq!(next_char_end(&offsets, 0, 2), Some(3));
            assert_eq!(next_char_end(&offsets, 0, 1), None); // mid-char
            assert_eq!(next_char_end(&offsets, 0, 3), None); // past last
        });
    }

    #[test]
    fn star_memchr_skip_misses_and_fails_fast() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // Text lacks 'z', so *z must not match and must return after the
            // memchr miss without per-byte recursion.
            assert!(!pattern_matches(b"abcdefghij", b"*z"));
        });
    }

    #[test]
    fn star_with_meta_next_skips_memchr() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // next = '?' is meta, memchr branch must be skipped.
            assert!(pattern_matches(b"abc", b"*?c"));
            assert!(pattern_matches(b"abc", b"*[abc]c"));
            assert!(pattern_matches(b"ab*", b"*\\*"));
            // Trailing star: pi == pattern.len() after collapsing.
            assert!(pattern_matches(b"abc", b"***"));
        });
    }

    #[test]
    fn backslash_at_end_of_pattern_is_literal() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert!(pattern_matches(b"\\", b"\\"));
            assert!(!pattern_matches(b"a", b"\\"));
        });
    }
}
