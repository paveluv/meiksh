use std::borrow::Cow;

use crate::bstr;
use crate::syntax::byte_class::{is_digit, is_name, is_name_cont, is_name_start};

use super::core::{Context, ExpandError};
use super::glob::pattern_matches_with_offsets;
use super::word::expand_parameter_text_owned;

pub(super) fn expand_parameter_dollar<C: Context>(
    ctx: &mut C,
    source: &[u8],
) -> Result<(Vec<u8>, usize), ExpandError> {
    if source.len() < 2 {
        return Ok((b"$".to_vec(), 1));
    }

    let c1 = source[1];
    match c1 {
        b'\'' => parse_dollar_single_quoted(source),
        b'{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let value = expand_braced_parameter_text(ctx, expr)?;
            Ok((value, end + 1))
        }
        b'?' | b'$' | b'!' | b'#' | b'*' | b'@' | b'-' | b'0' => {
            let ch_name = &source[1..2];
            let value = if c1 == b'0' {
                require_set_parameter(ctx, b"0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_name, ctx.special_param(c1))?
            };
            Ok((value.into_owned(), 2))
        }
        next if is_digit(next) => {
            let value = ctx.positional_param((next - b'0') as usize);
            Ok((
                require_set_parameter(ctx, &source[1..2], value)?.into_owned(),
                2,
            ))
        }
        next if is_name_start(next) => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source[index];
                if is_name_cont(b) {
                    index += 1;
                } else {
                    break;
                }
            }
            let name = &source[1..index];
            Ok((
                require_set_parameter(ctx, name, lookup_param(ctx, name))?.into_owned(),
                index,
            ))
        }
        _ => Ok((b"$".to_vec(), 1)),
    }
}

pub(crate) fn parse_dollar_single_quoted_body(body: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < body.len() {
        match body[index] {
            b'\\' => {
                index += 1;
                if index >= body.len() {
                    break;
                }
                let ch = body[index];
                match ch {
                    b'"' => result.push(b'"'),
                    b'\'' => result.push(b'\''),
                    b'\\' => result.push(b'\\'),
                    b'a' => result.push(0x07),
                    b'b' => result.push(0x08),
                    b'e' => result.push(0x1b),
                    b'f' => result.push(0x0c),
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'v' => result.push(0x0b),
                    b'c' => {
                        index += 1;
                        if index >= body.len() {
                            break;
                        }
                        if body[index] == b'\\' && index + 1 < body.len() {
                            index += 1;
                            result.push(control_escape(body[index]));
                        } else {
                            result.push(control_escape(body[index]));
                        }
                    }
                    b'x' => {
                        let (value, consumed) =
                            parse_variable_base_escape(&body[(index + 1)..], 16, 2);
                        if consumed == 0 {
                            result.push(b'x');
                        } else {
                            result.push(value);
                            index += consumed;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut digits = vec![ch];
                        let mut consumed = 0usize;
                        while consumed < 2
                            && index + 1 + consumed < body.len()
                            && matches!(body[index + 1 + consumed], b'0'..=b'7')
                        {
                            digits.push(body[index + 1 + consumed]);
                            consumed += 1;
                        }
                        let value = parse_octal_digits(&digits);
                        result.push(value);
                        index += consumed;
                    }
                    other => result.push(other),
                }
                index += 1;
            }
            _ => {
                result.push(body[index]);
                index += 1;
            }
        }
    }
    result
}

pub(super) fn parse_dollar_single_quoted(source: &[u8]) -> Result<(Vec<u8>, usize), ExpandError> {
    let mut index = 2usize;
    let mut result = Vec::new();
    while index < source.len() {
        match source[index] {
            b'\'' => return Ok((result, index + 1)),
            b'\\' => {
                index += 1;
                if index >= source.len() {
                    return Err(ExpandError {
                        message: b"unterminated dollar-single-quotes".as_ref().into(),
                    });
                }
                let ch = source[index];
                match ch {
                    b'"' => result.push(b'"'),
                    b'\'' => result.push(b'\''),
                    b'\\' => result.push(b'\\'),
                    b'a' => result.push(0x07),
                    b'b' => result.push(0x08),
                    b'e' => result.push(0x1b),
                    b'f' => result.push(0x0c),
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'v' => result.push(0x0b),
                    b'c' => {
                        index += 1;
                        if index >= source.len() {
                            return Err(ExpandError {
                                message: b"unterminated dollar-single-quotes".as_ref().into(),
                            });
                        }
                        if source[index] == b'\\' && index + 1 < source.len() {
                            index += 1;
                            result.push(control_escape(source[index]));
                        } else {
                            result.push(control_escape(source[index]));
                        }
                    }
                    b'x' => {
                        let (value, consumed) =
                            parse_variable_base_escape(&source[(index + 1)..], 16, 2);
                        if consumed == 0 {
                            result.push(b'x');
                        } else {
                            result.push(value);
                            index += consumed;
                        }
                    }
                    b'0'..=b'7' => {
                        let mut digits = vec![ch];
                        let mut consumed = 0usize;
                        while consumed < 2
                            && index + 1 + consumed < source.len()
                            && matches!(source[index + 1 + consumed], b'0'..=b'7')
                        {
                            digits.push(source[index + 1 + consumed]);
                            consumed += 1;
                        }
                        let value = parse_octal_digits(&digits);
                        result.push(value);
                        index += consumed;
                    }
                    other => result.push(other),
                }
                index += 1;
            }
            _ => {
                result.push(source[index]);
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: b"unterminated dollar-single-quotes".as_ref().into(),
    })
}

pub(super) fn parse_octal_digits(digits: &[u8]) -> u8 {
    let mut val: u8 = 0;
    for &d in digits {
        val = val.wrapping_mul(8).wrapping_add(d - b'0');
    }
    val
}

pub(super) fn scan_to_closing_brace(source: &[u8], start: usize) -> Result<usize, ExpandError> {
    let mut index = start;
    while index < source.len() {
        match source[index] {
            b'}' => return Ok(index),
            b'\\' => {
                index += 2;
            }
            b'\'' => {
                index += 1;
                while index < source.len() && source[index] != b'\'' {
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'"' => {
                index += 1;
                while index < source.len() && source[index] != b'"' {
                    if source[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            b'$' if source.get(index + 1) == Some(&b'{') => {
                index += 2;
                let inner = scan_to_closing_brace(source, index)?;
                index = inner + 1;
            }
            b'$' if source.get(index + 1) == Some(&b'(') => {
                if source.get(index + 2) == Some(&b'(') {
                    index += 3;
                    let mut depth = 1usize;
                    while index < source.len() {
                        if source[index] == b'(' {
                            depth += 1;
                        } else if source[index] == b')' {
                            if depth == 1 && source.get(index + 1) == Some(&b')') {
                                index += 2;
                                break;
                            }
                            depth = depth.saturating_sub(1);
                        }
                        index += 1;
                    }
                } else {
                    index += 2;
                    let mut depth = 1usize;
                    while index < source.len() {
                        if source[index] == b'(' {
                            depth += 1;
                        } else if source[index] == b')' {
                            depth -= 1;
                            if depth == 0 {
                                index += 1;
                                break;
                            }
                        }
                        index += 1;
                    }
                }
            }
            b'`' => {
                index += 1;
                while index < source.len() && source[index] != b'`' {
                    if source[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < source.len() {
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: b"unterminated parameter expansion".as_ref().into(),
    })
}

pub(super) fn control_escape(ch: u8) -> u8 {
    match ch {
        b'\\' => 0x1c,
        b'?' => 0x7f,
        other => other & 0x1f,
    }
}

pub(super) fn parse_variable_base_escape(
    source: &[u8],
    base: u32,
    max_digits: usize,
) -> (u8, usize) {
    let mut consumed = 0usize;
    while consumed < max_digits
        && consumed < source.len()
        && is_digit_for_base(source[consumed], base)
    {
        consumed += 1;
    }
    if consumed == 0 {
        return (0, 0);
    }
    let mut val: u8 = 0;
    for &b in &source[..consumed] {
        let digit = if b >= b'a' {
            b - b'a' + 10
        } else if b >= b'A' {
            b - b'A' + 10
        } else {
            b - b'0'
        };
        val = val.wrapping_mul(base as u8).wrapping_add(digit);
    }
    (val, consumed)
}

pub(super) fn is_digit_for_base(b: u8, base: u32) -> bool {
    let digit = if is_digit(b) {
        (b - b'0') as u32
    } else if b.is_ascii_lowercase() {
        (b - b'a') as u32 + 10
    } else if b.is_ascii_uppercase() {
        (b - b'A') as u32 + 10
    } else {
        return false;
    };
    digit < base
}

pub(super) fn expand_braced_parameter_text<C: Context>(
    ctx: &mut C,
    expr: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    if expr == b"#" {
        return Ok(lookup_param(ctx, b"#")
            .map(|c| c.into_owned())
            .unwrap_or_default());
    }
    if expr.first() == Some(&b'#') && expr.len() > 1 {
        let name = &expr[1..];
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(bstr::u64_to_bytes(crate::sys::locale::count_chars(&value)));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    if op.is_none() {
        return Ok(require_set_parameter(ctx, name, value)?.into_owned());
    }
    let op_bytes = op.unwrap();
    let w = word.unwrap_or(b"");
    let into_owned =
        |v: Option<Cow<'_, [u8]>>| -> Vec<u8> { v.map(Cow::into_owned).unwrap_or_default() };
    if op_bytes == b":-" {
        if !is_set || is_null {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b"-" {
        if !is_set {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b":=" {
        if !is_set || is_null {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b"=" {
        if !is_set {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b":?" {
        if !is_set || is_null {
            let message =
                expand_parameter_error_text(ctx, name, word, b"parameter null or not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b"?" {
        if !is_set {
            let message = expand_parameter_error_text(ctx, name, word, b"parameter not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(into_owned(value))
        }
    } else if op_bytes == b":+" {
        if is_set && !is_null {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(Vec::new())
        }
    } else if op_bytes == b"+" {
        if is_set {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(Vec::new())
        }
    } else if op_bytes == b"%" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?.into_owned(),
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestSuffix,
        )
    } else if op_bytes == b"%%" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?.into_owned(),
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestSuffix,
        )
    } else if op_bytes == b"#" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?.into_owned(),
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestPrefix,
        )
    } else if op_bytes == b"##" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?.into_owned(),
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestPrefix,
        )
    } else {
        Err(ExpandError {
            message: b"unsupported parameter expansion".as_ref().into(),
        })
    }
}

pub(super) fn assign_parameter_text<C: Context>(
    ctx: &mut C,
    name: &[u8],
    raw_word: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    if !is_name(name) {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": cannot assign in parameter expansion");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    let value = expand_parameter_text_owned(ctx, raw_word)?;
    ctx.set_var(name, &value)?;
    Ok(value)
}

pub(super) fn expand_parameter_error_text<C: Context>(
    ctx: &mut C,
    name: &[u8],
    word: Option<&[u8]>,
    default_message: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let owned;
    let raw = match word {
        Some(w) if !w.is_empty() => w,
        _ => {
            let mut m = Vec::new();
            m.extend_from_slice(name);
            m.extend_from_slice(b": ");
            m.extend_from_slice(default_message);
            owned = m;
            &owned
        }
    };
    expand_parameter_text_owned(ctx, raw)
}

pub(super) fn parse_parameter_expression(
    expr: &[u8],
) -> Result<(&[u8], Option<&[u8]>, Option<&[u8]>), ExpandError> {
    if expr.is_empty() {
        return Err(ExpandError {
            message: b"empty parameter expansion".as_ref().into(),
        });
    }
    let mut index = 0usize;
    let b0 = expr[0];
    let name: &[u8] = if is_digit(b0) {
        while index < expr.len() && is_digit(expr[index]) {
            index += 1;
        }
        &expr[..index]
    } else if matches!(b0, b'?' | b'$' | b'!' | b'#' | b'*' | b'@') {
        index = 1;
        &expr[..index]
    } else if is_name_start(b0) {
        while index < expr.len() && is_name_cont(expr[index]) {
            index += 1;
        }
        &expr[..index]
    } else {
        return Err(ExpandError {
            message: b"invalid parameter expansion".as_ref().into(),
        });
    };

    if index == expr.len() {
        return Ok((name, None, None));
    }

    let rest = &expr[index..];
    let (op, word): (&[u8], &[u8]) = match rest[0] {
        b':' if rest.len() > 1 => match rest[1] {
            b'-' => (b":-", &rest[2..]),
            b'=' => (b":=", &rest[2..]),
            b'?' => (b":?", &rest[2..]),
            b'+' => (b":+", &rest[2..]),
            _ => (&rest[..1], &rest[1..]),
        },
        b'%' if rest.len() > 1 && rest[1] == b'%' => (b"%%", &rest[2..]),
        b'#' if rest.len() > 1 && rest[1] == b'#' => (b"##", &rest[2..]),
        b'-' => (b"-", &rest[1..]),
        b'=' => (b"=", &rest[1..]),
        b'?' => (b"?", &rest[1..]),
        b'+' => (b"+", &rest[1..]),
        b'%' => (b"%", &rest[1..]),
        b'#' => (b"#", &rest[1..]),
        _ => (&rest[..1], &rest[1..]),
    };
    Ok((name, Some(op), Some(word)))
}

pub(super) fn lookup_param<'a, C: Context>(ctx: &'a C, name: &[u8]) -> Option<Cow<'a, [u8]>> {
    if name == b"0" {
        return Some(Cow::Borrowed(ctx.shell_name()));
    }
    if !name.is_empty() && name.iter().all(|&b| is_digit(b)) {
        return bstr::parse_i64(name)
            .and_then(|n| if n >= 0 { Some(n as usize) } else { None })
            .and_then(|index| ctx.positional_param(index));
    }
    if name.len() == 1
        && let Some(value) = ctx.special_param(name[0])
    {
        return Some(value);
    }
    ctx.env_var(name)
}

/// Variant of [`lookup_param`] that routes the plain `$NAME` env
/// lookup through a parse-time-stable [`CachedVarBinding`] so that
/// after the first expansion, the lookup bypasses the
/// `ShellMap<Vec<u8>, u32>` hash probe. The special-parameter and
/// positional-parameter fast paths are identical to `lookup_param`.
pub(super) fn lookup_param_cached<'a, C: Context>(
    ctx: &'a C,
    cache: &crate::shell::vars::CachedVarBinding,
    name: &[u8],
) -> Option<Cow<'a, [u8]>> {
    if name == b"0" {
        return Some(Cow::Borrowed(ctx.shell_name()));
    }
    if !name.is_empty() && name.iter().all(|&b| is_digit(b)) {
        return bstr::parse_i64(name)
            .and_then(|n| if n >= 0 { Some(n as usize) } else { None })
            .and_then(|index| ctx.positional_param(index));
    }
    if name.len() == 1
        && let Some(value) = ctx.special_param(name[0])
    {
        return Some(value);
    }
    ctx.env_var_cached(cache, name)
}

pub(super) fn require_set_parameter<'a, C: Context>(
    ctx: &C,
    name: &[u8],
    value: Option<Cow<'a, [u8]>>,
) -> Result<Cow<'a, [u8]>, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != b"@" && name != b"*" {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": parameter not set");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    Ok(value.unwrap_or(Cow::Borrowed(b"")))
}

pub(super) enum PatternRemoval {
    SmallestSuffix,
    LargestSuffix,
    SmallestPrefix,
    LargestPrefix,
}

pub(super) fn remove_parameter_pattern(
    mut value: Vec<u8>,
    pattern: &[u8],
    mode: PatternRemoval,
) -> Result<Vec<u8>, ExpandError> {
    let offsets = super::expand_parts::char_boundary_offsets(&value);
    match mode {
        PatternRemoval::SmallestPrefix => {
            for (k, &end) in offsets.iter().enumerate() {
                if pattern_matches_with_offsets(&value[..end], &offsets[..=k], 0, pattern) {
                    value.drain(..end);
                    return Ok(value);
                }
            }
        }
        PatternRemoval::LargestPrefix => {
            for (k, &end) in offsets.iter().enumerate().rev() {
                if pattern_matches_with_offsets(&value[..end], &offsets[..=k], 0, pattern) {
                    value.drain(..end);
                    return Ok(value);
                }
            }
        }
        PatternRemoval::SmallestSuffix => {
            for (k, &start) in offsets.iter().enumerate().rev() {
                if pattern_matches_with_offsets(&value[start..], &offsets[k..], start, pattern) {
                    value.truncate(start);
                    return Ok(value);
                }
            }
        }
        PatternRemoval::LargestSuffix => {
            for (k, &start) in offsets.iter().enumerate() {
                if pattern_matches_with_offsets(&value[start..], &offsets[k..], start, pattern) {
                    value.truncate(start);
                    return Ok(value);
                }
            }
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::test_support::{DefaultPathContext, FakeContext};
    use crate::expand::word::{expand_parameter_text, expand_word_text};
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn dollar_single_quote_helpers_cover_escape_matrix() {
        let input = b"$'\\\"\\'\\\\\\a\\b\\e\\f\\n\\r\\t\\v\\cA\\c\\\\\\x41\\101Z'";
        let (value, consumed) = parse_dollar_single_quoted(input).expect("parse");
        assert_eq!(consumed, input.len());
        let mut expected = Vec::new();
        expected.push(b'"');
        expected.push(b'\'');
        expected.push(b'\\');
        expected.push(0x07); // \a
        expected.push(0x08); // \b
        expected.push(0x1b); // \e
        expected.push(0x0c); // \f
        expected.push(b'\n');
        expected.push(b'\r');
        expected.push(b'\t');
        expected.push(0x0b); // \v
        expected.push(0x01); // \cA
        expected.push(0x1c); // \c\\
        expected.push(b'A'); // \x41
        expected.push(b'A'); // \101
        expected.push(b'Z');
        assert_eq!(value, expected);

        assert!(parse_dollar_single_quoted(b"$'\\").is_err());

        assert!(parse_dollar_single_quoted(b"$'\\c").is_err());

        let (value, _) = parse_dollar_single_quoted(b"$'\\xZ'").expect("parse no hex");
        assert_eq!(value, b"xZ");

        let (value, _) = parse_dollar_single_quoted(b"$'\\x41'").expect("parse hex");
        assert_eq!(value, b"A");

        let (value, _) = parse_dollar_single_quoted(b"$'\\z'").expect("parse unspecified");
        assert_eq!(value, b"z");

        assert_eq!(control_escape(b'\\'), 0x1c);
        assert_eq!(control_escape(b'?'), 0x7f);
        assert_eq!(control_escape(b'A'), 0x01);
        assert_eq!(parse_variable_base_escape(b"412", 16, 2), (0x41, 2));
        assert_eq!(parse_variable_base_escape(b"1017", 8, 3), (0o101, 3));
        assert_eq!(parse_variable_base_escape(b"Z", 16, 2), (0, 0));
    }
    #[test]
    fn parameter_text_expansion_avoids_command_substitution() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());

        assert_eq!(
            expand_parameter_text(&mut ctx, b"${HOME:-/fallback}/.shrc").expect("parameter text"),
            b"/tmp/home/.shrc"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"${EMPTY:-$HOME}/nested").expect("nested default"),
            b"/tmp/home/nested"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"$(printf nope)${HOME}").expect("literal command"),
            b"$(printf nope)/tmp/home"
        );
    }

    #[test]
    fn parameter_text_dollar_helpers_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$").expect("single"),
            (b"$".to_vec(), 1)
        );
        assert!(expand_parameter_dollar(&mut ctx, b"${HOME").is_err());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$0").expect("zero"),
            (b"meiksh".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$?").expect("special"),
            (b"0".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$1").expect("positional"),
            (b"alpha".to_vec(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$HOME").expect("name"),
            (b"/tmp/home".to_vec(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$HOME+rest").expect("name stops at +"),
            (b"/tmp/home".to_vec(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, b"$-").expect("dash"),
            (b"aC".to_vec(), 2)
        );
    }

    #[test]
    fn parameter_text_assignment_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"#").expect("hash"),
            b"2"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"#HOME").expect("length"),
            b"9"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME-word").expect("dash set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"UNSET-word").expect("dash unset"),
            b"word"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:=value").expect("colon assign set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"UNSET:=value").expect("assign unset"),
            b"value"
        );
        assert_eq!(
            ctx.env.get(b"UNSET".as_ref()).map(|v| v.as_slice()),
            Some(b"value".as_ref())
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING3=value").expect("assign equals unset"),
            b"value"
        );
        assert_eq!(
            ctx.env.get(b"MISSING3".as_ref()).map(|v| v.as_slice()),
            Some(b"value".as_ref())
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME=value").expect("assign set"),
            b"/tmp/home"
        );
        assert!(assign_parameter_text(&mut ctx, b"1", b"value").is_err());

        let err = expand_braced_parameter_text(&mut ctx, b"MISSING4?").expect_err("? no word");
        assert_eq!(&*err.message, b"MISSING4: parameter not set".as_ref());
        let text = expand_parameter_error_text(&mut ctx, b"X", Some(b""), b"my default")
            .expect("empty word");
        assert_eq!(text, b"X: my default");
    }

    #[test]
    fn nounset_option_rejects_length_and_pattern_expansions_of_unset_parameters() {
        let mut ctx = DefaultPathContext::new();
        ctx.nounset_enabled = true;

        let error = expand_braced_parameter_text(&mut ctx, b"#UNSET").expect_err("nounset length");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error =
            expand_braced_parameter_text(&mut ctx, b"UNSET%.*").expect_err("nounset pattern");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());
    }

    #[test]
    fn parameter_text_question_operator_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:?boom").expect("colon question set"),
            b"/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME?boom").expect("question set"),
            b"/tmp/home"
        );
        let colon_question = expand_braced_parameter_text(&mut ctx, b"EMPTY:?boom")
            .expect_err("colon question unset");
        assert_eq!(&*colon_question.message, b"boom".as_ref());
        let question =
            expand_braced_parameter_text(&mut ctx, b"MISSING?boom").expect_err("question unset");
        assert_eq!(&*question.message, b"boom".as_ref());
        let colon_default =
            expand_braced_parameter_text(&mut ctx, b"EMPTY:?").expect_err("colon default");
        assert_eq!(
            &*colon_default.message,
            b"EMPTY: parameter null or not set".as_ref()
        );
        let question_default =
            expand_braced_parameter_text(&mut ctx, b"MISSING?").expect_err("question default");
        assert_eq!(
            &*question_default.message,
            b"MISSING: parameter not set".as_ref()
        );
    }

    #[test]
    fn parameter_text_question_propagates_word_expansion_error() {
        let mut ctx = FakeContext::new();
        let err = expand_braced_parameter_text(&mut ctx, b"MISSING:?$'unterminated")
            .expect_err("colon-question text expansion error");
        assert!(!err.message.is_empty());
        let err = expand_braced_parameter_text(&mut ctx, b"MISSING?$'unterminated")
            .expect_err("plain-question text expansion error");
        assert!(!err.message.is_empty());
    }

    #[test]
    fn parameter_text_plus_and_pattern_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"DOTTED".to_vec(), b"alpha.beta".to_vec());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME:+alt").expect("colon plus"),
            b"alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING2:+alt").expect("colon plus unset"),
            b""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"HOME+alt").expect("plus set"),
            b"alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"MISSING2+alt").expect("plus unset"),
            b""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED%.*").expect("suffix"),
            b"alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED%%.*").expect("largest suffix"),
            b"alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED#*.").expect("prefix"),
            b"beta"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, b"DOTTED##*.").expect("largest prefix"),
            b"beta"
        );
        assert!(expand_braced_parameter_text(&mut ctx, b"HOME::word").is_err());
    }

    #[test]
    fn parameter_helpers_cover_more_edge_cases() {
        let parsed = parse_parameter_expression(b"@").expect("special name");
        assert_eq!(parsed, (b"@".as_ref(), None, None));

        let error = parse_parameter_expression(b"").expect_err("empty expr");
        assert_eq!(&*error.message, b"empty parameter expansion".as_ref());

        let error = parse_parameter_expression(b"%oops").expect_err("invalid expr");
        assert_eq!(&*error.message, b"invalid parameter expansion".as_ref());
        let parsed = parse_parameter_expression(b"USER%%tail").expect("largest suffix");
        assert_eq!(
            parsed,
            (
                b"USER".as_ref(),
                Some(b"%%".as_ref()),
                Some(b"tail".as_ref())
            )
        );
        let parsed = parse_parameter_expression(b"USER/tail").expect("unknown operator");
        assert_eq!(
            parsed,
            (
                b"USER".as_ref(),
                Some(b"/".as_ref()),
                Some(b"tail".as_ref())
            )
        );
    }
    #[test]
    fn scan_to_closing_brace_error_on_unterminated() {
        let err = scan_to_closing_brace(b"${var", 2).expect_err("unterminated");
        assert_eq!(&*err.message, b"unterminated parameter expansion".as_ref());
    }

    #[test]
    fn scan_to_closing_brace_skips_backslash() {
        assert_no_syscalls(|| {
            let pos = scan_to_closing_brace(b"a\\}b}", 0).unwrap();
            assert_eq!(pos, 4);
        });
    }

    #[test]
    fn scan_to_closing_brace_crosses_nested_constructs() {
        // These inputs intentionally force every major branch of
        // `scan_to_closing_brace`: single-quoted, double-quoted,
        // backslash-escaped, `$(...)`, `$((...))`, `${...}` nesting, and
        // backtick command substitution. Each body's closing `}` must
        // be located correctly.
        assert_no_syscalls(|| {
            // Single quote with literal `}` inside.
            assert_eq!(scan_to_closing_brace(b"'a}b'}tail", 0).unwrap(), 5);
            // Double quote with escaped `}` inside.
            assert_eq!(scan_to_closing_brace(b"\"a\\}b\"}tail", 0).unwrap(), 6);
            // ${...} nesting.
            assert_eq!(scan_to_closing_brace(b"${inner}}tail", 0).unwrap(), 8);
            // $(...) command substitution.
            assert_eq!(scan_to_closing_brace(b"$(echo )x}tail", 0).unwrap(), 9);
            // $((...)) arithmetic.
            assert_eq!(scan_to_closing_brace(b"$((1+2))}tail", 0).unwrap(), 8);
            // Backtick command substitution with escaped backtick.
            assert_eq!(scan_to_closing_brace(b"`a\\`b`}tail", 0).unwrap(), 6);
            // $(( ... )) with nested parentheses — forces the
            // `depth += 1` / saturating `depth - 1` arms.
            assert_eq!(scan_to_closing_brace(b"$(((1+2)*3))}t", 0).unwrap(), 12);
            // $( ... ) with nested parentheses — forces the
            // balanced-paren `depth += 1` / `break on depth == 0` arms.
            assert_eq!(scan_to_closing_brace(b"$(echo (x))}t", 0).unwrap(), 11);
        });
    }

    #[test]
    fn parse_variable_base_escape_hex_digit_conversion() {
        assert_eq!(parse_variable_base_escape(b"4F", 16, 2), (0x4F, 2));
        assert_eq!(parse_variable_base_escape(b"ff", 16, 2), (0xff, 2));
        assert_eq!(parse_variable_base_escape(b"a0", 16, 2), (0xa0, 2));
        assert_eq!(parse_variable_base_escape(b"A0", 16, 2), (0xA0, 2));
        assert_eq!(parse_variable_base_escape(b"00", 16, 2), (0x00, 2));
    }

    #[test]
    fn is_digit_for_base_covers_all_branches() {
        assert!(is_digit_for_base(b'0', 10));
        assert!(is_digit_for_base(b'9', 10));
        assert!(!is_digit_for_base(b'a', 10));
        assert!(is_digit_for_base(b'a', 16));
        assert!(is_digit_for_base(b'f', 16));
        assert!(!is_digit_for_base(b'g', 16));
        assert!(is_digit_for_base(b'A', 16));
        assert!(is_digit_for_base(b'F', 16));
        assert!(!is_digit_for_base(b'G', 16));
        assert!(!is_digit_for_base(b'!', 10));
        assert!(!is_digit_for_base(b' ', 16));
    }

    #[test]
    fn is_name_empty_input() {
        assert!(!is_name(b""));
    }

    fn parsed_word(source: &[u8]) -> crate::syntax::ast::Word {
        let prog = crate::syntax::parse(source).expect("parse");
        let item = &prog.items[0];
        let cmd = &item.and_or.first.commands[0];
        match cmd {
            crate::syntax::ast::Command::Simple(sc) => sc.words[1].clone(),
            _ => panic!("expected simple command"),
        }
    }
    #[test]
    fn tilde_expanded_in_braced_default_word() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"HOME".to_vec(), b"/my_home".to_vec());
            let w = parsed_word(b"echo ${missing:-~}\n");
            let text = expand_word_text(&mut ctx, &w).expect("expand_word_text must succeed");
            assert_eq!(text, b"/my_home");
        });
    }

    #[test]
    fn colon_equals_rejects_positional_parameter() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec![b"arg".to_vec()];
            let err = expand_word_text(&mut ctx, &parsed_word(b"echo ${2:=foo}\n"))
                .expect_err("${2:=foo} must error");
            assert!(!err.message.is_empty());
        });
    }

    #[test]
    fn colon_equals_rejects_special_parameter() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional.clear();
            let err = expand_word_text(&mut ctx, &parsed_word(b"echo ${*:=foo}\n"))
                .expect_err("${*:=foo} must error");
            assert!(!err.message.is_empty());
        });
    }
    #[test]
    fn braced_expansion_with_trailing_junk_errors() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let err = expand_word_text(&mut ctx, &parsed_word(b"echo ${x!y}\n"))
                .expect_err("${x!y} must error");
            assert_eq!(&*err.message, b"bad substitution");
        });
    }

    #[test]
    fn dollar_single_quote_trailing_backslash() {
        assert_no_syscalls(|| {
            let result = super::parse_dollar_single_quoted_body(b"abc\\");
            assert_eq!(result, b"abc");
        });
    }

    #[test]
    fn dollar_single_quote_trailing_ctrl_c() {
        assert_no_syscalls(|| {
            let result = super::parse_dollar_single_quoted_body(b"\\c");
            assert_eq!(result, b"");
        });
    }

    #[test]
    fn dollar_single_quote_ctrl_backslash_escape() {
        assert_no_syscalls(|| {
            let result = super::parse_dollar_single_quoted_body(b"\\c\\M");
            assert_eq!(result, &[0x0d]);
        });
    }

    #[test]
    fn lookup_param_cached_handles_zero_and_special_parameter() {
        // `lookup_param_cached` has bespoke fast paths for `$0` (the
        // shell name) and single-character special parameters that
        // short-circuit ahead of the cached env-var lookup.
        assert_no_syscalls(|| {
            let ctx = FakeContext::new();
            let cache = crate::shell::vars::CachedVarBinding::default();
            let zero = lookup_param_cached(&ctx, &cache, b"0").expect("$0");
            assert_eq!(&*zero, b"meiksh");
            // `$?` (last exit status) — a special parameter.
            let last = lookup_param_cached(&ctx, &cache, b"?").expect("$?");
            assert_eq!(&*last, b"0");
            // Positional `$1` is reached via the digit branch.
            let one = lookup_param_cached(&ctx, &cache, b"1").expect("$1");
            assert_eq!(&*one, b"alpha");
        });
    }

    #[test]
    fn parse_parameter_expression_accepts_multi_digit_positional() {
        // The digit-name loop in parse_parameter_expression is only reached
        // for names longer than one digit (e.g. `${12}`).  Assert the parser
        // returns the full digit run as the name component and the correct
        // trailing `op`/`word` slices.
        assert_no_syscalls(|| {
            let (name, op, word) = parse_parameter_expression(b"12").expect("12");
            assert_eq!(name, b"12");
            assert!(op.is_none() && word.is_none());

            let (name, op, word) = parse_parameter_expression(b"99:-default").expect("99:-default");
            assert_eq!(name, b"99");
            assert_eq!(op, Some(b":-".as_ref()));
            assert_eq!(word, Some(b"default".as_ref()));
        });
    }

    #[test]
    fn remove_parameter_pattern_no_match_returns_value_unchanged() {
        // When no prefix/suffix matches, `remove_parameter_pattern` falls
        // through to the final `Ok(value)` return with the value untouched.
        // Covers that fall-through for every mode.
        assert_no_syscalls(|| {
            for mode in [
                PatternRemoval::SmallestPrefix,
                PatternRemoval::LargestPrefix,
                PatternRemoval::SmallestSuffix,
                PatternRemoval::LargestSuffix,
            ] {
                let out =
                    remove_parameter_pattern(b"hello".to_vec(), b"xyz", mode).expect("no match");
                assert_eq!(out, b"hello", "no-match pattern must not mutate input");
            }
        });
    }
}
