use std::borrow::Cow;

use crate::bstr;
use crate::syntax::word_parts::{BracedOp, ExpansionKind, WordPart};

use super::arithmetic::eval_arithmetic;
use super::core::{Context, ExpandError};
use super::glob::pattern_matches;
use super::model::{is_glob_byte, render_pattern_from_segments, QuoteState, Segment};
use super::parameter::{lookup_param, require_set_parameter};
use super::word::trim_trailing_newlines;

#[derive(Debug)]
pub(super) struct ExpandOutput {
    pub(super) fields: Vec<Vec<u8>>,
    pub(super) current: Vec<u8>,
    current_has_glob: bool,
    had_quoted_content: bool,
    has_at_expansion: bool,
    had_quoted_null_outside_at: bool,
    at_field_breaks: Vec<usize>,
    at_empty: bool,
}

impl ExpandOutput {
    pub(super) fn new() -> Self {
        ExpandOutput {
            fields: Vec::new(),
            current: Vec::new(),
            current_has_glob: false,
            had_quoted_content: false,
            has_at_expansion: false,
            had_quoted_null_outside_at: false,
            at_field_breaks: Vec::new(),
            at_empty: false,
        }
    }

    pub(super) fn push_literal(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if is_glob_byte(b) {
                self.current_has_glob = true;
            }
            self.current.push(b);
        }
    }

    pub(super) fn push_quoted(&mut self, bytes: &[u8]) {
        self.had_quoted_content = true;
        self.had_quoted_null_outside_at = true;
        self.current.extend_from_slice(bytes);
    }

    pub(super) fn push_expanded(&mut self, bytes: &[u8], ifs: &[u8]) {
        if ifs.is_empty() {
            self.current.extend_from_slice(bytes);
            return;
        }
        let ifs_ws: Vec<u8> = ifs.iter().copied().filter(|b| b.is_ascii_whitespace()).collect();
        let ifs_other: Vec<u8> = ifs
            .iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();

        for &b in bytes {
            if ifs_other.contains(&b) {
                self.fields.push(std::mem::take(&mut self.current));
                self.current_has_glob = false;
            } else if ifs_ws.contains(&b) {
                if !self.current.is_empty() {
                    self.fields.push(std::mem::take(&mut self.current));
                    self.current_has_glob = false;
                }
            } else {
                if is_glob_byte(b) {
                    self.current_has_glob = true;
                }
                self.current.push(b);
            }
        }
    }

    pub(super) fn push_at_fields(&mut self, fields: Vec<Vec<u8>>) {
        self.has_at_expansion = true;
        if fields.is_empty() {
            self.at_empty = true;
        } else {
            self.had_quoted_content = true;
            for (i, field) in fields.into_iter().enumerate() {
                if i > 0 {
                    self.at_field_breaks.push(self.fields.len() + 1);
                    self.fields.push(std::mem::take(&mut self.current));
                    self.current_has_glob = false;
                }
                self.current.extend_from_slice(&field);
            }
        }
    }

    pub(super) fn finish(mut self) -> ExpandResult {
        if self.has_at_expansion {
            return self.finish_at_expansion();
        }

        if self.current.is_empty() && self.fields.is_empty() {
            if self.had_quoted_content {
                return ExpandResult::Fields(vec![Vec::new()]);
            }
            return ExpandResult::Fields(Vec::new());
        }

        if !self.current.is_empty() || self.fields.is_empty() {
            self.fields.push(self.current);
        }

        ExpandResult::FieldsWithGlob(
            self.fields
                .into_iter()
                .map(|f| FieldEntry {
                    text: f,
                    has_glob: self.current_has_glob,
                })
                .collect(),
        )
    }

    fn finish_at_expansion(mut self) -> ExpandResult {
        if self.at_empty && self.at_field_breaks.is_empty() {
            if !self.current.is_empty() || self.had_quoted_null_outside_at {
                let mut text = Vec::new();
                for f in &self.fields {
                    text.extend_from_slice(f);
                }
                text.extend_from_slice(&self.current);
                return ExpandResult::Fields(vec![text]);
            }
            return ExpandResult::Fields(Vec::new());
        }

        if self.at_field_breaks.is_empty() {
            let mut text = Vec::new();
            for f in &self.fields {
                text.extend_from_slice(f);
            }
            text.extend_from_slice(&self.current);
            return ExpandResult::Fields(vec![text]);
        }

        if !self.current.is_empty() {
            self.fields.push(std::mem::take(&mut self.current));
        }

        ExpandResult::Fields(self.fields)
    }
}

#[derive(Debug)]
pub(super) enum ExpandResult {
    Fields(Vec<Vec<u8>>),
    FieldsWithGlob(Vec<FieldEntry>),
}

#[derive(Debug)]
pub(super) struct FieldEntry {
    pub(super) text: Vec<u8>,
    pub(super) has_glob: bool,
}

pub(super) fn expand_parts<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
) -> Result<ExpandOutput, ExpandError> {
    let mut output = ExpandOutput::new();
    expand_parts_into(ctx, raw, parts, ifs, quoted, &mut output)?;
    Ok(output)
}

pub(super) fn expand_parts_into<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    for part in parts {
        match part {
            WordPart::Literal { start, end } => {
                let bytes = &raw[*start..*end];
                let is_tilde_candidate = bytes.starts_with(b"~")
                    && *start == 0
                    && !quoted
                    && (bytes.len() > 1 || parts.len() == 1);
                if is_tilde_candidate {
                    expand_tilde_in_literal(ctx, bytes, output);
                } else if bytes.contains(&b'\n') {
                    for &b in bytes {
                        if b == b'\n' {
                            ctx.inc_lineno();
                        }
                    }
                    output.push_literal(bytes);
                } else {
                    output.push_literal(bytes);
                }
            }
            WordPart::QuotedLiteral { bytes } => {
                for &b in bytes.iter() {
                    if b == b'\n' {
                        ctx.inc_lineno();
                    }
                }
                output.push_quoted(bytes);
            }
            WordPart::Tilde { end } => {
                let user = &raw[1..*end];
                expand_tilde(ctx, user, output);
            }
            WordPart::Expand { kind, quoted: q } => {
                let effective_quoted = quoted || *q;
                expand_kind(ctx, raw, kind, ifs, effective_quoted, output)?;
            }
        }
    }
    Ok(())
}

fn expand_tilde<C: Context>(ctx: &mut C, user: &[u8], output: &mut ExpandOutput) {
    if user.is_empty() {
        match ctx.env_var(b"HOME") {
            Some(home) if !home.is_empty() => {
                output.push_quoted(&home);
            }
            Some(_) => {
                output.push_quoted(b"");
            }
            None => {
                output.push_literal(b"~");
            }
        }
    } else if let Some(dir) = ctx.home_dir_for_user(user) {
        output.push_quoted(&dir);
    } else {
        output.push_literal(b"~");
        output.push_literal(user);
    }
}

fn expand_tilde_in_literal<C: Context>(ctx: &mut C, bytes: &[u8], output: &mut ExpandOutput) {
    if !bytes.starts_with(b"~") {
        output.push_literal(bytes);
        return;
    }
    let slash_pos = bytes.iter().position(|&b| b == b'/');
    let user_end = slash_pos.unwrap_or(bytes.len());
    let user = &bytes[1..user_end];
    expand_tilde(ctx, user, output);
    if user_end < bytes.len() {
        output.push_literal(&bytes[user_end..]);
    }
}

fn expand_kind<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    kind: &ExpansionKind,
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    match kind {
        ExpansionKind::SimpleVar { start, end } => {
            let name = &raw[*start..*end];
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            if quoted {
                output.push_quoted(value.as_bytes());
            } else {
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
        ExpansionKind::SpecialVar { ch } => {
            expand_special_var(ctx, *ch, ifs, quoted, output)?;
        }
        ExpansionKind::Braced {
            name_start,
            name_end,
            op,
            parts,
        } => {
            expand_braced(ctx, raw, *name_start, *name_end, *op, parts, ifs, quoted, output)?;
        }
        ExpansionKind::Command { program } => {
            let out = ctx.command_substitute(program)?;
            let trimmed = trim_trailing_newlines(&out);
            if quoted {
                output.push_quoted(trimmed);
            } else {
                output.push_expanded(trimmed, ifs);
            }
        }
        ExpansionKind::Arithmetic { parts } => {
            expand_arithmetic(ctx, raw, parts, ifs, quoted, output)?;
        }
        ExpansionKind::LiteralDollar => {
            if quoted {
                output.push_quoted(b"$");
            } else {
                output.push_literal(b"$");
            }
        }
    }
    Ok(())
}

fn expand_special_var<C: Context>(
    ctx: &mut C,
    ch: u8,
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    match ch {
        b'@' => {
            if quoted {
                let params = ctx.positional_params().to_vec();
                output.push_at_fields(params);
            } else {
                let joined = Cow::Owned(bstr::join_bstrings(ctx.positional_params(), b" "));
                let value = require_set_parameter(ctx, b"@", Some(joined))?;
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
        b'*' => {
            let ifs_val = ctx.env_var(b"IFS");
            let sep = match ifs_val.as_deref() {
                None => b" ".to_vec(),
                Some(b"") => Vec::new(),
                Some(s) => vec![s[0]],
            };
            let value = bstr::join_bstrings(ctx.positional_params(), &sep);
            if quoted {
                output.push_quoted(&value);
            } else {
                output.push_expanded(&value, ifs);
            }
        }
        b'0' => {
            let name = ctx.shell_name().to_vec();
            if quoted {
                output.push_quoted(&name);
            } else {
                output.push_expanded(&name, ifs);
            }
        }
        b'1'..=b'9' => {
            let index = (ch - b'0') as usize;
            let value = ctx.positional_param(index);
            let value = require_set_parameter(ctx, &[ch], value)?;
            if quoted {
                output.push_quoted(value.as_bytes());
            } else {
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
        _ => {
            let value = ctx.special_param(ch);
            let value = require_set_parameter(ctx, &[ch], value)?;
            if quoted {
                output.push_quoted(value.as_bytes());
            } else {
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
    }
    Ok(())
}

fn expand_braced<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    name_start: usize,
    name_end: usize,
    op: BracedOp,
    word_parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    let name = &raw[name_start..name_end];
    if name.is_empty() && op != BracedOp::None {
        return Err(ExpandError {
            message: b"bad substitution".as_ref().into(),
        });
    }
    match op {
        BracedOp::Length => {
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            let len = bstr::u64_to_bytes(value.len() as u64);
            if quoted {
                output.push_quoted(&len);
            } else {
                output.push_expanded(&len, ifs);
            }
        }
        BracedOp::None => {
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            if quoted {
                output.push_quoted(value.as_bytes());
            } else {
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
        BracedOp::Default | BracedOp::DefaultColon => {
            let value = lookup_param(ctx, name);
            let use_word = match &value {
                None => true,
                Some(v) if op == BracedOp::DefaultColon && v.is_empty() => true,
                _ => false,
            };
            let _ = require_set_parameter(ctx, name, value.clone());
            if use_word {
                expand_braced_word(ctx, raw, word_parts, ifs, quoted, output)?;
            } else {
                let val = value.unwrap();
                if quoted {
                    output.push_quoted(val.as_bytes());
                } else {
                    output.push_expanded(val.as_bytes(), ifs);
                }
            }
        }
        BracedOp::Assign | BracedOp::AssignColon => {
            let value = lookup_param(ctx, name);
            let use_word = match &value {
                None => true,
                Some(v) if op == BracedOp::AssignColon && v.is_empty() => true,
                _ => false,
            };
            if use_word {
                let expanded = expand_braced_word_text(ctx, raw, word_parts)?;
                ctx.set_var(name, &expanded)?;
                if quoted {
                    output.push_quoted(&expanded);
                } else {
                    output.push_expanded(&expanded, ifs);
                }
            } else {
                let val = value.unwrap();
                if quoted {
                    output.push_quoted(val.as_bytes());
                } else {
                    output.push_expanded(val.as_bytes(), ifs);
                }
            }
        }
        BracedOp::Error | BracedOp::ErrorColon => {
            let value = lookup_param(ctx, name);
            let trigger = match &value {
                None => true,
                Some(v) if op == BracedOp::ErrorColon && v.is_empty() => true,
                _ => false,
            };
            if trigger {
                let msg = if word_parts.is_empty() {
                    let mut m = name.to_vec();
                    m.extend_from_slice(b": parameter null or not set");
                    m
                } else {
                    let expanded = expand_braced_word_text(ctx, raw, word_parts)?;
                    let mut m = name.to_vec();
                    m.extend_from_slice(b": ");
                    m.extend_from_slice(&expanded);
                    m
                };
                return Err(ExpandError {
                    message: msg.into(),
                });
            }
            let val = value.unwrap();
            if quoted {
                output.push_quoted(val.as_bytes());
            } else {
                output.push_expanded(val.as_bytes(), ifs);
            }
        }
        BracedOp::Alt | BracedOp::AltColon => {
            let value = lookup_param(ctx, name);
            let use_word = match &value {
                None => false,
                Some(v) if op == BracedOp::AltColon && v.is_empty() => false,
                _ => true,
            };
            if use_word {
                expand_braced_word(ctx, raw, word_parts, ifs, quoted, output)?;
            }
        }
        BracedOp::TrimSuffix | BracedOp::TrimSuffixLong => {
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts)?;
            let trimmed = trim_suffix(value.as_bytes(), &pattern, op == BracedOp::TrimSuffixLong);
            if quoted {
                output.push_quoted(trimmed);
            } else {
                output.push_expanded(trimmed, ifs);
            }
        }
        BracedOp::TrimPrefix | BracedOp::TrimPrefixLong => {
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts)?;
            let trimmed = trim_prefix(value.as_bytes(), &pattern, op == BracedOp::TrimPrefixLong);
            if quoted {
                output.push_quoted(trimmed);
            } else {
                output.push_expanded(trimmed, ifs);
            }
        }
    }
    Ok(())
}

fn expand_braced_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    expand_parts_into(ctx, raw, word_parts, ifs, quoted, output)
}

fn expand_braced_word_text<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
) -> Result<Vec<u8>, ExpandError> {
    let mut out = ExpandOutput::new();
    expand_parts_into(ctx, raw, word_parts, b"", true, &mut out)?;
    let mut result = Vec::new();
    for f in &out.fields {
        result.extend_from_slice(f);
    }
    result.extend_from_slice(&out.current);
    Ok(result)
}

fn expand_braced_word_pattern<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
) -> Result<Vec<u8>, ExpandError> {
    let mut segments = Vec::new();
    build_pattern_segments(ctx, raw, word_parts, &mut segments)?;
    Ok(render_pattern_from_segments(&segments))
}

fn build_pattern_segments<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    segments: &mut Vec<Segment>,
) -> Result<(), ExpandError> {
    for part in parts {
        match part {
            WordPart::Literal { start, end } => {
                let bytes = &raw[*start..*end];
                segments.push(Segment::Text(bytes.to_vec(), QuoteState::Literal));
            }
            WordPart::QuotedLiteral { bytes } => {
                segments.push(Segment::Text(bytes.to_vec(), QuoteState::Quoted));
            }
            WordPart::Expand { kind, quoted } => {
                let mut temp = ExpandOutput::new();
                expand_kind(ctx, raw, kind, b"", *quoted, &mut temp)?;
                let mut text = Vec::new();
                for f in &temp.fields {
                    text.extend_from_slice(f);
                }
                text.extend_from_slice(&temp.current);
                let state = if *quoted {
                    QuoteState::Quoted
                } else {
                    QuoteState::Expanded
                };
                segments.push(Segment::Text(text, state));
            }
            WordPart::Tilde { .. } => {}
        }
    }
    Ok(())
}

fn expand_arithmetic<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    let mut expr_text = Vec::new();
    for part in parts {
        match part {
            WordPart::Literal { start, end } => {
                expr_text.extend_from_slice(&raw[*start..*end]);
            }
            WordPart::QuotedLiteral { bytes } => {
                expr_text.extend_from_slice(bytes);
            }
            WordPart::Expand { kind, .. } => {
                let mut temp = ExpandOutput::new();
                expand_kind(ctx, raw, kind, b"", true, &mut temp)?;
                for f in &temp.fields {
                    expr_text.extend_from_slice(f);
                }
                expr_text.extend_from_slice(&temp.current);
            }
            WordPart::Tilde { .. } => {}
        }
    }
    let saved_line = ctx.lineno();
    let value = eval_arithmetic(ctx, &expr_text)?;
    ctx.set_lineno(saved_line);
    let buf = bstr::I64Buf::new(value);
    if quoted {
        output.push_quoted(buf.as_bytes());
    } else {
        output.push_expanded(buf.as_bytes(), ifs);
    }
    Ok(())
}

fn trim_suffix<'a>(value: &'a [u8], pattern: &[u8], longest: bool) -> &'a [u8] {
    if longest {
        for i in 0..=value.len() {
            if pattern_matches(&value[i..], pattern) {
                return &value[..i];
            }
        }
    } else {
        for i in (0..=value.len()).rev() {
            if pattern_matches(&value[i..], pattern) {
                return &value[..i];
            }
        }
    }
    value
}

fn trim_prefix<'a>(value: &'a [u8], pattern: &[u8], longest: bool) -> &'a [u8] {
    if longest {
        for i in (0..=value.len()).rev() {
            if pattern_matches(&value[..i], pattern) {
                return &value[i..];
            }
        }
    } else {
        for i in 0..=value.len() {
            if pattern_matches(&value[..i], pattern) {
                return &value[i..];
            }
        }
    }
    value
}

trait AsBytes {
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for Vec<u8> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl AsBytes for Cow<'_, [u8]> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}
