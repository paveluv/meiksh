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
    let mut value: u32 = 0;
    let mut n = 0;
    while n < 3 && n < input.len() && (b'0'..=b'7').contains(&input[n]) {
        value = value * 8 + (input[n] - b'0') as u32;
        n += 1;
    }
    if n == 0 {
        return Err("empty octal escape".to_string());
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
    if out.len() > 2 {
        return Err("keyname expands to more than two bytes".to_string());
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
    fn keyname_too_long_is_error() {
        assert_no_syscalls(|| {
            // "Control-Meta-Meta-..." — impossible case; test Meta- applied
            // to a multi-byte escape
            assert!(decode_keyname(b"ZZZ").is_err());
        });
    }
}
