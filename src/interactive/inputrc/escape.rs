//! String-escape decoding for inputrc quoted forms (spec § 4.5).

#![allow(dead_code)]

/// Parse a quoted-string slice starting immediately after the opening
/// `"`. On success returns `(bytes, consumed)` where `consumed` is
/// the number of input bytes consumed including the closing quote.
pub(crate) fn decode_quoted(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'"' => return Ok((out, i + 1)),
            b'\\' => {
                let (bytes, step) = decode_escape(&input[i + 1..])?;
                out.extend_from_slice(&bytes);
                i += 1 + step;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Err("unterminated string".to_string())
}

/// Decode a `\`-escape starting *after* the `\`. Returns the emitted
/// bytes and the number of bytes consumed from `input`.
pub(crate) fn decode_escape(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if input.is_empty() {
        return Err("dangling backslash".to_string());
    }
    let c = input[0];
    match c {
        b'\\' => Ok((vec![b'\\'], 1)),
        b'"' => Ok((vec![b'"'], 1)),
        b'\'' => Ok((vec![b'\''], 1)),
        b'a' => Ok((vec![0x07], 1)),
        b'b' => Ok((vec![0x08], 1)),
        b'd' => Ok((vec![0x7f], 1)),
        b'e' => Ok((vec![0x1b], 1)),
        b'f' => Ok((vec![0x0c], 1)),
        b'n' => Ok((vec![0x0a], 1)),
        b'r' => Ok((vec![0x0d], 1)),
        b't' => Ok((vec![0x09], 1)),
        b'v' => Ok((vec![0x0b], 1)),
        b'0'..=b'7' => decode_octal(input),
        b'x' | b'X' => decode_hex(input),
        b'C' => decode_control(&input[1..]).map(|(b, n)| (b, n + 1)),
        b'M' => decode_meta(&input[1..]).map(|(b, n)| (b, n + 1)),
        _ => Err(format!("unknown escape: \\{}", c as char)),
    }
}

fn decode_octal(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    // Callers (`decode_escape`) only route here when `input[0]` is a
    // digit in `0..=7`, so the loop always consumes at least one byte
    // and `n >= 1` on exit.
    let mut value: u32 = 0;
    let mut n = 0;
    while n < 3 && n < input.len() && (b'0'..=b'7').contains(&input[n]) {
        value = value * 8 + (input[n] - b'0') as u32;
        n += 1;
    }
    if value > 0xff {
        return Err(format!("octal escape out of range: \\{}", value));
    }
    Ok((vec![value as u8], n))
}

fn decode_hex(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    // input begins at 'x'
    let mut value: u32 = 0;
    let mut n = 1;
    while n < 3 && n < input.len() {
        let d = match input[n] {
            b'0'..=b'9' => input[n] - b'0',
            b'a'..=b'f' => input[n] - b'a' + 10,
            b'A'..=b'F' => input[n] - b'A' + 10,
            _ => break,
        };
        value = value * 16 + d as u32;
        n += 1;
    }
    if n == 1 {
        return Err("empty hex escape".to_string());
    }
    Ok((vec![value as u8], n))
}

fn decode_control(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if input.is_empty() || input[0] != b'-' {
        return Err("\\C- must be followed by `-`".to_string());
    }
    let rest = &input[1..];
    if rest.is_empty() {
        return Err("\\C- requires a character".to_string());
    }
    let (sub_bytes, consumed) = if rest[0] == b'\\' {
        let (b, n) = decode_escape(&rest[1..])?;
        (b, n + 1)
    } else {
        (vec![rest[0]], 1)
    };
    let mut out: Vec<u8> = Vec::with_capacity(sub_bytes.len());
    for (i, b) in sub_bytes.iter().enumerate() {
        if i + 1 == sub_bytes.len() {
            out.push(*b & 0x1f);
        } else {
            out.push(*b);
        }
    }
    Ok((out, 1 + consumed))
}

fn decode_meta(input: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if input.is_empty() || input[0] != b'-' {
        return Err("\\M- must be followed by `-`".to_string());
    }
    let rest = &input[1..];
    if rest.is_empty() {
        return Err("\\M- requires a character".to_string());
    }
    let (sub_bytes, consumed) = if rest[0] == b'\\' {
        let (b, n) = decode_escape(&rest[1..])?;
        (b, n + 1)
    } else {
        (vec![rest[0]], 1)
    };
    let mut out = Vec::with_capacity(1 + sub_bytes.len());
    out.push(0x1b);
    out.extend_from_slice(&sub_bytes);
    Ok((out, 1 + consumed))
}

/// Resolve a keyname-form token (case-insensitive, supports `C-`,
/// `Control-`, `M-`, `Meta-` prefixes).
pub(crate) fn decode_keyname(token: &[u8]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut rest = token;
    let mut control = false;
    let mut meta = false;
    loop {
        if let Some(r) = strip_prefix_ci(rest, b"Control-") {
            control = true;
            rest = r;
        } else if let Some(r) = strip_prefix_ci(rest, b"C-") {
            control = true;
            rest = r;
        } else if let Some(r) = strip_prefix_ci(rest, b"Meta-") {
            meta = true;
            rest = r;
        } else if let Some(r) = strip_prefix_ci(rest, b"M-") {
            meta = true;
            rest = r;
        } else {
            break;
        }
    }
    let byte = keyname_to_byte(rest)?;
    if meta {
        out.push(0x1b);
    }
    if control {
        out.push(byte & 0x1f);
    } else {
        out.push(byte);
    }
    Ok(out)
}

fn keyname_to_byte(token: &[u8]) -> Result<u8, String> {
    match token {
        t if eq_ci(t, b"Return") || eq_ci(t, b"RET") || eq_ci(t, b"Newline") => Ok(b'\n'),
        t if eq_ci(t, b"Escape") || eq_ci(t, b"ESC") => Ok(0x1b),
        t if eq_ci(t, b"Tab") => Ok(b'\t'),
        t if eq_ci(t, b"Rubout") || eq_ci(t, b"DEL") => Ok(0x7f),
        t if eq_ci(t, b"Space") || eq_ci(t, b"SPC") => Ok(b' '),
        t if eq_ci(t, b"LFD") => Ok(b'\n'),
        [b] if b.is_ascii_graphic() || *b == b' ' => Ok(*b),
        other => Err(format!(
            "unknown keyname: {}",
            String::from_utf8_lossy(other)
        )),
    }
}

fn strip_prefix_ci<'a>(input: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    if input.len() < prefix.len() {
        return None;
    }
    for (a, b) in input.iter().zip(prefix.iter()) {
        if a.eq_ignore_ascii_case(b) {
            continue;
        } else {
            return None;
        }
    }
    Some(&input[prefix.len()..])
}

fn eq_ci(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| x.eq_ignore_ascii_case(y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn decode_basic_escapes() {
        assert_no_syscalls(|| {
            assert_eq!(decode_quoted(b"\\e\"").unwrap().0, vec![0x1b]);
            assert_eq!(decode_quoted(b"\\n\\t\"").unwrap().0, vec![b'\n', b'\t']);
            assert_eq!(decode_quoted(b"\\C-a\"").unwrap().0, vec![1]);
            assert_eq!(decode_quoted(b"\\M-a\"").unwrap().0, vec![0x1b, b'a']);
        });
    }

    #[test]
    fn decode_hex_and_octal() {
        assert_no_syscalls(|| {
            assert_eq!(decode_quoted(b"\\x1b\"").unwrap().0, vec![0x1b]);
            assert_eq!(decode_quoted(b"\\033\"").unwrap().0, vec![0x1b]);
        });
    }

    #[test]
    fn decode_nested_c_m() {
        assert_no_syscalls(|| {
            // \M-\C-h → ESC, BS
            assert_eq!(decode_quoted(b"\\M-\\C-h\"").unwrap().0, vec![0x1b, 0x08]);
        });
    }

    #[test]
    fn decode_keyname_control_meta() {
        assert_no_syscalls(|| {
            assert_eq!(decode_keyname(b"C-a").unwrap(), vec![0x01]);
            assert_eq!(decode_keyname(b"Meta-a").unwrap(), vec![0x1b, b'a']);
            assert_eq!(decode_keyname(b"Control-Meta-a").unwrap(), vec![0x1b, 0x01]);
            assert_eq!(decode_keyname(b"Escape").unwrap(), vec![0x1b]);
            assert_eq!(decode_keyname(b"RET").unwrap(), vec![b'\n']);
        });
    }

    #[test]
    fn unknown_escape_is_error() {
        assert_no_syscalls(|| {
            assert!(decode_quoted(b"\\Z\"").is_err());
        });
    }

    #[test]
    fn unknown_keyname_token_is_error() {
        assert_no_syscalls(|| {
            // Multi-byte non-canonical tokens fall to the
            // `unknown keyname` arm.
            let err = decode_keyname(b"ZZZ").unwrap_err();
            assert!(err.contains("unknown keyname"), "got: {err}");
        });
    }

    #[test]
    fn every_simple_escape_letter_decodes() {
        assert_no_syscalls(|| {
            // One assertion per single-char escape branch in decode_escape.
            assert_eq!(decode_quoted(br#"\\""#).unwrap().0, vec![b'\\']);
            assert_eq!(decode_quoted(br#"\"""#).unwrap().0, vec![b'"']);
            assert_eq!(decode_quoted(br#"\'""#).unwrap().0, vec![b'\'']);
            assert_eq!(decode_quoted(br#"\a""#).unwrap().0, vec![0x07]);
            assert_eq!(decode_quoted(br#"\b""#).unwrap().0, vec![0x08]);
            assert_eq!(decode_quoted(br#"\d""#).unwrap().0, vec![0x7f]);
            assert_eq!(decode_quoted(br#"\f""#).unwrap().0, vec![0x0c]);
            assert_eq!(decode_quoted(br#"\r""#).unwrap().0, vec![0x0d]);
            assert_eq!(decode_quoted(br#"\v""#).unwrap().0, vec![0x0b]);
        });
    }

    #[test]
    fn unterminated_string_is_error() {
        assert_no_syscalls(|| {
            // Scan to end without hitting closing quote.
            let err = decode_quoted(b"abc").unwrap_err();
            assert!(err.contains("unterminated"), "got: {err}");
        });
    }

    #[test]
    fn dangling_backslash_is_error() {
        assert_no_syscalls(|| {
            // `\` followed by nothing is caught inside decode_escape.
            let err = decode_escape(b"").unwrap_err();
            assert!(err.contains("dangling"), "got: {err}");
        });
    }

    #[test]
    fn octal_out_of_range_is_error() {
        assert_no_syscalls(|| {
            // `\777` = 511 decimal, > 0xff.
            let err = decode_quoted(br#"\777""#).unwrap_err();
            assert!(err.contains("out of range"), "got: {err}");
        });
    }

    #[test]
    fn hex_escape_accepts_lowercase_and_uppercase() {
        assert_no_syscalls(|| {
            assert_eq!(decode_quoted(br#"\xab""#).unwrap().0, vec![0xab]);
            assert_eq!(decode_quoted(br#"\xCD""#).unwrap().0, vec![0xcd]);
            assert_eq!(decode_quoted(br#"\X0f""#).unwrap().0, vec![0x0f]);
        });
    }

    #[test]
    fn empty_hex_escape_is_error() {
        assert_no_syscalls(|| {
            // `\x` followed by a non-hex character (the closing quote).
            let err = decode_quoted(br#"\x""#).unwrap_err();
            assert!(err.contains("empty hex"), "got: {err}");
        });
    }

    #[test]
    fn control_missing_dash_is_error() {
        assert_no_syscalls(|| {
            // `\C` without `-` after — decode_control receives a slice
            // whose first byte is not `-`.
            let err = decode_escape(b"Cx").unwrap_err();
            assert!(err.contains("C-"), "got: {err}");
            // And the empty case (`\C` at end of input).
            let err = decode_escape(b"C").unwrap_err();
            assert!(err.contains("C-"), "got: {err}");
        });
    }

    #[test]
    fn control_without_target_is_error() {
        assert_no_syscalls(|| {
            // `\C-` followed by nothing.
            let err = decode_escape(b"C-").unwrap_err();
            assert!(err.contains("requires"), "got: {err}");
        });
    }

    #[test]
    fn meta_missing_dash_is_error() {
        assert_no_syscalls(|| {
            let err = decode_escape(b"Mx").unwrap_err();
            assert!(err.contains("M-"), "got: {err}");
            let err = decode_escape(b"M").unwrap_err();
            assert!(err.contains("M-"), "got: {err}");
        });
    }

    #[test]
    fn meta_without_target_is_error() {
        assert_no_syscalls(|| {
            let err = decode_escape(b"M-").unwrap_err();
            assert!(err.contains("requires"), "got: {err}");
        });
    }

    #[test]
    fn control_wraps_multibyte_inner_escape() {
        assert_no_syscalls(|| {
            // `\C-\M-a` → inner decode yields two bytes [0x1b, 'a'];
            // the control wrap masks only the last byte and passes the
            // rest through verbatim. This is the only production path
            // that hits the non-last-byte branch of decode_control.
            assert_eq!(decode_quoted(br#"\C-\M-a""#).unwrap().0, vec![0x1b, 0x01]);
        });
    }

    #[test]
    fn keyname_m_prefix_distinct_from_meta_prefix() {
        assert_no_syscalls(|| {
            // The `Meta-` branch is exercised elsewhere; this covers
            // the short `M-` form specifically.
            assert_eq!(decode_keyname(b"M-a").unwrap(), vec![0x1b, b'a']);
            // Case-insensitive per spec.
            assert_eq!(decode_keyname(b"m-a").unwrap(), vec![0x1b, b'a']);
        });
    }
}
