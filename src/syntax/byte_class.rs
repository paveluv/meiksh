const BC_WORD_BREAK: u8 = 0x01;
const BC_DELIM: u8 = 0x02;
const BC_ASCII_WS: u8 = 0x04;
const BC_QUOTE: u8 = 0x08;
const BC_NAME_START: u8 = 0x10;
const BC_DIGIT: u8 = 0x20;
const BC_GLOB: u8 = 0x40;
const BC_SPECIAL_PARAM: u8 = 0x80;

const TABLE: [u8; 256] = {
    let mut t = [0u8; 256];

    t[b' ' as usize] = BC_WORD_BREAK | BC_DELIM | BC_ASCII_WS;
    t[b'\t' as usize] = BC_WORD_BREAK | BC_DELIM | BC_ASCII_WS;

    t[b'\n' as usize] = BC_WORD_BREAK | BC_DELIM | BC_ASCII_WS;
    t[b';' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'&' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'|' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'(' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b')' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'<' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'>' as usize] = BC_WORD_BREAK | BC_DELIM;

    t[0x0C] = BC_ASCII_WS; // form feed
    t[b'\r' as usize] = BC_ASCII_WS;

    t[b'#' as usize] = BC_DELIM | BC_SPECIAL_PARAM;

    t[b'\'' as usize] = BC_QUOTE;
    t[b'"' as usize] = BC_QUOTE;
    t[b'\\' as usize] = BC_QUOTE;
    t[b'$' as usize] = BC_QUOTE | BC_SPECIAL_PARAM;
    t[b'`' as usize] = BC_QUOTE;

    t[b'_' as usize] = BC_NAME_START;
    let mut c: u8 = b'A';
    while c <= b'Z' {
        t[c as usize] = BC_NAME_START;
        c += 1;
    }
    c = b'a';
    while c <= b'z' {
        t[c as usize] = BC_NAME_START;
        c += 1;
    }
    c = b'0';
    while c <= b'9' {
        t[c as usize] = BC_DIGIT;
        c += 1;
    }

    t[b'*' as usize] = BC_GLOB | BC_SPECIAL_PARAM;
    t[b'?' as usize] = BC_GLOB | BC_SPECIAL_PARAM;
    t[b'[' as usize] = BC_GLOB;

    t[b'@' as usize] = BC_SPECIAL_PARAM;
    t[b'!' as usize] = BC_SPECIAL_PARAM;
    t[b'-' as usize] = BC_SPECIAL_PARAM;

    t
};

#[inline(always)]
pub(crate) fn is_delim(b: u8) -> bool {
    TABLE[b as usize] & BC_DELIM != 0
}

#[inline(always)]
pub(crate) fn is_word_break(b: u8) -> bool {
    TABLE[b as usize] & BC_WORD_BREAK != 0
}

#[inline(always)]
pub(crate) fn is_quote(b: u8) -> bool {
    TABLE[b as usize] & BC_QUOTE != 0
}

#[inline(always)]
pub(crate) fn is_ascii_ws(b: u8) -> bool {
    TABLE[b as usize] & BC_ASCII_WS != 0
}

#[inline(always)]
pub(crate) fn is_name_start(b: u8) -> bool {
    TABLE[b as usize] & BC_NAME_START != 0
}

#[inline(always)]
pub(crate) fn is_name_cont(b: u8) -> bool {
    TABLE[b as usize] & (BC_NAME_START | BC_DIGIT) != 0
}

#[inline(always)]
pub(crate) fn is_digit(b: u8) -> bool {
    TABLE[b as usize] & BC_DIGIT != 0
}

#[inline(always)]
pub(crate) fn is_glob_char(b: u8) -> bool {
    TABLE[b as usize] & BC_GLOB != 0
}

#[inline(always)]
pub(crate) fn is_special_param(b: u8) -> bool {
    TABLE[b as usize] & BC_SPECIAL_PARAM != 0
}

#[inline(always)]
pub(crate) fn is_tilde_user_break(b: u8) -> bool {
    is_quote(b) || b == b'/' || b == b':'
}

pub(crate) fn alias_has_trailing_blank(s: &[u8]) -> bool {
    s.last().map_or(false, |&b| b == b' ' || b == b'\t')
}

pub(crate) fn is_name(name: &[u8]) -> bool {
    !name.is_empty() && is_name_start(name[0]) && name[1..].iter().all(|&b| is_name_cont(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_match_reference_for_all_256_bytes() {
        for b in 0u8..=255 {
            assert_eq!(
                is_word_break(b),
                matches!(
                    b,
                    b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>'
                ),
                "is_word_break mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_delim(b),
                matches!(
                    b,
                    b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>' | b'#'
                ),
                "is_delim mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_ascii_ws(b),
                b.is_ascii_whitespace(),
                "is_ascii_ws mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_quote(b),
                matches!(b, b'\'' | b'"' | b'\\' | b'$' | b'`'),
                "is_quote mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_name_start(b),
                b == b'_' || b.is_ascii_alphabetic(),
                "is_name_start mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_digit(b),
                b.is_ascii_digit(),
                "is_digit mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_name_cont(b),
                b == b'_' || b.is_ascii_alphanumeric(),
                "is_name_cont mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_glob_char(b),
                matches!(b, b'*' | b'?' | b'['),
                "is_glob_char mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_special_param(b),
                matches!(b, b'@' | b'*' | b'?' | b'$' | b'!' | b'#' | b'-'),
                "is_special_param mismatch for 0x{b:02x}"
            );
            assert_eq!(
                is_tilde_user_break(b),
                matches!(b, b'/' | b'\'' | b'"' | b'\\' | b'$' | b'`' | b':'),
                "is_tilde_user_break mismatch for 0x{b:02x}"
            );
        }
    }

    #[test]
    fn table_flags_are_independent() {
        let flags = [
            BC_WORD_BREAK,
            BC_DELIM,
            BC_ASCII_WS,
            BC_QUOTE,
            BC_NAME_START,
            BC_DIGIT,
            BC_GLOB,
            BC_SPECIAL_PARAM,
        ];
        for (i, &a) in flags.iter().enumerate() {
            for &b in &flags[i + 1..] {
                assert_eq!(a & b, 0, "flags 0x{a:02x} and 0x{b:02x} overlap");
            }
        }
    }

    #[test]
    fn word_break_chars() {
        for &b in b" \t\n;&|()<>" {
            assert!(is_word_break(b), "expected word_break for 0x{b:02x}");
        }
        assert!(!is_word_break(b'#'));
        assert!(!is_word_break(b'a'));
    }

    #[test]
    fn delim_chars() {
        for &b in b" \t\n;&|()<>#" {
            assert!(is_delim(b), "expected delim for 0x{b:02x}");
        }
        assert!(!is_delim(b'a'));
    }

    #[test]
    fn ascii_ws_chars() {
        for &b in b" \t\n\x0c\r" {
            assert!(is_ascii_ws(b), "expected ascii_ws for 0x{b:02x}");
        }
        assert!(!is_ascii_ws(b'\x0b'));
        assert!(!is_ascii_ws(b'a'));
        assert!(!is_ascii_ws(b';'));
    }

    #[test]
    fn quote_chars() {
        for &b in b"'\"\\$`" {
            assert!(is_quote(b), "expected quote for 0x{b:02x}");
        }
        assert!(!is_quote(b'a'));
    }

    #[test]
    fn name_start_and_cont() {
        assert!(is_name_start(b'a'));
        assert!(is_name_start(b'Z'));
        assert!(is_name_start(b'_'));
        assert!(!is_name_start(b'0'));
        assert!(!is_name_start(b'-'));

        assert!(is_name_cont(b'a'));
        assert!(is_name_cont(b'Z'));
        assert!(is_name_cont(b'_'));
        assert!(is_name_cont(b'5'));
        assert!(!is_name_cont(b'-'));
    }

    #[test]
    fn digit_chars() {
        for c in b'0'..=b'9' {
            assert!(is_digit(c));
        }
        assert!(!is_digit(b'a'));
        assert!(!is_digit(b'/'));
    }

    #[test]
    fn glob_chars() {
        assert!(is_glob_char(b'*'));
        assert!(is_glob_char(b'?'));
        assert!(is_glob_char(b'['));
        assert!(!is_glob_char(b']'));
        assert!(!is_glob_char(b'a'));
    }

    #[test]
    fn special_param_chars() {
        for &b in b"@*?$!#-" {
            assert!(is_special_param(b), "expected special_param for {b}");
        }
        assert!(!is_special_param(b'a'));
        assert!(!is_special_param(b'0'));
    }

    #[test]
    fn tilde_user_break_chars() {
        for &b in b"/'\"\\$`:" {
            assert!(
                is_tilde_user_break(b),
                "expected tilde_user_break for 0x{b:02x}"
            );
        }
        assert!(!is_tilde_user_break(b'a'));
        assert!(!is_tilde_user_break(b'~'));
    }

    #[test]
    fn is_name_validates_posix_names() {
        assert!(is_name(b"foo"));
        assert!(is_name(b"_bar"));
        assert!(is_name(b"VAR_1"));
        assert!(is_name(b"a"));
        assert!(!is_name(b""));
        assert!(!is_name(b"1abc"));
        assert!(!is_name(b"-x"));
        assert!(!is_name(b"a-b"));
    }

    #[test]
    fn alias_trailing_blank() {
        assert!(alias_has_trailing_blank(b"value "));
        assert!(alias_has_trailing_blank(b"value\t"));
        assert!(!alias_has_trailing_blank(b"value"));
        assert!(!alias_has_trailing_blank(b""));
    }
}
