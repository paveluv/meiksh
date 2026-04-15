use std::borrow::Cow;

use crate::bstr;

use super::arithmetic::{eval_arithmetic, expand_arithmetic_expression};
use super::core::{Context, ExpandError};
use super::glob::pattern_matches;
use super::model::{Expansion, Segment, flatten_segments, render_pattern_from_segments};
use super::word::{expand_parameter_text_owned, expand_raw, trim_trailing_newlines};

pub(super) fn expand_dollar<C: Context>(
    ctx: &mut C,
    source: &[u8],
    quoted: bool,
) -> Result<(Expansion, usize), ExpandError> {
    if source.len() < 2 {
        return Ok((Expansion::One(b"$".to_vec()), 1));
    }

    let c1 = source[1];
    match c1 {
        b'\'' if !quoted => {
            let (s, n) = parse_dollar_single_quoted(source)?;
            Ok((Expansion::One(s), n))
        }
        b'{' => {
            let end = scan_to_closing_brace(source, 2)?;
            let expr = &source[2..end];
            let expansion = expand_braced_parameter(ctx, expr, quoted)?;
            Ok((expansion, end + 1))
        }
        b'(' => {
            if source.get(2) == Some(&b'(') {
                let mut index = 3usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source[index];
                    if ch == b'(' {
                        depth += 1;
                    } else if ch == b')' {
                        if depth == 1 && source.get(index + 1) == Some(&b')') {
                            let expression = source[3..index].to_vec();
                            let saved_line = ctx.lineno();
                            let pre_expanded = expand_arithmetic_expression(ctx, &expression)?;
                            ctx.set_lineno(saved_line);
                            let value = eval_arithmetic(ctx, &pre_expanded)?;
                            let buf = bstr::I64Buf::new(value);
                            return Ok((Expansion::One(buf.as_bytes().to_vec()), index + 2));
                        }
                        depth = depth.saturating_sub(1);
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: b"unterminated arithmetic expansion".as_ref().into(),
                })
            } else {
                let mut index = 2usize;
                let mut depth = 1usize;
                while index < source.len() {
                    let ch = source[index];
                    if ch == b'(' {
                        depth += 1;
                    } else if ch == b')' {
                        depth -= 1;
                        if depth == 0 {
                            let command = source[2..index].to_vec();
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = trim_trailing_newlines(&output).to_vec();
                            return Ok((Expansion::One(trimmed), index + 1));
                        }
                    }
                    index += 1;
                }
                Err(ExpandError {
                    message: b"unterminated command substitution".as_ref().into(),
                })
            }
        }
        b'@' => {
            if quoted {
                let params = ctx.positional_params().to_vec();
                Ok((Expansion::AtFields(params), 2))
            } else {
                let joined = Cow::Owned(bstr::join_bstrings(ctx.positional_params(), b" "));
                let value = require_set_parameter(ctx, b"@", Some(joined))?;
                Ok((Expansion::One(value), 2))
            }
        }
        b'*' => {
            let ifs = ctx.env_var(b"IFS");
            let sep = match ifs.as_deref() {
                None => b" ".to_vec(),
                Some(b"") => Vec::new(),
                Some(s) => vec![s[0]],
            };
            let value = bstr::join_bstrings(ctx.positional_params(), &sep);
            Ok((Expansion::One(value), 2))
        }
        b'?' | b'$' | b'!' | b'#' | b'-' | b'0' => {
            let ch_name = &source[1..2];
            let value = if c1 == b'0' {
                require_set_parameter(ctx, b"0", Some(Cow::Borrowed(ctx.shell_name())))?
            } else {
                require_set_parameter(ctx, ch_name, ctx.special_param(c1))?
            };
            Ok((Expansion::One(value), 2))
        }
        next if next.is_ascii_digit() => Ok((
            Expansion::One(require_set_parameter(
                ctx,
                &source[1..2],
                ctx.positional_param((next - b'0') as usize),
            )?),
            2,
        )),
        next if next == b'_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source[index];
                if b == b'_' || b.is_ascii_alphanumeric() {
                    index += 1;
                } else {
                    break;
                }
            }
            let name = &source[1..index];
            Ok((
                Expansion::One(require_set_parameter(ctx, name, lookup_param(ctx, name))?),
                index,
            ))
        }
        _ => Ok((Expansion::One(b"$".to_vec()), 1)),
    }
}

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
            Ok((value, 2))
        }
        next if next.is_ascii_digit() => {
            let value = ctx.positional_param((next - b'0') as usize);
            Ok((require_set_parameter(ctx, &source[1..2], value)?, 2))
        }
        next if next == b'_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            while index < source.len() {
                let b = source[index];
                if b == b'_' || b.is_ascii_alphanumeric() {
                    index += 1;
                } else {
                    break;
                }
            }
            let name = &source[1..index];
            Ok((
                require_set_parameter(ctx, name, lookup_param(ctx, name))?,
                index,
            ))
        }
        _ => Ok((b"$".to_vec(), 1)),
    }
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
    let digit = if b.is_ascii_digit() {
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

pub(super) fn expand_braced_parameter<C: Context>(
    ctx: &mut C,
    expr: &[u8],
    quoted: bool,
) -> Result<Expansion, ExpandError> {
    if expr == b"#" {
        return Ok(Expansion::One(
            lookup_param(ctx, b"#")
                .map(|c| c.into_owned())
                .unwrap_or_default(),
        ));
    }
    if expr.first() == Some(&b'#') && expr.len() > 1 {
        let name = &expr[1..];
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(Expansion::One(bstr::u64_to_bytes(value.len() as u64)));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    let value_owned = || {
        value
            .as_ref()
            .map(|c| c.clone().into_owned())
            .unwrap_or_default()
    };
    if op.is_none() {
        return Ok(Expansion::One(require_set_parameter(ctx, name, value)?));
    }
    let op_bytes = op.unwrap();
    let w = word.unwrap_or(b"");
    if op_bytes == b":-" {
        if !is_set || is_null {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"-" {
        if !is_set {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":=" {
        if !is_set || is_null {
            let val = assign_parameter(ctx, name, w, quoted)?;
            Ok(Expansion::One(val))
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"=" {
        if !is_set {
            let val = assign_parameter(ctx, name, w, quoted)?;
            Ok(Expansion::One(val))
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":?" {
        if !is_set || is_null {
            let default_msg = {
                let mut m = Vec::new();
                m.extend_from_slice(name);
                m.extend_from_slice(b": parameter null or not set");
                m
            };
            let raw = match word {
                Some(w2) if !w2.is_empty() => w2,
                _ => &default_msg,
            };
            let message = expand_parameter_word(ctx, raw, quoted)?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b"?" {
        if !is_set {
            let default_msg = {
                let mut m = Vec::new();
                m.extend_from_slice(name);
                m.extend_from_slice(b": parameter not set");
                m
            };
            let raw = match word {
                Some(w2) if !w2.is_empty() => w2,
                _ => &default_msg,
            };
            let message = expand_parameter_word(ctx, raw, quoted)?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(Expansion::One(value_owned()))
        }
    } else if op_bytes == b":+" {
        if is_set && !is_null {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(Vec::new()))
        }
    } else if op_bytes == b"+" {
        if is_set {
            expand_parameter_word_as_expansion(ctx, w, quoted)
        } else {
            Ok(Expansion::One(Vec::new()))
        }
    } else if op_bytes == b"%" || op_bytes == b"%%" || op_bytes == b"#" || op_bytes == b"##" {
        let val = require_set_parameter(ctx, name, value)?;
        let pat = expand_parameter_pattern_word(ctx, w)?;
        let mode = if op_bytes == b"%" {
            PatternRemoval::SmallestSuffix
        } else if op_bytes == b"%%" {
            PatternRemoval::LargestSuffix
        } else if op_bytes == b"#" {
            PatternRemoval::SmallestPrefix
        } else {
            PatternRemoval::LargestPrefix
        };
        Ok(Expansion::One(remove_parameter_pattern(val, &pat, mode)?))
    } else {
        Err(ExpandError {
            message: b"unsupported parameter expansion".as_ref().into(),
        })
    }
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
        return Ok(bstr::u64_to_bytes(value.len() as u64));
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    let value_owned = || {
        value
            .as_ref()
            .map(|c| c.clone().into_owned())
            .unwrap_or_default()
    };
    if op.is_none() {
        return require_set_parameter(ctx, name, value);
    }
    let op_bytes = op.unwrap();
    let w = word.unwrap_or(b"");
    if op_bytes == b":-" {
        if !is_set || is_null {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"-" {
        if !is_set {
            expand_parameter_text_owned(ctx, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b":=" {
        if !is_set || is_null {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"=" {
        if !is_set {
            assign_parameter_text(ctx, name, w)
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b":?" {
        if !is_set || is_null {
            let message =
                expand_parameter_error_text(ctx, name, word, b"parameter null or not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(value_owned())
        }
    } else if op_bytes == b"?" {
        if !is_set {
            let message = expand_parameter_error_text(ctx, name, word, b"parameter not set")?;
            Err(ExpandError {
                message: message.into(),
            })
        } else {
            Ok(value_owned())
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
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestSuffix,
        )
    } else if op_bytes == b"%%" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestSuffix,
        )
    } else if op_bytes == b"#" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::SmallestPrefix,
        )
    } else if op_bytes == b"##" {
        remove_parameter_pattern(
            require_set_parameter(ctx, name, value)?,
            &expand_parameter_text_owned(ctx, w)?,
            PatternRemoval::LargestPrefix,
        )
    } else {
        Err(ExpandError {
            message: b"unsupported parameter expansion".as_ref().into(),
        })
    }
}

pub(super) fn assign_parameter<C: Context>(
    ctx: &mut C,
    name: &[u8],
    raw_word: &[u8],
    quoted: bool,
) -> Result<Vec<u8>, ExpandError> {
    if !is_name(name) {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": cannot assign in parameter expansion");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    let value = expand_parameter_word(ctx, raw_word, quoted)?;
    ctx.set_var(name, &value)?;
    Ok(value)
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

pub(super) fn expand_parameter_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    _quoted: bool,
) -> Result<Vec<u8>, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_segments(&expanded.segments))
}

pub(super) fn expand_parameter_word_as_expansion<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    _quoted: bool,
) -> Result<Expansion, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    let has_at = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak | Segment::AtEmpty));
    if has_at {
        let mut fields = Vec::new();
        let mut current = Vec::new();
        for seg in &expanded.segments {
            match seg {
                Segment::Text(s, _) => current.extend_from_slice(s),
                Segment::AtBreak => {
                    fields.push(std::mem::take(&mut current));
                }
                Segment::AtEmpty => {}
            }
        }
        fields.push(current);
        Ok(Expansion::AtFields(fields))
    } else {
        Ok(Expansion::One(flatten_segments(&expanded.segments)))
    }
}

pub(super) fn expand_parameter_pattern_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
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
    let name: &[u8] = if b0.is_ascii_digit() {
        while index < expr.len() && expr[index].is_ascii_digit() {
            index += 1;
        }
        &expr[..index]
    } else if matches!(b0, b'?' | b'$' | b'!' | b'#' | b'*' | b'@') {
        index = 1;
        &expr[..index]
    } else if b0 == b'_' || b0.is_ascii_alphabetic() {
        while index < expr.len() && (expr[index] == b'_' || expr[index].is_ascii_alphanumeric()) {
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
    if !name.is_empty() && name.iter().all(|b| b.is_ascii_digit()) {
        return bstr::parse_i64(name)
            .and_then(|n| if n >= 0 { Some(n as usize) } else { None })
            .and_then(|index| ctx.positional_param(index));
    }
    if name.len() == 1 {
        if let Some(value) = ctx.special_param(name[0]) {
            return Some(value);
        }
    }
    ctx.env_var(name)
}

pub(super) fn require_set_parameter<C: Context>(
    ctx: &C,
    name: &[u8],
    value: Option<Cow<'_, [u8]>>,
) -> Result<Vec<u8>, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != b"@" && name != b"*" {
        let mut msg = Vec::new();
        msg.extend_from_slice(name);
        msg.extend_from_slice(b": parameter not set");
        return Err(ExpandError {
            message: msg.into(),
        });
    }
    Ok(value.map(|c| c.into_owned()).unwrap_or_default())
}

pub(super) enum PatternRemoval {
    SmallestSuffix,
    LargestSuffix,
    SmallestPrefix,
    LargestPrefix,
}

pub(super) fn remove_parameter_pattern(
    value: Vec<u8>,
    pattern: &[u8],
    mode: PatternRemoval,
) -> Result<Vec<u8>, ExpandError> {
    let boundaries: Vec<usize> = (0..=value.len()).collect();
    match mode {
        PatternRemoval::SmallestPrefix => {
            for &end in &boundaries {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_vec());
                }
            }
        }
        PatternRemoval::LargestPrefix => {
            for &end in boundaries.iter().rev() {
                if pattern_matches(&value[..end], pattern) {
                    return Ok(value[end..].to_vec());
                }
            }
        }
        PatternRemoval::SmallestSuffix => {
            for &start in boundaries.iter().rev() {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_vec());
                }
            }
        }
        PatternRemoval::LargestSuffix => {
            for &start in &boundaries {
                if pattern_matches(&value[start..], pattern) {
                    return Ok(value[..start].to_vec());
                }
            }
        }
    }
    Ok(value)
}

pub(super) fn is_name(name: &[u8]) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }
    name[1..]
        .iter()
        .all(|&b| b == b'_' || b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ByteArena;
    use crate::expand::model::Expansion;
    use crate::expand::test_support::{DefaultPathContext, FakeContext, expect_one};
    use crate::expand::word::{expand_parameter_text, expand_word, expand_word_text};
    use crate::syntax::ast::Word;
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
    fn supports_parameter_operators_and_positionals() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.positional = vec![
            b"a".to_vec(),
            b"b".to_vec(),
            b"c".to_vec(),
            b"d".to_vec(),
            b"e".to_vec(),
            b"f".to_vec(),
            b"g".to_vec(),
            b"h".to_vec(),
            b"i".to_vec(),
            b"j".to_vec(),
        ];

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${10}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"j".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$10".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![b"a0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${#10}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"1".as_ref()]
        );
        ctx.env.insert(b"IFS".to_vec(), b":".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$*".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![
                b"a".as_ref(),
                b"b".as_ref(),
                b"c".as_ref(),
                b"d".as_ref(),
                b"e".as_ref(),
                b"f".as_ref(),
                b"g".as_ref(),
                b"h".as_ref(),
                b"i".as_ref(),
                b"j".as_ref()
            ]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"a:b:c:d:e:f:g:h:i:j".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET:-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${EMPTY-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${EMPTY:-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${USER:+alt}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"alt".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET+alt}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            Vec::<&[u8]>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${NEW:=value}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("expand"),
            vec![b"value".as_ref()]
        );
        assert_eq!(
            ctx.env.get(b"NEW".as_ref()).map(|v| v.as_slice()),
            Some(b"value".as_ref())
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${#}".as_ref().into(),
                    line: 0
                },
                &arena
            )
            .expect("expand"),
            vec![b"10".as_ref()]
        );

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?boom}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("unset error");
        assert_eq!(&*error.message, b"boom".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?$'unterminated}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon-question word expansion error");
        assert!(!error.message.is_empty());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${MISSING?$'unterminated}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("plain-question word expansion error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn nounset_option_rejects_plain_unset_parameter_expansions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"$UNSET".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset variable");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset braced");
        assert_eq!(&*error.message, b"UNSET: parameter not set".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"$9".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("nounset positional");
        assert_eq!(&*error.message, b"9: parameter not set".as_ref());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${UNSET-word}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("default still works"),
            vec![b"word".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"\"$*\"".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("star exempt"),
            vec![b"alpha beta".as_ref()]
        );
    }

    #[test]
    fn direct_expand_dollar_covers_fallbacks_and_nesting() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$", false)),
            (b"$".to_vec(), 1)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$-", false)),
            (b"aC".to_vec(), 2)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, b"$$", false)),
            (b"".to_vec(), 2)
        );

        let (at_expansion, at_consumed) = expand_dollar(&mut ctx, b"$@", true).expect("quoted at");
        assert_eq!(at_consumed, 2);
        let Expansion::AtFields(fields) = at_expansion else {
            panic!("expected AtFields for quoted $@")
        };
        assert_eq!(fields, vec![b"alpha".to_vec(), b"beta".to_vec()]);

        let arithmetic_input = b"$((1 + (2 * 3)))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, arithmetic_input, false)),
            (b"7".to_vec(), arithmetic_input.len())
        );

        let command_input = b"$(printf (hi))";
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, command_input, false)),
            (b"printf (hi)".to_vec(), command_input.len())
        );
    }

    #[test]
    fn parameter_text_expansion_avoids_command_substitution() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());

        assert_eq!(
            expand_parameter_text(&mut ctx, b"${HOME:-/fallback}/.shrc", &arena)
                .expect("parameter text"),
            b"/tmp/home/.shrc"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"${EMPTY:-$HOME}/nested", &arena)
                .expect("nested default"),
            b"/tmp/home/nested"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, b"$(printf nope)${HOME}", &arena)
                .expect("literal command"),
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
        let mut ctx = FakeContext::new();

        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:-word", false).expect("default set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:=word", false).expect("assign set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"MISSING=value", false).expect("assign unset"),
            Expansion::One(b"value".to_vec())
        );
        assert_eq!(
            ctx.env.get(b"MISSING".as_ref()).map(|v| v.as_slice()),
            Some(b"value".as_ref())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER=value", false).expect("assign set"),
            Expansion::One(b"meiksh".to_vec())
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER?boom", false).expect("question set"),
            Expansion::One(b"meiksh".to_vec())
        );
        let error =
            expand_braced_parameter(&mut ctx, b"UNSET?boom", false).expect_err("question unset");
        assert_eq!(&*error.message, b"boom".as_ref());
        assert_eq!(
            expand_braced_parameter(&mut ctx, b"USER:?boom", false).expect("colon question set"),
            Expansion::One(b"meiksh".to_vec())
        );

        let error = assign_parameter(&mut ctx, b"1", b"value", false).expect_err("invalid assign");
        assert_eq!(
            &*error.message,
            b"1: cannot assign in parameter expansion".as_ref()
        );

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

        let error =
            expand_braced_parameter(&mut ctx, b"USER/tail", false).expect_err("unsupported expr");
        assert_eq!(&*error.message, b"unsupported parameter expansion".as_ref());
    }

    #[test]
    fn supports_pattern_removal_parameter_expansions() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env
            .insert(b"PATHNAME".to_vec(), b"src/bin/main.rs".to_vec());
        ctx.env
            .insert(b"DOTTED".to_vec(), b"alpha.beta.gamma".to_vec());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME#*/}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("small prefix"),
            vec![b"bin/main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME##*/}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("large prefix"),
            vec![b"main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME%/*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("small suffix"),
            vec![b"src/bin".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME%%/*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("large suffix"),
            vec![b"src".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${PATHNAME#\"src/\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted pattern"),
            vec![b"bin/main.rs".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED#*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("wildcard prefix"),
            vec![b"beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED##*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("largest wildcard prefix"),
            vec![b"gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%.*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("wildcard suffix"),
            vec![b"alpha.beta".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%%.*}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("largest wildcard suffix"),
            vec![b"alpha".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED#\"*.\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted wildcard"),
            vec![b"alpha.beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${DOTTED%}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("empty suffix pattern"),
            vec![b"alpha.beta.gamma".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"${MISSING%%*.}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("unset value"),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn brace_scanning_respects_quotes_and_nesting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"VAR".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\"a}b\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("quoted brace in default"),
            b"a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$(echo ok)}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("command sub in brace"),
            b"echo ok"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$((1+2))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("arith in brace"),
            b"3"
        );

        ctx.env.insert(b"INNER".to_vec(), b"val".to_vec());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-${INNER}}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested brace"),
            b"val"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-`echo hi`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("backtick in brace"),
            b"echo hi"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-'a}b'}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("single quote in brace"),
            b"a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\\}}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("escaped brace"),
            b"}"
        );
    }

    #[test]
    fn error_parameter_expansion_operators() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET:?custom error}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon question");
        assert_eq!(&*error.message, b"custom error".as_ref());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: b"${UNSET?also error}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("question");
        assert_eq!(&*error.message, b"also error".as_ref());
    }

    #[test]
    fn scan_to_closing_brace_error_on_unterminated() {
        let err = scan_to_closing_brace(b"${var", 2).expect_err("unterminated");
        assert_eq!(&*err.message, b"unterminated parameter expansion".as_ref());
    }

    #[test]
    fn brace_scanning_handles_complex_nesting() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"VAR".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$((2+3))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("arith in brace scan"),
            b"5"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-$(echo deep)}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("cmd sub in brace scan"),
            b"echo deep"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-`echo bt`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("backtick in brace scan"),
            b"echo bt"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${VAR:-\"inside\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("dq in brace scan with escape"),
            b"inside"
        );
    }

    #[test]
    fn error_parameter_expansion_with_null_or_not_set() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"EMPTY".to_vec(), Vec::new());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${EMPTY:?null or unset}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("colon question null");
        assert_eq!(&*err.message, b"null or unset".as_ref());

        let ok = expand_word(
            &mut ctx,
            &Word {
                raw: b"\"${EMPTY?not an error}\"".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect("question set but empty");
        assert_eq!(ok, vec![b"".as_ref()]);

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"${NOEXIST?custom msg}".as_ref().into(),
                line: 0,
            },
            &arena,
        )
        .expect_err("question unset");
        assert_eq!(&*err.message, b"custom msg".as_ref());
    }

    #[test]
    fn brace_scanning_with_arith_and_cmd_sub_and_backtick() {
        let arena = ByteArena::new();
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"V".to_vec(), Vec::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-$((1+(2*3)))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested arith in scan"),
            b"7"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-$(echo (hi))}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("nested cmd sub in scan"),
            b"echo (hi)"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-`echo \\\\x`}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("bt escape in scan"),
            b"echo \\x"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: b"${V:-\"q\\}x\"}".as_ref().into(),
                    line: 0
                },
                &arena,
            )
            .expect("dq escape in scan"),
            b"q}x"
        );
    }

    #[test]
    fn expand_braced_parameter_pattern_removal_operators() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.env.insert(b"FILE".to_vec(), b"archive.tar.gz".to_vec());

            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE%.*", false).unwrap(),
                Expansion::One(b"archive.tar".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE%%.*", false).unwrap(),
                Expansion::One(b"archive".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE#*.", false).unwrap(),
                Expansion::One(b"tar.gz".to_vec())
            );
            assert_eq!(
                expand_braced_parameter(&mut ctx, b"FILE##*.", false).unwrap(),
                Expansion::One(b"gz".to_vec())
            );
        });
    }

    #[test]
    fn scan_to_closing_brace_skips_backslash() {
        assert_no_syscalls(|| {
            let pos = scan_to_closing_brace(b"a\\}b}", 0).unwrap();
            assert_eq!(pos, 4);
        });
    }

    #[test]
    fn expand_parameter_word_as_expansion_with_at_fields() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.positional = vec![b"x".to_vec(), b"y".to_vec()];
            let result = expand_parameter_word_as_expansion(&mut ctx, b"\"$@\"", false).unwrap();
            assert_eq!(
                result,
                Expansion::AtFields(vec![b"x".to_vec(), b"y".to_vec()])
            );
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
}
