/// Byte-string utilities for POSIX shell data.
///
/// POSIX shells must handle arbitrary byte sequences (0x01-0xFF) in
/// filenames, variable values, and command output. Rust's `String`/`str`
/// enforce UTF-8, which silently corrupts non-UTF-8 data. This module
/// provides thin helpers over `[u8]`/`Vec<u8>` for the shell's internal
/// string representation.
///
/// This module intentionally avoids `std::fmt`, `String`, `str`, `char`,
/// `Display`, `format!`, `write!`, and all UTF-8 types.

/// Owned byte string (analogous to `String`).
pub type BString = Vec<u8>;
/// Boxed byte string (analogous to `Box<str>`).
pub type BoxBStr = Box<[u8]>;

// ---------------------------------------------------------------------------
// Extension trait
// ---------------------------------------------------------------------------

pub trait BStrExt {
    fn trim_trailing_newlines(&self) -> &[u8];
    fn contains_byte(&self, byte: u8) -> bool;
    fn split_once_byte(&self, byte: u8) -> Option<(&[u8], &[u8])>;
    fn trim_ascii_ws(&self) -> &[u8];
}

impl BStrExt for [u8] {
    fn trim_trailing_newlines(&self) -> &[u8] {
        let mut end = self.len();
        while end > 0 && self[end - 1] == b'\n' {
            end -= 1;
        }
        &self[..end]
    }

    #[inline]
    fn contains_byte(&self, byte: u8) -> bool {
        self.contains(&byte)
    }

    fn split_once_byte(&self, byte: u8) -> Option<(&[u8], &[u8])> {
        let pos = self.iter().position(|&b| b == byte)?;
        Some((&self[..pos], &self[pos + 1..]))
    }

    fn trim_ascii_ws(&self) -> &[u8] {
        let start = self
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(self.len());
        let end = self
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map_or(start, |p| p + 1);
        &self[start..end]
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Create a CString from arbitrary bytes. Returns `Err` if the bytes contain
/// an interior NUL.
pub fn to_cstring(bytes: &[u8]) -> Result<std::ffi::CString, std::ffi::NulError> {
    std::ffi::CString::new(bytes.to_vec())
}

/// Convert raw bytes from the OS (CStr, readdir d_name, getcwd result, etc.)
/// into our internal `Vec<u8>` representation. Identity on Unix.
#[inline]
pub fn bytes_from_cstr(cstr: &std::ffi::CStr) -> BString {
    cstr.to_bytes().to_vec()
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a byte string as a decimal integer, returning None on failure.
pub fn parse_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let (negative, digits) = if s[0] == b'-' {
        (true, &s[1..])
    } else if s[0] == b'+' {
        (false, &s[1..])
    } else {
        (false, s)
    };
    if digits.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in digits {
        if !b.is_ascii_digit() {
            return None;
        }
        result = result.checked_mul(10)?;
        result = result.checked_add((b - b'0') as i64)?;
    }
    if negative {
        Some(-result)
    } else {
        Some(result)
    }
}

pub fn parse_hex_i64(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in bytes {
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

pub fn parse_octal_i64(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for &b in bytes {
        if b < b'0' || b > b'7' {
            return None;
        }
        result = result.checked_mul(8)?.checked_add((b - b'0') as i64)?;
    }
    Some(result)
}

/// POSIX shell name validation: `[A-Za-z_][A-Za-z0-9_]*`
pub fn is_name(s: &[u8]) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }
    s[1..].iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_')
}

// ---------------------------------------------------------------------------
// Numeric-to-bytes formatters (no std::fmt)
// ---------------------------------------------------------------------------

/// Append decimal representation of an i64.
pub fn push_i64(buf: &mut Vec<u8>, val: i64) {
    if val < 0 {
        buf.push(b'-');
        // Handle i64::MIN without overflow: cast to u64 first.
        let abs = (val as u64).wrapping_neg();
        push_u64(buf, abs);
    } else {
        push_u64(buf, val as u64);
    }
}

/// Return decimal bytes for an i64.
pub fn i64_to_bytes(val: i64) -> Vec<u8> {
    let mut buf = Vec::new();
    push_i64(&mut buf, val);
    buf
}

/// Append decimal representation of a u64.
pub fn push_u64(buf: &mut Vec<u8>, val: u64) {
    if val == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    let mut v = val;
    while v > 0 {
        buf.push(b'0' + (v % 10) as u8);
        v /= 10;
    }
    buf[start..].reverse();
}

/// Return decimal bytes for a u64.
pub fn u64_to_bytes(val: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    push_u64(&mut buf, val);
    buf
}

/// Append octal representation of a u64 (no prefix).
pub fn push_u64_octal(buf: &mut Vec<u8>, val: u64) {
    if val == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    let mut v = val;
    while v > 0 {
        buf.push(b'0' + (v & 7) as u8);
        v >>= 3;
    }
    buf[start..].reverse();
}

/// Append zero-padded octal to `width` digits (e.g. width=4 -> "0022").
pub fn push_u64_octal_padded(buf: &mut Vec<u8>, val: u64, width: usize) {
    let start = buf.len();
    push_u64_octal(buf, val);
    let digits = buf.len() - start;
    if digits < width {
        let pad = width - digits;
        buf.resize(buf.len() + pad, 0);
        buf.copy_within(start..start + digits, start + pad);
        for b in &mut buf[start..start + pad] {
            *b = b'0';
        }
    }
}

/// Append lowercase hex representation of a u64 (no prefix).
pub fn push_u64_hex(buf: &mut Vec<u8>, val: u64) {
    if val == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    let mut v = val;
    while v > 0 {
        let nibble = (v & 0xF) as u8;
        buf.push(if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        });
        v >>= 4;
    }
    buf[start..].reverse();
}

/// Append uppercase hex representation of a u64 (no prefix).
pub fn push_u64_hex_upper(buf: &mut Vec<u8>, val: u64) {
    if val == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    let mut v = val;
    while v > 0 {
        let nibble = (v & 0xF) as u8;
        buf.push(if nibble < 10 {
            b'0' + nibble
        } else {
            b'A' + nibble - 10
        });
        v >>= 4;
    }
    buf[start..].reverse();
}

/// Append fixed-point f64 with the given number of decimal places.
/// Handles negative values. Does NOT handle NaN/Inf specially —
/// those produce "0.000..." (acceptable for POSIX shell `time` output).
pub fn push_f64_fixed(buf: &mut Vec<u8>, val: f64, precision: usize) {
    if val.is_nan() || val.is_infinite() {
        push_u64(buf, 0);
        if precision > 0 {
            buf.push(b'.');
            for _ in 0..precision {
                buf.push(b'0');
            }
        }
        return;
    }

    let negative = val < 0.0;
    let val = if negative { -val } else { val };
    if negative {
        buf.push(b'-');
    }

    let mut multiplier = 1u64;
    for _ in 0..precision {
        multiplier *= 10;
    }

    let scaled = (val * multiplier as f64 + 0.5) as u64;
    let integer_part = scaled / multiplier;
    let frac_part = scaled % multiplier;

    push_u64(buf, integer_part);
    if precision > 0 {
        buf.push(b'.');
        // frac_part must be zero-padded to `precision` digits
        let start = buf.len();
        push_u64(buf, frac_part);
        let digits = buf.len() - start;
        if digits < precision {
            let pad = precision - digits;
            buf.resize(buf.len() + pad, 0);
            buf.copy_within(start..start + digits, start + pad);
            for b in &mut buf[start..start + pad] {
                *b = b'0';
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ByteWriter — ergonomic multi-part byte message builder
// ---------------------------------------------------------------------------

pub struct ByteWriter(pub Vec<u8>);

impl ByteWriter {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self(Vec::with_capacity(cap))
    }

    pub fn bytes(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }

    pub fn byte(mut self, b: u8) -> Self {
        self.0.push(b);
        self
    }

    pub fn u64_val(mut self, v: u64) -> Self {
        push_u64(&mut self.0, v);
        self
    }

    pub fn i64_val(mut self, v: i64) -> Self {
        push_i64(&mut self.0, v);
        self
    }

    pub fn i32_val(mut self, v: i32) -> Self {
        push_i64(&mut self.0, v as i64);
        self
    }

    pub fn usize_val(mut self, v: usize) -> Self {
        push_u64(&mut self.0, v as u64);
        self
    }

    pub fn f64_fixed(mut self, v: f64, prec: usize) -> Self {
        push_f64_fixed(&mut self.0, v, prec);
        self
    }

    pub fn octal_padded(mut self, v: u64, w: usize) -> Self {
        push_u64_octal_padded(&mut self.0, v, w);
        self
    }

    pub fn hex_lower(mut self, v: u64) -> Self {
        push_u64_hex(&mut self.0, v);
        self
    }

    pub fn hex_upper(mut self, v: u64) -> Self {
        push_u64_hex_upper(&mut self.0, v);
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }

    /// Borrow the accumulated bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Join helpers
// ---------------------------------------------------------------------------

/// Join byte slices with a separator byte.
pub fn join_bytes(parts: &[&[u8]], sep: u8) -> BString {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.extend_from_slice(part);
    }
    out
}

/// Join owned byte strings with a separator.
pub fn join_bstrings(parts: &[BString], sep: &[u8]) -> BString {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(sep);
        }
        out.extend_from_slice(part);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_trailing_newlines_removes_only_trailing() {
        assert_eq!(b"hello\nworld".trim_trailing_newlines(), b"hello\nworld");
        assert_eq!(b"hello\n\n".trim_trailing_newlines(), b"hello");
        assert_eq!(b"\n".trim_trailing_newlines(), b"");
        assert_eq!(b"".trim_trailing_newlines(), b"");
    }

    #[test]
    fn split_once_byte_works() {
        assert_eq!(
            b"key=value".split_once_byte(b'='),
            Some((b"key".as_ref(), b"value".as_ref()))
        );
        assert_eq!(b"noequals".split_once_byte(b'='), None);
        assert_eq!(
            b"=value".split_once_byte(b'='),
            Some((b"".as_ref(), b"value".as_ref()))
        );
    }

    #[test]
    fn parse_i64_basic() {
        assert_eq!(parse_i64(b"42"), Some(42));
        assert_eq!(parse_i64(b"-7"), Some(-7));
        assert_eq!(parse_i64(b"+3"), Some(3));
        assert_eq!(parse_i64(b""), None);
        assert_eq!(parse_i64(b"abc"), None);
        assert_eq!(parse_i64(b"12x"), None);
    }

    #[test]
    fn is_name_validates_posix_names() {
        assert!(is_name(b"foo"));
        assert!(is_name(b"_bar"));
        assert!(is_name(b"VAR_1"));
        assert!(!is_name(b""));
        assert!(!is_name(b"1abc"));
        assert!(!is_name(b"-x"));
    }

    #[test]
    fn trim_ascii_ws_works() {
        assert_eq!(b"  hello  ".trim_ascii_ws(), b"hello");
        assert_eq!(b"\t\n hi \n".trim_ascii_ws(), b"hi");
        assert_eq!(b"".trim_ascii_ws(), b"");
    }

    #[test]
    fn join_bytes_works() {
        let parts: &[&[u8]] = &[b"a", b"b", b"c"];
        assert_eq!(join_bytes(parts, b' '), b"a b c");
    }

    #[test]
    fn bytes_from_cstr_preserves_non_utf8() {
        let raw = std::ffi::CString::new(vec![0x80, 0xFF, 0x01]).unwrap();
        let result = bytes_from_cstr(&raw);
        assert_eq!(result, vec![0x80, 0xFF, 0x01]);
    }

    // ---- Numeric formatters ----

    #[test]
    fn push_u64_basic() {
        let mut buf = Vec::new();
        push_u64(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_u64(&mut buf, 12345);
        assert_eq!(buf, b"12345");
        buf.clear();
        push_u64(&mut buf, u64::MAX);
        assert_eq!(buf, b"18446744073709551615");
    }

    #[test]
    fn push_i64_basic() {
        let mut buf = Vec::new();
        push_i64(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_i64(&mut buf, -42);
        assert_eq!(buf, b"-42");
        buf.clear();
        push_i64(&mut buf, i64::MIN);
        assert_eq!(buf, b"-9223372036854775808");
        buf.clear();
        push_i64(&mut buf, i64::MAX);
        assert_eq!(buf, b"9223372036854775807");
    }

    #[test]
    fn push_u64_octal_basic() {
        let mut buf = Vec::new();
        push_u64_octal(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_u64_octal(&mut buf, 8);
        assert_eq!(buf, b"10");
        buf.clear();
        push_u64_octal(&mut buf, 0o755);
        assert_eq!(buf, b"755");
    }

    #[test]
    fn push_u64_octal_padded_basic() {
        let mut buf = Vec::new();
        push_u64_octal_padded(&mut buf, 0o22, 4);
        assert_eq!(buf, b"0022");
        buf.clear();
        push_u64_octal_padded(&mut buf, 0o755, 4);
        assert_eq!(buf, b"0755");
        buf.clear();
        push_u64_octal_padded(&mut buf, 0o7777, 4);
        assert_eq!(buf, b"7777");
    }

    #[test]
    fn push_u64_hex_basic() {
        let mut buf = Vec::new();
        push_u64_hex(&mut buf, 0);
        assert_eq!(buf, b"0");
        buf.clear();
        push_u64_hex(&mut buf, 255);
        assert_eq!(buf, b"ff");
        buf.clear();
        push_u64_hex(&mut buf, 0xDEAD);
        assert_eq!(buf, b"dead");
    }

    #[test]
    fn push_u64_hex_upper_basic() {
        let mut buf = Vec::new();
        push_u64_hex_upper(&mut buf, 0xCAFE);
        assert_eq!(buf, b"CAFE");
    }

    #[test]
    fn push_f64_fixed_basic() {
        let mut buf = Vec::new();
        push_f64_fixed(&mut buf, 1.5, 2);
        assert_eq!(buf, b"1.50");
        buf.clear();
        push_f64_fixed(&mut buf, 0.0, 3);
        assert_eq!(buf, b"0.000");
        buf.clear();
        push_f64_fixed(&mut buf, 123.456, 2);
        assert_eq!(buf, b"123.46");
        buf.clear();
        push_f64_fixed(&mut buf, -2.5, 1);
        assert_eq!(buf, b"-2.5");
        buf.clear();
        push_f64_fixed(&mut buf, 0.001, 3);
        assert_eq!(buf, b"0.001");
    }

    #[test]
    fn push_f64_fixed_zero_precision() {
        let mut buf = Vec::new();
        push_f64_fixed(&mut buf, 3.7, 0);
        assert_eq!(buf, b"4");
    }

    #[test]
    fn i64_to_bytes_convenience() {
        assert_eq!(i64_to_bytes(42), b"42");
        assert_eq!(i64_to_bytes(-1), b"-1");
    }

    #[test]
    fn u64_to_bytes_convenience() {
        assert_eq!(u64_to_bytes(0), b"0");
        assert_eq!(u64_to_bytes(999), b"999");
    }

    // ---- ByteWriter ----

    #[test]
    fn byte_writer_basic() {
        let result = ByteWriter::new()
            .bytes(b"hello ")
            .bytes(b"world")
            .byte(b'!')
            .finish();
        assert_eq!(result, b"hello world!");
    }

    #[test]
    fn byte_writer_numeric() {
        let result = ByteWriter::new()
            .bytes(b"[")
            .usize_val(1)
            .bytes(b"] Done(")
            .i32_val(42)
            .bytes(b")\n")
            .finish();
        assert_eq!(result, b"[1] Done(42)\n");
    }

    #[test]
    fn byte_writer_time_format() {
        let result = ByteWriter::new()
            .bytes(b"real ")
            .u64_val(2)
            .byte(b'm')
            .f64_fixed(3.141, 3)
            .byte(b's')
            .finish();
        assert_eq!(result, b"real 2m3.141s");
    }

    #[test]
    fn byte_writer_octal() {
        let result = ByteWriter::new().octal_padded(0o22, 4).finish();
        assert_eq!(result, b"0022");
    }
}
