use super::*;

pub(super) fn printf_builtin(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<BuiltinOutcome, ShellError> {
    if argv.len() < 2 {
        shell.diagnostic(1, b"printf: missing format operand");
        return Ok(BuiltinOutcome::Status(1));
    }
    let format = &argv[1];
    let args = &argv[2..];
    let mut had_error = false;
    let mut arg_idx = 0;
    loop {
        let (output, consumed, stop, error) = printf_format(shell, format, args, arg_idx);
        if !output.is_empty() {
            let _ = sys::write_all_fd(sys::STDOUT_FILENO, &output);
        }
        if error {
            had_error = true;
        }
        if stop {
            break;
        }
        arg_idx += consumed;
        if arg_idx >= args.len() {
            break;
        }
    }
    Ok(BuiltinOutcome::Status(if had_error { 1 } else { 0 }))
}

pub(super) fn printf_parse_numeric_arg(
    shell: &Shell,
    arg: &[u8],
    had_error: &mut bool,
) -> (i64, bool) {
    match printf_parse_int(arg) {
        Ok(v) => (v, true),
        Err(msg) => {
            let full = ByteWriter::new().bytes(b"printf: ").bytes(&msg).finish();
            shell.diagnostic(1, &full);
            *had_error = true;
            (0, false)
        }
    }
}

pub(super) fn printf_check_trailing(shell: &Shell, arg: &[u8], had_error: &mut bool) {
    if !arg.is_empty() && arg[0] != b'\'' && arg[0] != b'"' {
        if printf_find_trailing_garbage(arg).is_some() {
            let msg = ByteWriter::new()
                .bytes(b"printf: \"")
                .bytes(arg)
                .bytes(b"\": not completely converted")
                .finish();
            shell.diagnostic(1, &msg);
            *had_error = true;
        }
    }
}

pub(super) fn printf_get_arg<'a>(args: &'a [Vec<u8>], base: usize, idx: usize) -> &'a [u8] {
    args.get(base + idx).map(|s| s.as_slice()).unwrap_or(b"")
}

pub(super) fn printf_parse_int(s: &[u8]) -> Result<i64, Vec<u8>> {
    if s.is_empty() {
        return Ok(0);
    }
    if s[0] == b'\'' || s[0] == b'"' {
        let ch = s.get(1).copied().unwrap_or(0);
        return Ok(ch as i64);
    }
    let (neg, s) = if let Some(rest) = s.strip_prefix(b"-") {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix(b"+") {
        (false, rest)
    } else {
        (false, s)
    };
    let val = if let Some(hex) = s.strip_prefix(b"0x").or_else(|| s.strip_prefix(b"0X")) {
        parse_hex_i64(hex).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    } else if s.first() == Some(&b'0') && s.len() > 1 {
        parse_octal_i64(s).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    } else {
        bstr::parse_i64(s).ok_or_else(|| {
            let mut msg = s.to_vec();
            msg.extend_from_slice(b": invalid number");
            msg
        })
    };
    val.map(|v| if neg { -v } else { v })
}

pub(super) fn parse_hex_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in s {
        let digit = match b {
            b'0'..=b'9' => (b - b'0') as i64,
            b'a'..=b'f' => (b - b'a' + 10) as i64,
            b'A'..=b'F' => (b - b'A' + 10) as i64,
            _ => return None,
        };
        result = result.checked_mul(16)?.checked_add(digit)?;
    }
    Some(result)
}

pub(super) fn parse_octal_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in s {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        result = result.checked_mul(8)?.checked_add((b - b'0') as i64)?;
    }
    Some(result)
}

pub(super) fn printf_format(
    shell: &Shell,
    format: &[u8],
    args: &[Vec<u8>],
    arg_base: usize,
) -> (Vec<u8>, usize, bool, bool) {
    let mut out: Vec<u8> = Vec::new();
    let bytes = format;
    let mut i = 0;
    let mut arg_consumed = 0;
    let mut had_error = false;
    let mut stop = false;

    while i < bytes.len() {
        if stop {
            break;
        }
        if bytes[i] == b'\\' {
            let (esc, advance) = printf_format_escape(bytes, i + 1);
            out.extend_from_slice(&esc);
            i += 1 + advance;
            continue;
        }
        if bytes[i] == b'%' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'%' {
                out.push(b'%');
                i += 2;
                continue;
            }
            let spec_start = i;
            i += 1;

            let mut numbered_arg: Option<usize> = None;
            let saved = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'$' && i > saved {
                let n = parse_usize(&bytes[saved..i]).unwrap_or(0);
                if n > 0 {
                    numbered_arg = Some(n - 1);
                }
                i += 1;
            } else {
                i = saved;
            }

            while i < bytes.len() && matches!(bytes[i], b'-' | b'+' | b' ' | b'0' | b'#') {
                i += 1;
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'.' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i >= bytes.len() {
                out.extend_from_slice(&format[spec_start..]);
                break;
            }
            let conv = bytes[i];
            i += 1;
            let full_spec = &format[spec_start..i];

            let arg_index = if let Some(n) = numbered_arg {
                n
            } else {
                let idx = arg_consumed;
                arg_consumed += 1;
                idx
            };
            let arg = printf_get_arg(args, arg_base, arg_index);

            match conv {
                b's' => {
                    let spec_for_rust = remove_byte(full_spec, conv);
                    printf_format_string(&mut out, &spec_for_rust, arg);
                }
                b'b' => {
                    let (expanded, saw_c) = printf_expand_b(arg);
                    let spec_for_rust = remove_byte(full_spec, b'b');
                    printf_format_string(&mut out, &spec_for_rust, &expanded);
                    if saw_c {
                        stop = true;
                    }
                }
                b'c' => {
                    if let Some(&b) = arg.first() {
                        out.push(b);
                    }
                }
                b'd' | b'i' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    let mut c_spec = remove_byte(full_spec, conv);
                    c_spec.extend_from_slice(b"ld");
                    printf_format_signed(&mut out, &c_spec, val);
                }
                b'u' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    let mut c_spec = remove_byte(full_spec, b'u');
                    c_spec.extend_from_slice(b"lu");
                    printf_format_unsigned(&mut out, &c_spec, val as u64);
                }
                b'o' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    printf_format_octal(&mut out, full_spec, val as u64);
                }
                b'x' | b'X' => {
                    let (val, parse_ok) = printf_parse_numeric_arg(shell, arg, &mut had_error);
                    if parse_ok {
                        printf_check_trailing(shell, arg, &mut had_error);
                    }
                    printf_format_hex(&mut out, full_spec, val as u64, conv == b'X');
                }
                _ => {
                    out.extend_from_slice(full_spec);
                }
            }
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    (
        out,
        arg_consumed,
        stop || arg_base + arg_consumed >= args.len(),
        had_error,
    )
}

pub(super) fn remove_byte(s: &[u8], byte: u8) -> Vec<u8> {
    s.iter().copied().filter(|&b| b != byte).collect()
}

pub(super) fn printf_find_trailing_garbage(s: &[u8]) -> Option<usize> {
    let s = if s.first() == Some(&b'+') || s.first() == Some(&b'-') {
        &s[1..]
    } else {
        s
    };
    if let Some(hex) = s.strip_prefix(b"0x").or_else(|| s.strip_prefix(b"0X")) {
        for (i, &c) in hex.iter().enumerate() {
            if !c.is_ascii_hexdigit() {
                return Some(i + 2);
            }
        }
        return None;
    }
    if s.first() == Some(&b'0') && s.len() > 1 {
        for (i, &c) in s.iter().enumerate().skip(1) {
            if !(b'0'..=b'7').contains(&c) {
                return Some(i);
            }
        }
        return None;
    }
    for (i, &c) in s.iter().enumerate() {
        if !c.is_ascii_digit() {
            return Some(i);
        }
    }
    None
}

pub(super) fn printf_format_escape(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
    if start >= bytes.len() {
        return (vec![b'\\'], 0);
    }
    match bytes[start] {
        b'\\' => (vec![b'\\'], 1),
        b'a' => (vec![0x07], 1),
        b'b' => (vec![0x08], 1),
        b'f' => (vec![0x0c], 1),
        b'n' => (vec![b'\n'], 1),
        b'r' => (vec![b'\r'], 1),
        b't' => (vec![b'\t'], 1),
        b'v' => (vec![0x0b], 1),
        b'0'..=b'7' => {
            let mut val: u8 = 0;
            let mut count = 0;
            let mut j = start;
            while j < bytes.len() && count < 3 && bytes[j] >= b'0' && bytes[j] <= b'7' {
                val = val.wrapping_mul(8).wrapping_add(bytes[j] - b'0');
                j += 1;
                count += 1;
            }
            (vec![val], count)
        }
        _ => (vec![b'\\', bytes[start]], 1),
    }
}

pub(super) fn printf_expand_b(s: &[u8]) -> (Vec<u8>, bool) {
    let mut out: Vec<u8> = Vec::new();
    let bytes = s;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 1;
            if i >= bytes.len() {
                out.push(b'\\');
                break;
            }
            match bytes[i] {
                b'\\' => {
                    out.push(b'\\');
                    i += 1;
                }
                b'a' => {
                    out.push(0x07);
                    i += 1;
                }
                b'b' => {
                    out.push(0x08);
                    i += 1;
                }
                b'f' => {
                    out.push(0x0c);
                    i += 1;
                }
                b'n' => {
                    out.push(b'\n');
                    i += 1;
                }
                b'r' => {
                    out.push(b'\r');
                    i += 1;
                }
                b't' => {
                    out.push(b'\t');
                    i += 1;
                }
                b'v' => {
                    out.push(0x0b);
                    i += 1;
                }
                b'c' => return (out, true),
                b'0' => {
                    i += 1;
                    let mut val: u8 = 0;
                    let mut count = 0;
                    while i < bytes.len() && count < 3 && bytes[i] >= b'0' && bytes[i] <= b'7' {
                        val = val.wrapping_mul(8).wrapping_add(bytes[i] - b'0');
                        i += 1;
                        count += 1;
                    }
                    out.push(val);
                }
                _ => {
                    out.push(b'\\');
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    (out, false)
}

pub(super) fn printf_format_string(out: &mut Vec<u8>, spec: &[u8], s: &[u8]) {
    let spec = if spec.first() == Some(&b'%') {
        &spec[1..]
    } else {
        spec
    };
    let left = has_left_flag(spec);
    let spec_rest = trim_leading_flags(spec);

    let (width_str, prec_str) = if let Some(dot_pos) = spec_rest.iter().position(|&b| b == b'.') {
        (&spec_rest[..dot_pos], Some(&spec_rest[dot_pos + 1..]))
    } else {
        (spec_rest, None)
    };

    let width: usize = parse_usize(width_str).unwrap_or(0);
    let value = if let Some(prec) = prec_str {
        let max: usize = parse_usize(prec).unwrap_or(usize::MAX);
        if s.len() > max { &s[..max] } else { s }
    } else {
        s
    };

    if left || width <= value.len() {
        out.extend_from_slice(value);
        if left && width > value.len() {
            out.resize(out.len() + width - value.len(), b' ');
        }
    } else {
        out.resize(out.len() + width - value.len(), b' ');
        out.extend_from_slice(value);
    }
}

pub(super) fn flags_end(spec: &[u8]) -> usize {
    let mut i = 0;
    while i < spec.len() && matches!(spec[i], b'-' | b'+' | b' ' | b'0' | b'#') {
        i += 1;
    }
    i
}

pub(super) fn trim_leading_flags(spec: &[u8]) -> &[u8] {
    &spec[flags_end(spec)..]
}

pub(super) fn has_zero_flag(spec: &[u8]) -> bool {
    spec[..flags_end(spec)].contains_byte(b'0')
}

pub(super) fn has_left_flag(spec: &[u8]) -> bool {
    spec[..flags_end(spec)].contains_byte(b'-')
}

pub(super) fn has_alt_flag(spec: &[u8]) -> bool {
    spec[..flags_end(spec)].contains_byte(b'#')
}

pub(super) fn printf_format_signed(out: &mut Vec<u8>, spec: &[u8], val: i64) {
    let spec = if spec.first() == Some(&b'%') {
        &spec[1..]
    } else {
        spec
    };
    let spec = if spec.ends_with(b"ld") {
        &spec[..spec.len() - 2]
    } else {
        spec
    };
    let left = has_left_flag(spec);
    let zero_pad = has_zero_flag(spec) && !left;
    let spec_rest = trim_leading_flags(spec);

    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = bstr::i64_to_bytes(val);

    if width <= num.len() || (!left && !zero_pad) {
        if width > num.len() && !left {
            out.resize(out.len() + width - num.len(), b' ');
        }
        out.extend_from_slice(&num);
    } else if zero_pad {
        if val < 0 {
            out.push(b'-');
            let digits = &num[1..];
            if width > num.len() {
                out.resize(out.len() + width - num.len(), b'0');
            }
            out.extend_from_slice(digits);
        } else {
            out.resize(out.len() + width - num.len(), b'0');
            out.extend_from_slice(&num);
        }
    } else {
        out.extend_from_slice(&num);
    }
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

pub(super) fn printf_format_unsigned(out: &mut Vec<u8>, spec: &[u8], val: u64) {
    let spec = if spec.first() == Some(&b'%') {
        &spec[1..]
    } else {
        spec
    };
    let spec = if spec.ends_with(b"lu") {
        &spec[..spec.len() - 2]
    } else {
        spec
    };
    let left = has_left_flag(spec);
    let zero_pad = has_zero_flag(spec) && !left;
    let spec_rest = trim_leading_flags(spec);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = bstr::u64_to_bytes(val);
    if zero_pad && width > num.len() {
        out.resize(out.len() + width - num.len(), b'0');
    } else if !left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
    out.extend_from_slice(&num);
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

pub(super) fn printf_format_octal(out: &mut Vec<u8>, spec: &[u8], val: u64) {
    let spec_inner = if spec.first() == Some(&b'%') {
        &spec[1..]
    } else {
        spec
    };
    let spec_inner = if spec_inner.last() == Some(&b'o') {
        &spec_inner[..spec_inner.len() - 1]
    } else {
        spec_inner
    };
    let alt = has_alt_flag(spec_inner);
    let left = has_left_flag(spec_inner);
    let zero_pad = has_zero_flag(spec_inner) && !left;
    let spec_rest = trim_leading_flags(spec_inner);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = if alt && val != 0 {
        let mut r = b"0".to_vec();
        bstr::push_u64_octal(&mut r, val);
        r
    } else if alt {
        b"0".to_vec()
    } else {
        let mut r = Vec::new();
        bstr::push_u64_octal(&mut r, val);
        r
    };
    if zero_pad && width > num.len() {
        out.resize(out.len() + width - num.len(), b'0');
    } else if !left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
    out.extend_from_slice(&num);
    if left && width > num.len() {
        out.resize(out.len() + width - num.len(), b' ');
    }
}

pub(super) fn printf_format_hex(out: &mut Vec<u8>, spec: &[u8], val: u64, upper: bool) {
    let suffix = if upper { b'X' } else { b'x' };
    let spec_inner = if spec.first() == Some(&b'%') {
        &spec[1..]
    } else {
        spec
    };
    let spec_inner = if spec_inner.last() == Some(&suffix) {
        &spec_inner[..spec_inner.len() - 1]
    } else {
        spec_inner
    };
    let alt = has_alt_flag(spec_inner);
    let left = has_left_flag(spec_inner);
    let zero_pad = has_zero_flag(spec_inner) && !left;
    let spec_rest = trim_leading_flags(spec_inner);
    let width: usize = parse_usize(spec_rest).unwrap_or(0);
    let num = if upper {
        let mut r = Vec::new();
        bstr::push_u64_hex_upper(&mut r, val);
        r
    } else {
        let mut r = Vec::new();
        bstr::push_u64_hex(&mut r, val);
        r
    };
    let prefix: &[u8] = if alt && val != 0 {
        if upper { b"0X" } else { b"0x" }
    } else {
        b""
    };
    let total = prefix.len() + num.len();
    if zero_pad && width > total {
        out.extend_from_slice(prefix);
        out.resize(out.len() + width - total, b'0');
    } else if !left && width > total {
        out.resize(out.len() + width - total, b' ');
        out.extend_from_slice(prefix);
    } else {
        out.extend_from_slice(prefix);
    }
    out.extend_from_slice(&num);
    if left && width > total {
        out.resize(out.len() + width - total, b' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;

    #[test]
    fn printf_format_loop_basic() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(1),
                    ArgMatcher::Bytes(b"hello world".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[
                        b"printf".to_vec(),
                        b"%s %s".to_vec(),
                        b"hello".to_vec(),
                        b"world".to_vec(),
                    ],
                )
                .expect("printf");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn printf_literal_percent_sign() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"100%".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &[b"printf".to_vec(), b"100%%".to_vec()]).expect("printf %%");
            },
        );
    }

    #[test]
    fn printf_escape_sequences() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"a\tb\n".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &[b"printf".to_vec(), b"a\\tb\\n".to_vec()])
                    .expect("printf escapes");
            },
        );
    }

    #[test]
    fn printf_format_signed_integer() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"42".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%d".to_vec(), b"42".to_vec()],
                )
                .expect("printf %d");
            },
        );
    }

    #[test]
    fn printf_format_unsigned_integer() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"42".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%u".to_vec(), b"42".to_vec()],
                )
                .expect("printf %u");
            },
        );
    }

    #[test]
    fn printf_format_octal_output() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"52".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%o".to_vec(), b"42".to_vec()],
                )
                .expect("printf %o");
            },
        );
    }

    #[test]
    fn printf_format_hex_output() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"2a".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%x".to_vec(), b"42".to_vec()],
                )
                .expect("printf %x");
            },
        );
    }

    #[test]
    fn printf_format_hex_upper() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"2A".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%X".to_vec(), b"42".to_vec()],
                )
                .expect("printf %X");
            },
        );
    }

    #[test]
    fn printf_char_format() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"A".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%c".to_vec(), b"ABC".to_vec()],
                )
                .expect("printf %c");
            },
        );
    }

    #[test]
    fn printf_b_format_with_escape() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"a\tb".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%b".to_vec(), b"a\\tb".to_vec()],
                )
                .expect("printf %b");
            },
        );
    }

    #[test]
    fn printf_b_format_backslash_c_stops() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"hello".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[
                        b"printf".to_vec(),
                        b"%b".to_vec(),
                        b"hello\\c world".to_vec(),
                    ],
                )
                .expect("printf %b \\c");
            },
        );
    }

    #[test]
    fn printf_width_padding() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"  hi".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%4s".to_vec(), b"hi".to_vec()],
                )
                .expect("printf %4s");
            },
        );
    }

    #[test]
    fn printf_left_align_width() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"hi  ".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%-4s".to_vec(), b"hi".to_vec()],
                )
                .expect("printf %-4s");
            },
        );
    }

    #[test]
    fn printf_precision_truncation() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"hel".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%.3s".to_vec(), b"hello".to_vec()],
                )
                .expect("printf %.3s");
            },
        );
    }

    #[test]
    fn printf_width_and_precision_combined() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"  hel".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%5.3s".to_vec(), b"hello".to_vec()],
                )
                .expect("printf %5.3s");
            },
        );
    }

    #[test]
    fn printf_zero_padded_integer() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"00042".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%05d".to_vec(), b"42".to_vec()],
                )
                .expect("printf %05d");
            },
        );
    }

    #[test]
    fn printf_left_padded_integer() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"42   ".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%-5d".to_vec(), b"42".to_vec()],
                )
                .expect("printf %-5d");
            },
        );
    }

    #[test]
    fn printf_negative_zero_padded() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"-0042".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%05d".to_vec(), b"-42".to_vec()],
                )
                .expect("printf %05d negative");
            },
        );
    }

    #[test]
    fn printf_hex_parse() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"0xFF"), Ok(255));
            assert_eq!(printf_parse_int(b"0x10"), Ok(16));
        });
    }

    #[test]
    fn printf_octal_parse() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"010"), Ok(8));
            assert_eq!(printf_parse_int(b"077"), Ok(63));
        });
    }

    #[test]
    fn printf_char_value_parse() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"'A"), Ok(65));
            assert_eq!(printf_parse_int(b"\"B"), Ok(66));
            assert_eq!(printf_parse_int(b"'"), Ok(0));
        });
    }

    #[test]
    fn printf_sign_prefix_parse() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"+42"), Ok(42));
            assert_eq!(printf_parse_int(b"-42"), Ok(-42));
        });
    }

    #[test]
    fn printf_invalid_number_error() {
        assert_no_syscalls(|| {
            let result = printf_parse_int(b"abc");
            assert!(result.is_err());
        });
    }

    #[test]
    fn printf_format_escape_all_sequences() {
        assert_no_syscalls(|| {
            assert_eq!(printf_format_escape(b"\\", 0), (vec![b'\\'], 1));
            assert_eq!(printf_format_escape(b"a", 0), (vec![0x07], 1));
            assert_eq!(printf_format_escape(b"b", 0), (vec![0x08], 1));
            assert_eq!(printf_format_escape(b"f", 0), (vec![0x0c], 1));
            assert_eq!(printf_format_escape(b"n", 0), (vec![b'\n'], 1));
            assert_eq!(printf_format_escape(b"r", 0), (vec![b'\r'], 1));
            assert_eq!(printf_format_escape(b"t", 0), (vec![b'\t'], 1));
            assert_eq!(printf_format_escape(b"v", 0), (vec![0x0b], 1));
            assert_eq!(printf_format_escape(b"101", 0), (vec![b'A'], 3));
            assert_eq!(printf_format_escape(b"z", 0), (vec![b'\\', b'z'], 1));
            assert_eq!(printf_format_escape(b"", 0), (vec![b'\\'], 0));
        });
    }

    #[test]
    fn printf_expand_b_all_escapes() {
        assert_no_syscalls(|| {
            assert_eq!(printf_expand_b(b"\\\\"), (vec![b'\\'], false));
            assert_eq!(printf_expand_b(b"\\a"), (vec![0x07], false));
            assert_eq!(printf_expand_b(b"\\b"), (vec![0x08], false));
            assert_eq!(printf_expand_b(b"\\f"), (vec![0x0c], false));
            assert_eq!(printf_expand_b(b"\\n"), (vec![b'\n'], false));
            assert_eq!(printf_expand_b(b"\\r"), (vec![b'\r'], false));
            assert_eq!(printf_expand_b(b"\\t"), (vec![b'\t'], false));
            assert_eq!(printf_expand_b(b"\\v"), (vec![0x0b], false));
            assert_eq!(printf_expand_b(b"\\c rest"), (Vec::<u8>::new(), true));
            assert_eq!(printf_expand_b(b"\\0101"), (vec![b'A'], false));
            assert_eq!(printf_expand_b(b"\\z"), (vec![b'\\', b'z'], false));
            assert_eq!(printf_expand_b(b"plain"), (b"plain".to_vec(), false));
            assert_eq!(printf_expand_b(b"\\"), (vec![b'\\'], false));
        });
    }

    #[test]
    fn printf_format_string_right_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_string(&mut out, b"%6", b"hi");
            assert_eq!(out, b"    hi");
        });
    }

    #[test]
    fn printf_format_string_left_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_string(&mut out, b"%-6", b"hi");
            assert_eq!(out, b"hi    ");
        });
    }

    #[test]
    fn printf_format_string_with_precision() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_string(&mut out, b"%.3", b"hello");
            assert_eq!(out, b"hel");
        });
    }

    #[test]
    fn printf_format_signed_right_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_signed(&mut out, b"%8ld", 42);
            assert_eq!(out, b"      42");
        });
    }

    #[test]
    fn printf_format_signed_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_signed(&mut out, b"%08ld", 42);
            assert_eq!(out, b"00000042");
        });
    }

    #[test]
    fn printf_format_signed_negative_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_signed(&mut out, b"%08ld", -42);
            assert_eq!(out, b"-0000042");
        });
    }

    #[test]
    fn printf_format_signed_left_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_signed(&mut out, b"%-8ld", 42);
            assert_eq!(out, b"42      ");
        });
    }

    #[test]
    fn printf_format_unsigned_right_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_unsigned(&mut out, b"%8lu", 42);
            assert_eq!(out, b"      42");
        });
    }

    #[test]
    fn printf_format_unsigned_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_unsigned(&mut out, b"%08lu", 42);
            assert_eq!(out, b"00000042");
        });
    }

    #[test]
    fn printf_format_unsigned_left_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_unsigned(&mut out, b"%-8lu", 42);
            assert_eq!(out, b"42      ");
        });
    }

    #[test]
    fn printf_format_octal_alt_flag() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%#o", 42);
            assert_eq!(out, b"052");

            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%#o", 0);
            assert_eq!(out, b"0");
        });
    }

    #[test]
    fn printf_format_octal_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%06o", 42);
            assert_eq!(out, b"000052");
        });
    }

    #[test]
    fn printf_format_octal_left_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%-6o", 42);
            assert_eq!(out, b"52    ");
        });
    }

    #[test]
    fn printf_format_hex_alt_flag() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#x", 42, false);
            assert_eq!(out, b"0x2a");

            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#X", 42, true);
            assert_eq!(out, b"0X2A");

            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#x", 0, false);
            assert_eq!(out, b"0");
        });
    }

    #[test]
    fn printf_format_hex_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%08x", 42, false);
            assert_eq!(out, b"0000002a");
        });
    }

    #[test]
    fn printf_format_hex_left_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%-8x", 42, false);
            assert_eq!(out, b"2a      ");
        });
    }

    #[test]
    fn printf_format_hex_alt_zero_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#010x", 255, false);
            assert_eq!(out, b"0x000000ff");
        });
    }

    #[test]
    fn printf_format_hex_alt_right_padded() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#10x", 255, false);
            assert_eq!(out, b"      0xff");
        });
    }

    #[test]
    fn printf_format_reuse_with_multiple_args() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"a ".to_vec())],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"b ".to_vec())],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                invoke(
                    &mut shell,
                    &[
                        b"printf".to_vec(),
                        b"%s ".to_vec(),
                        b"a".to_vec(),
                        b"b".to_vec(),
                    ],
                )
                .expect("printf reuse");
            },
        );
    }

    #[test]
    fn printf_check_trailing_garbage() {
        assert_no_syscalls(|| {
            assert!(printf_find_trailing_garbage(b"123abc").is_some());
            assert!(printf_find_trailing_garbage(b"123").is_none());
            assert!(printf_find_trailing_garbage(b"0xFG").is_some());
            assert!(printf_find_trailing_garbage(b"0xFF").is_none());
            assert!(printf_find_trailing_garbage(b"089").is_some());
            assert!(printf_find_trailing_garbage(b"077").is_none());
        });
    }

    #[test]
    fn printf_missing_format_operand() {
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(b"meiksh: printf: missing format operand\n".to_vec()),
                ],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"printf".to_vec()]).expect("printf no args");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn printf_unknown_conversion() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"%Q".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &[b"printf".to_vec(), b"%Q".to_vec()]).expect("printf %Q");
            },
        );
    }

    #[test]
    fn printf_incomplete_format_spec() {
        run_trace(
            vec![t(
                "write",
                vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"%5".to_vec())],
                TraceResult::Auto,
            )],
            || {
                let mut shell = test_shell();
                invoke(&mut shell, &[b"printf".to_vec(), b"%5".to_vec()])
                    .expect("printf %5 incomplete");
            },
        );
    }

    #[test]
    fn printf_format_parse_numeric_error() {
        let msg = diag(b"printf: abc: invalid number");
        run_trace(
            vec![
                trace_write_stderr(&msg),
                t(
                    "write",
                    vec![ArgMatcher::Fd(1), ArgMatcher::Bytes(b"0".to_vec())],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"printf".to_vec(), b"%d".to_vec(), b"abc".to_vec()],
                )
                .expect("printf %d abc");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn printf_parse_int_hex() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"0xff"), Ok(255));
            assert_eq!(printf_parse_int(b"0XFF"), Ok(255));
        });
    }

    #[test]
    fn printf_parse_int_octal() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"010"), Ok(8));
        });
    }

    #[test]
    fn printf_parse_int_char_literal() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"'A"), Ok(65));
            assert_eq!(printf_parse_int(b"\"B"), Ok(66));
        });
    }

    #[test]
    fn printf_parse_int_empty() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b""), Ok(0));
        });
    }

    #[test]
    fn printf_parse_int_negative() {
        assert_no_syscalls(|| {
            assert_eq!(printf_parse_int(b"-42"), Ok(-42));
        });
    }

    #[test]
    fn printf_parse_int_invalid_hex() {
        assert_no_syscalls(|| {
            assert!(printf_parse_int(b"0xZZZ").is_err());
        });
    }

    #[test]
    fn printf_parse_int_invalid_octal() {
        assert_no_syscalls(|| {
            assert!(printf_parse_int(b"08").is_err());
        });
    }

    #[test]
    fn printf_format_signed_negative_zero_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_signed(&mut out, b"%08ld", -42);
            assert_eq!(out, b"-0000042");
        });
    }

    #[test]
    fn printf_format_unsigned_zero_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_unsigned(&mut out, b"%08lu", 42);
            assert_eq!(out, b"00000042");
        });
    }

    #[test]
    fn printf_format_unsigned_left_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_unsigned(&mut out, b"%-8lu", 42);
            assert_eq!(out, b"42      ");
        });
    }

    #[test]
    fn printf_format_octal_alt_zero() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%#o", 0);
            assert_eq!(out, b"0");
        });
    }

    #[test]
    fn printf_format_octal_zero_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%08o", 255);
            assert_eq!(out, b"00000377");
        });
    }

    #[test]
    fn printf_format_octal_left_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_octal(&mut out, b"%-8o", 255);
            assert_eq!(out, b"377     ");
        });
    }

    #[test]
    fn printf_format_hex_upper_alt() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%#X", 255, true);
            assert_eq!(out, b"0XFF");
        });
    }

    #[test]
    fn printf_format_hex_zero_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%08x", 255, false);
            assert_eq!(out, b"000000ff");
        });
    }

    #[test]
    fn printf_format_hex_left_pad() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_hex(&mut out, b"%-10x", 255, false);
            assert_eq!(out, b"ff        ");
        });
    }

    #[test]
    fn printf_format_string_left_flag() {
        assert_no_syscalls(|| {
            let mut out = Vec::new();
            printf_format_string(&mut out, b"%-10", b"abc");
            assert_eq!(out, b"abc       ");
        });
    }
}
