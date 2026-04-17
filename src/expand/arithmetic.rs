use crate::bstr;
use crate::syntax::byte_class::{is_ascii_ws, is_digit, is_name_cont, is_name_start};

use super::core::{Context, ExpandError};
use super::model::Expansion;
use super::parameter::expand_dollar;
use super::word::{scan_backtick_command, trim_trailing_newlines};

pub(super) fn expand_arithmetic_expression<C: Context>(
    ctx: &mut C,
    expression: &[u8],
) -> Result<Vec<u8>, ExpandError> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < expression.len() {
        if expression[i] == b'$' {
            let (expansion, consumed) = expand_dollar(ctx, &expression[i..], true)?;
            match expansion {
                Expansion::One(s) => result.extend_from_slice(&s),
                Expansion::Static(s) => result.extend_from_slice(s),
                Expansion::AtFields(fields) => {
                    result.extend_from_slice(&bstr::join_bstrings(&fields, b" "));
                }
            }
            i += consumed;
        } else if expression[i] == b'`' {
            i += 1;
            let command = scan_backtick_command(expression, &mut i, true)?;
            let output = ctx.command_substitute_raw(&command)?;
            result.extend_from_slice(trim_trailing_newlines(&output));
        } else if expression[i] == b'\n' {
            ctx.inc_lineno();
            result.push(b'\n');
            i += 1;
        } else {
            result.push(expression[i]);
            i += 1;
        }
    }
    Ok(result)
}

pub(super) fn eval_arithmetic<C: Context>(
    ctx: &mut C,
    expression: &[u8],
) -> Result<i64, ExpandError> {
    let mut parser = ArithmeticParser::new(ctx, expression);
    let value = parser.parse_assignment()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ExpandError {
            message: b"unexpected trailing arithmetic tokens".as_ref().into(),
        });
    }
    Ok(value)
}

pub(super) struct ArithmeticParser<'a, 'src, C> {
    pub(super) source: &'src [u8],
    pub(super) index: usize,
    pub(super) ctx: &'a mut C,
    pub(super) start_line: usize,
    pub(super) skip_depth: usize,
}

pub(super) fn arith_err(msg: &[u8]) -> ExpandError {
    ExpandError {
        message: msg.into(),
    }
}

impl<'a, 'src, C: Context> ArithmeticParser<'a, 'src, C> {
    pub(super) fn new(ctx: &'a mut C, raw: &'src [u8]) -> Self {
        let start_line = ctx.lineno();
        Self {
            source: raw,
            index: 0,
            ctx,
            start_line,
            skip_depth: 0,
        }
    }

    fn error_at_current(&mut self, msg: &[u8]) -> ExpandError {
        let newlines = self.source[..self.index.min(self.source.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        self.ctx.set_lineno(self.start_line + newlines);
        arith_err(msg)
    }

    fn parse_assignment(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let save = self.index;
        if let Some(name) = self.try_scan_name() {
            self.skip_ws();
            if let Some(op) = self.try_consume_assign_op() {
                let rhs = self.parse_assignment()?;
                if self.skip_depth > 0 {
                    return Ok(rhs);
                }
                let value = if op == b"=" {
                    rhs
                } else {
                    let lhs = self.resolve_var(&name)?;
                    apply_compound_assign(&op, lhs, rhs)?
                };
                let buf = bstr::I64Buf::new(value);
                self.ctx
                    .set_var(name, buf.as_bytes())
                    .map_err(|e| ExpandError { message: e.message })?;
                return Ok(value);
            }
            self.index = save;
        }
        self.parse_ternary()
    }

    fn try_consume_assign_op(&mut self) -> Option<&'static [u8]> {
        let remaining = &self.source[self.index..];
        for op in &[
            b"<<=".as_ref(),
            b">>=",
            b"&=",
            b"^=",
            b"|=",
            b"*=",
            b"/=",
            b"%=",
            b"+=",
            b"-=",
            b"=",
        ] {
            if remaining.starts_with(op) {
                if *op == b"=" && remaining.starts_with(b"==") {
                    return None;
                }
                self.index += op.len();
                return Some(op);
            }
        }
        None
    }

    fn parse_ternary(&mut self) -> Result<i64, ExpandError> {
        let cond = self.parse_logical_or()?;
        self.skip_ws();
        if self.consume(b'?') {
            if cond == 0 {
                self.skip_depth += 1;
            }
            let then_val = self.parse_assignment()?;
            if cond == 0 {
                self.skip_depth -= 1;
            }
            self.skip_ws();
            if !self.consume(b':') {
                return Err(self.error_at_current(b"expected ':' in ternary expression"));
            }
            if cond != 0 {
                self.skip_depth += 1;
            }
            let else_val = self.parse_assignment()?;
            if cond != 0 {
                self.skip_depth -= 1;
            }
            Ok(if cond != 0 { then_val } else { else_val })
        } else {
            Ok(cond)
        }
    }

    fn parse_logical_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_logical_and()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"||") {
                if value != 0 {
                    self.skip_depth += 1;
                    let _ = self.parse_logical_and()?;
                    self.skip_depth -= 1;
                    value = 1;
                } else {
                    let rhs = self.parse_logical_and()?;
                    value = i64::from(rhs != 0);
                }
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_logical_and(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_or()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"&&") {
                if value == 0 {
                    self.skip_depth += 1;
                    let _ = self.parse_bitwise_or()?;
                    self.skip_depth -= 1;
                } else {
                    let rhs = self.parse_bitwise_or()?;
                    value = i64::from(rhs != 0);
                }
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_xor()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'|')
                && self.peek_at(1) != Some(b'|')
                && self.peek_at(1) != Some(b'=')
            {
                self.index += 1;
                value |= self.parse_bitwise_xor()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_xor(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_bitwise_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'^') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value ^= self.parse_bitwise_and()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_bitwise_and(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_equality()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'&')
                && self.peek_at(1) != Some(b'&')
                && self.peek_at(1) != Some(b'=')
            {
                self.index += 1;
                value &= self.parse_equality()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_relational()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"==") {
                value = i64::from(value == self.parse_relational()?);
            } else if self.consume_bytes(b"!=") {
                value = i64::from(value != self.parse_relational()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_relational(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_shift()?;
        loop {
            self.skip_ws();
            if self.consume_bytes(b"<=") {
                value = i64::from(value <= self.parse_shift()?);
            } else if self.consume_bytes(b">=") {
                value = i64::from(value >= self.parse_shift()?);
            } else if self.peek() == Some(b'<') && self.peek_at(1) != Some(b'<') {
                self.index += 1;
                value = i64::from(value < self.parse_shift()?);
            } else if self.peek() == Some(b'>') && self.peek_at(1) != Some(b'>') {
                self.index += 1;
                value = i64::from(value > self.parse_shift()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_shift(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_additive()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'<')
                && self.peek_at(1) == Some(b'<')
                && self.peek_at(2) != Some(b'=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shl(rhs as u32);
            } else if self.peek() == Some(b'>')
                && self.peek_at(1) == Some(b'>')
                && self.peek_at(2) != Some(b'=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shr(rhs as u32);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_additive(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_multiplicative()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'+') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.peek() == Some(b'-') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_sub(self.parse_multiplicative()?);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'*') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.peek() == Some(b'/') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error_at_current(b"division by zero"));
                }
                value /= rhs;
            } else if self.peek() == Some(b'%') && self.peek_at(1) != Some(b'=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error_at_current(b"division by zero"));
                }
                value %= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume(b'+') {
            return self.parse_unary();
        }
        if self.consume(b'-') {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.consume(b'~') {
            return Ok(!self.parse_unary()?);
        }
        if self.peek() == Some(b'!') && self.peek_at(1) != Some(b'=') {
            self.index += 1;
            return Ok(i64::from(self.parse_unary()? == 0));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume(b'(') {
            let value = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(b')') {
                return Err(self.error_at_current(b"missing ')'"));
            }
            return Ok(value);
        }

        if let Some(name) = self.try_scan_name() {
            return self.resolve_var(&name);
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let start = self.index;
        if self.peek() == Some(b'0') {
            self.index += 1;
            if self.peek() == Some(b'x') || self.peek() == Some(b'X') {
                self.index += 1;
                let hex_start = self.index;
                while self.index < self.source.len() && self.source[self.index].is_ascii_hexdigit()
                {
                    self.index += 1;
                }
                if self.index == hex_start {
                    return Err(self.error_at_current(b"invalid hex constant"));
                }
                return bstr::parse_hex_i64(&self.source[hex_start..self.index])
                    .ok_or_else(|| self.error_at_current(b"invalid hex constant"));
            }
            if self.peek().map_or(false, |c| is_digit(c)) {
                while self.index < self.source.len() && is_digit(self.source[self.index]) {
                    self.index += 1;
                }
                return bstr::parse_octal_i64(&self.source[start + 1..self.index])
                    .ok_or_else(|| self.error_at_current(b"invalid octal constant"));
            }
            return Ok(0);
        }

        while self.index < self.source.len() && is_digit(self.source[self.index]) {
            self.index += 1;
        }
        if start == self.index {
            return Err(self.error_at_current(b"expected arithmetic operand"));
        }
        bstr::parse_i64(&self.source[start..self.index])
            .ok_or_else(|| self.error_at_current(b"invalid arithmetic operand"))
    }

    fn try_scan_name(&mut self) -> Option<&'src [u8]> {
        self.skip_ws();
        let start = self.index;
        if self.index < self.source.len() {
            let b = self.source[self.index];
            if is_name_start(b) {
                self.index += 1;
                while self.index < self.source.len() {
                    let b2 = self.source[self.index];
                    if is_name_cont(b2) {
                        self.index += 1;
                    } else {
                        break;
                    }
                }
                return Some(&self.source[start..self.index]);
            }
        }
        None
    }

    fn resolve_var(&mut self, name: &[u8]) -> Result<i64, ExpandError> {
        let val_opt = self.ctx.env_var(name);
        if val_opt.is_none() && self.ctx.nounset_enabled() {
            let mut msg = Vec::new();
            msg.extend_from_slice(name);
            msg.extend_from_slice(b": parameter not set");
            return Err(self.error_at_current(&msg));
        }
        let val_bytes = val_opt.unwrap_or_default();
        if val_bytes.is_empty() {
            return Ok(0);
        }
        let trimmed = trim_ascii_whitespace(&val_bytes);
        let parsed = if trimmed.starts_with(b"0x") || trimmed.starts_with(b"0X") {
            bstr::parse_hex_i64(&trimmed[2..])
        } else if trimmed.starts_with(b"0")
            && trimmed.len() > 1
            && trimmed[1..].iter().all(|&b| is_digit(b))
        {
            bstr::parse_octal_i64(&trimmed[1..])
        } else {
            bstr::parse_i64(trimmed)
        };
        parsed.ok_or_else(|| {
            let mut msg = Vec::new();
            msg.extend_from_slice(b"invalid variable value for '");
            msg.extend_from_slice(name);
            msg.push(b'\'');
            self.error_at_current(&msg)
        })
    }

    fn skip_ws(&mut self) {
        while self.index < self.source.len() && is_ascii_ws(self.source[self.index]) {
            self.index += 1;
        }
    }

    fn consume(&mut self, ch: u8) -> bool {
        if self.source.get(self.index) == Some(&ch) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_bytes(&mut self, s: &[u8]) -> bool {
        if self.source[self.index..].starts_with(s) {
            self.index += s.len();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.index + offset).copied()
    }

    pub(super) fn is_eof(&self) -> bool {
        self.index >= self.source.len()
    }
}

pub(super) fn trim_ascii_whitespace(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|b| !is_ascii_ws(*b)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !is_ascii_ws(*b))
        .map_or(start, |p| p + 1);
    &s[start..end]
}

pub(super) fn apply_compound_assign(op: &[u8], lhs: i64, rhs: i64) -> Result<i64, ExpandError> {
    match op {
        b"+=" => Ok(lhs.wrapping_add(rhs)),
        b"-=" => Ok(lhs.wrapping_sub(rhs)),
        b"*=" => Ok(lhs.wrapping_mul(rhs)),
        b"/=" => {
            if rhs == 0 {
                return Err(arith_err(b"division by zero"));
            }
            Ok(lhs / rhs)
        }
        b"%=" => {
            if rhs == 0 {
                return Err(arith_err(b"division by zero"));
            }
            Ok(lhs % rhs)
        }
        b"<<=" => Ok(lhs.wrapping_shl(rhs as u32)),
        b">>=" => Ok(lhs.wrapping_shr(rhs as u32)),
        b"&=" => Ok(lhs & rhs),
        b"^=" => Ok(lhs ^ rhs),
        b"|=" => Ok(lhs | rhs),
        _ => {
            let mut msg = b"unknown assignment operator '".to_vec();
            msg.extend_from_slice(op);
            msg.push(b'\'');
            Err(arith_err(&msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::test_support::FakeContext;
    use crate::expand::word::expand_word;
    use crate::syntax::ast::Word;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn arith_variable_reference() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"count".to_vec(), b"7".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((count + 3))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("arith var");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_dollar_variable_reference() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"n".to_vec(), b"5".to_vec());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$(($n * 2))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("arith $var");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_comparison_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 < 5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((5 < 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 <= 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((5 > 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 >= 5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 == 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 != 5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
    }

    #[test]
    fn arith_bitwise_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((6 & 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"2".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((6 | 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"7".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((6 ^ 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"5".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((~0))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"-1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 << 4))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"16".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((16 >> 2))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"4".as_ref()]
        );
    }

    #[test]
    fn arith_logical_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 && 1))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 && 0))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0 || 1))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0 || 0))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((!0))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((!5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
    }

    #[test]
    fn arith_logical_and_short_circuits() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(
            &mut ctx,
            &Word {
                raw: b"$((0 && (x = 5)))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_logical_or_short_circuits() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(
            &mut ctx,
            &Word {
                raw: b"$((1 || (x = 5)))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_ternary_short_circuits() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"0".to_vec());
        expand_word(
            &mut ctx,
            &Word {
                raw: b"$((1 ? 10 : (x = 99)))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap();
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"0");
    }

    #[test]
    fn arith_ternary_operator() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((1 ? 10 : 20))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"10".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0 ? 10 : 20))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"20".as_ref()]
        );
    }

    #[test]
    fn arith_assignment_operators() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"10".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x = 5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"5".as_ref()]
        );
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"5");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x += 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"8".as_ref()]
        );
        assert_eq!(ctx.env.get(b"x".as_ref()).unwrap(), b"8");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x -= 2))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"6".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x *= 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"18".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x /= 6))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"3".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x %= 2))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );

        ctx.env.insert(b"x".to_vec(), b"4".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x <<= 2))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"16".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x >>= 1))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"8".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x &= 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );

        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x |= 2))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"7".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x ^= 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"4".as_ref()]
        );
    }

    #[test]
    fn arith_hex_and_octal_constants() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0xff))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"255".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0X1A))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"26".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((010))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"8".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((0))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
    }

    #[test]
    fn arith_unary_plus() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((+5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"5".as_ref()]
        );
    }

    #[test]
    fn arith_unset_variable_is_zero() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((nosuch))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
    }

    #[test]
    fn arith_nested_parens_and_precedence() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((2 + 3 * 4))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"14".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$(((2 + 3) * 4))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"20".as_ref()]
        );
    }

    #[test]
    fn arith_variable_in_hex_value() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"h".to_vec(), b"0xff".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((h))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"255".as_ref()]
        );
    }

    #[test]
    fn arith_variable_in_octal_value() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"o".to_vec(), b"010".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((o))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"8".as_ref()]
        );
    }

    #[test]
    fn arith_backtick_in_expression() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((`7` + 3))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("arith backtick");
        assert_eq!(fields, vec![b"10".as_ref()]);
    }

    #[test]
    fn arith_not_equal_via_parse_unary() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((3 != 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
    }

    #[test]
    fn arith_compound_assign_div_by_zero() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((x /= 0))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap_err();
        assert_eq!(&*err.message, b"division by zero".as_ref());

        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((x %= 0))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap_err();
        assert_eq!(&*err.message, b"division by zero".as_ref());
    }

    #[test]
    fn arith_equality_not_confused_with_assignment() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"x".to_vec(), b"5".to_vec());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x == 5))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"1".as_ref()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: b"$((x == 3))".as_ref().into(),
                    parts: Box::new([]),
                    line: 0
                }
            )
            .unwrap(),
            vec![b"0".as_ref()]
        );
    }

    #[test]
    fn arith_ternary_missing_colon_error() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((1 ? 2 3))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap_err();
        assert!(err.message.windows(3).any(|w| w == b"':'"));
    }

    #[test]
    fn arith_invalid_hex_constant() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((0x))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap_err();
        assert!(err.message.windows(3).any(|w| w == b"hex"));
    }

    #[test]
    fn arith_at_fields_in_expression() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![b"3".to_vec()];
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: b"$(($@ + 2))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .expect("at fields arith");
        assert_eq!(fields, vec![b"5".as_ref()]);
    }

    #[test]
    fn apply_compound_assign_unknown_op_returns_error() {
        let err = apply_compound_assign(b"??=", 1, 2).unwrap_err();
        assert!(err.message.windows(7).any(|w| w == b"unknown"));
    }

    #[test]
    fn expand_arithmetic_with_literal_newlines() {
        let mut ctx = FakeContext::new();
        let result = expand_arithmetic_expression(&mut ctx, b"1\n+\n2").expect("newline arith");
        assert_eq!(result, b"1\n+\n2");
    }

    #[test]
    fn expand_arithmetic_expression_static_literal_dollar() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let result = expand_arithmetic_expression(&mut ctx, b"$ ").expect("static dollar");
            assert_eq!(result, b"$ ");
        });
    }

    #[test]
    fn arith_invalid_variable_value_error() {
        let mut ctx = FakeContext::new();
        ctx.env.insert(b"bad".to_vec(), b"notanumber".to_vec());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: b"$((bad + 1))".as_ref().into(),
                parts: Box::new([]),
                line: 0,
            },
        )
        .unwrap_err();
        assert!(err.message.windows(7).any(|w| w == b"invalid"));
    }
}
