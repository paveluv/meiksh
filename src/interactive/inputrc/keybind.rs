//! Parse a binding directive: `<lhs>: <rhs>` (spec § 4).

#![allow(dead_code)]

use super::super::emacs_editing::keymap::{EmacsFn, KeymapEntry};
use super::escape::{decode_keyname, decode_quoted};

/// Parse a single binding line (already trimmed).
///
/// Returns `(keyseq, entry)` on success. On parse failure, returns a
/// human-readable diagnostic.
pub(crate) fn parse(line: &[u8]) -> Result<(Vec<u8>, KeymapEntry), String> {
    // Split at the first `:` outside a quoted string.
    let split_at = find_unquoted_colon(line).ok_or_else(|| "missing `:` in binding".to_string())?;
    let (lhs_raw, rest) = line.split_at(split_at);
    let rhs_raw = trim_ws(&rest[1..]);
    let lhs = trim_ws(lhs_raw);
    if lhs.is_empty() {
        return Err("empty key sequence".to_string());
    }

    let seq = if lhs.first() == Some(&b'"') {
        let (bytes, consumed) = decode_quoted(&lhs[1..])?;
        if consumed != lhs.len() - 1 {
            return Err("trailing junk after quoted key sequence".to_string());
        }
        bytes
    } else {
        decode_keyname(lhs)?
    };

    if rhs_raw.first() == Some(&b'"') {
        let (macro_bytes, consumed) = decode_quoted(&rhs_raw[1..])?;
        if consumed != rhs_raw.len() - 1 {
            return Err("trailing junk after macro value".to_string());
        }
        return Ok((seq, KeymapEntry::Macro(macro_bytes)));
    }
    let name = rhs_raw;
    let func = EmacsFn::from_name(name)
        .ok_or_else(|| format!("unknown function: {}", String::from_utf8_lossy(name)))?;
    Ok((seq, KeymapEntry::Func(func)))
}

fn find_unquoted_colon(line: &[u8]) -> Option<usize> {
    let mut in_quote = false;
    let mut i = 0;
    while i < line.len() {
        let c = line[i];
        if in_quote {
            if c == b'\\' && i + 1 < line.len() {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_quote = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_quote = true;
        } else if c == b':' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn trim_ws(bytes: &[u8]) -> &[u8] {
    let mut s = 0;
    let mut e = bytes.len();
    while s < e && matches!(bytes[s], b' ' | b'\t') {
        s += 1;
    }
    while e > s && matches!(bytes[e - 1], b' ' | b'\t') {
        e -= 1;
    }
    &bytes[s..e]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn keyname_form_maps_to_function() {
        assert_no_syscalls(|| {
            let (seq, entry) = parse(b"Control-a: beginning-of-line").unwrap();
            assert_eq!(seq, vec![0x01]);
            assert_eq!(entry, KeymapEntry::Func(EmacsFn::BeginningOfLine));
        });
    }

    #[test]
    fn quoted_form_maps_to_function() {
        assert_no_syscalls(|| {
            let (seq, entry) = parse(b"\"\\C-a\": beginning-of-line").unwrap();
            assert_eq!(seq, vec![0x01]);
            assert_eq!(entry, KeymapEntry::Func(EmacsFn::BeginningOfLine));
        });
    }

    #[test]
    fn macro_rhs_parsed_as_bytes() {
        assert_no_syscalls(|| {
            let (seq, entry) = parse(b"\"\\C-xg\": \"git status\\n\"").unwrap();
            assert_eq!(seq, vec![0x18, b'g']);
            match entry {
                KeymapEntry::Macro(bytes) => assert_eq!(bytes, b"git status\n"),
                _ => panic!("expected macro"),
            }
        });
    }

    #[test]
    fn unknown_function_rejected() {
        assert_no_syscalls(|| {
            let err = parse(b"C-a: no-such").unwrap_err();
            assert!(err.contains("unknown function"));
        });
    }

    #[test]
    fn missing_colon_rejected() {
        assert_no_syscalls(|| {
            let err = parse(b"C-a beginning-of-line").unwrap_err();
            assert!(err.contains("missing"));
        });
    }

    #[test]
    fn empty_left_hand_side_rejected() {
        // The parser treats `:` with no LHS as an empty key sequence;
        // it must report the dedicated "empty key sequence" diagnostic
        // so the user knows which half of the binding is missing.
        assert_no_syscalls(|| {
            let err = parse(b":accept-line").unwrap_err();
            assert_eq!(err, "empty key sequence");
        });
    }

    #[test]
    fn trailing_junk_after_quoted_lhs_rejected() {
        // A properly closed `"..."` followed by stray bytes before the
        // colon is a malformed binding; `consumed != lhs.len() - 1`
        // triggers the trailing-junk diagnostic.
        assert_no_syscalls(|| {
            let err = parse(b"\"\\C-a\"xx: accept-line").unwrap_err();
            assert!(
                err.contains("trailing junk after quoted key sequence"),
                "got: {err}",
            );
        });
    }

    #[test]
    fn trailing_junk_after_quoted_macro_rejected() {
        // The macro branch has the same check applied to the RHS:
        // bytes after the closing `"` are rejected with a dedicated
        // diagnostic.
        assert_no_syscalls(|| {
            let err = parse(b"\"\\C-a\": \"macro\"xx").unwrap_err();
            assert!(
                err.contains("trailing junk after macro value"),
                "got: {err}",
            );
        });
    }

    #[test]
    fn leading_and_trailing_whitespace_stripped_from_rhs() {
        // `trim_ws` walks both ends of the RHS; a tab-padded name
        // exercises both the leading and trailing trim loops.
        assert_no_syscalls(|| {
            let (_, entry) = parse(b"C-a: \taccept-line\t").expect("parse");
            assert_eq!(entry, KeymapEntry::Func(EmacsFn::AcceptLine));
        });
    }
}
