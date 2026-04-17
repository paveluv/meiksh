use std::borrow::Cow;

use crate::bstr;
use crate::syntax::word_parts::{BracedName, BracedOp, ExpansionKind, WordPart};

use super::arithmetic::eval_arithmetic;
use super::core::{Context, ExpandError};
use super::glob::pattern_matches;
use super::model::{QuoteState, Segment, render_pattern_from_segments};
use super::parameter::{lookup_param, require_set_parameter};
use super::word::trim_trailing_newlines;
use crate::syntax::byte_class::{is_glob_char, is_name};

#[derive(Debug)]
pub(super) struct ExpandOutput {
    pub(super) fields: Vec<(Vec<u8>, bool)>,
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

    pub(super) fn push_literal_with_glob(&mut self, bytes: &[u8], has_glob: bool) {
        if has_glob {
            self.current_has_glob = true;
        }
        self.current.extend_from_slice(bytes);
    }

    pub(super) fn push_literal(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if is_glob_char(b) {
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

        let ifs_chars = decompose_ifs(ifs);
        let mut i = 0;
        while i < bytes.len() {
            if let Some((_, byte_seq, is_ws)) = find_ifs_char_at(&ifs_chars, &bytes[i..]) {
                if is_ws {
                    if !self.current.is_empty() {
                        let glob = self.current_has_glob;
                        self.fields.push((std::mem::take(&mut self.current), glob));
                        self.current_has_glob = false;
                    }
                } else {
                    let glob = self.current_has_glob;
                    self.fields.push((std::mem::take(&mut self.current), glob));
                    self.current_has_glob = false;
                }
                i += byte_seq.len();
            } else {
                let b = bytes[i];
                if is_glob_char(b) {
                    self.current_has_glob = true;
                }
                self.current.push(b);
                i += 1;
            }
        }
    }

    pub(super) fn push_value(&mut self, bytes: &[u8], quoted: bool, ifs: &[u8]) {
        if quoted {
            self.push_quoted(bytes);
        } else {
            self.push_expanded(bytes, ifs);
        }
    }

    pub(super) fn push_at_fields(&mut self, fields: &[Vec<u8>]) {
        self.has_at_expansion = true;
        if fields.is_empty() {
            self.at_empty = true;
        } else {
            self.had_quoted_content = true;
            for (i, field) in fields.iter().enumerate() {
                if i > 0 {
                    self.at_field_breaks.push(self.fields.len() + 1);
                    let glob = self.current_has_glob;
                    self.fields.push((std::mem::take(&mut self.current), glob));
                    self.current_has_glob = false;
                }
                self.current.extend_from_slice(field);
            }
        }
    }

    pub(super) fn drain_single_vec(&mut self) -> Vec<u8> {
        if self.fields.is_empty() {
            return std::mem::take(&mut self.current);
        }
        let total: usize =
            self.fields.iter().map(|(f, _)| f.len()).sum::<usize>() + self.current.len();
        let mut result = Vec::with_capacity(total);
        for (f, _) in &self.fields {
            result.extend_from_slice(f);
        }
        result.extend_from_slice(&self.current);
        self.fields.clear();
        self.current.clear();
        result
    }

    pub(super) fn clear(&mut self) {
        self.fields.clear();
        self.current.clear();
        self.current_has_glob = false;
        self.had_quoted_content = false;
        self.has_at_expansion = false;
        self.had_quoted_null_outside_at = false;
        self.at_field_breaks.clear();
        self.at_empty = false;
    }

    pub(super) fn finish(&mut self) -> ExpandResult {
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
            let glob = self.current_has_glob;
            self.fields.push((std::mem::take(&mut self.current), glob));
        }

        ExpandResult::FieldsWithGlob(
            self.fields
                .drain(..)
                .map(|(text, has_glob)| FieldEntry { text, has_glob })
                .collect(),
        )
    }

    fn finish_at_expansion(&mut self) -> ExpandResult {
        if self.at_empty && self.at_field_breaks.is_empty() {
            if !self.current.is_empty() || self.had_quoted_null_outside_at {
                return ExpandResult::Fields(vec![self.drain_single_vec()]);
            }
            return ExpandResult::Fields(Vec::new());
        }

        if self.at_field_breaks.is_empty() {
            return ExpandResult::Fields(vec![self.drain_single_vec()]);
        }

        if !self.current.is_empty() {
            self.fields.push((std::mem::take(&mut self.current), false));
        }

        ExpandResult::Fields(self.fields.drain(..).map(|(f, _)| f).collect())
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

pub(super) fn expand_parts_into_new<C: Context>(
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
            WordPart::Literal {
                start,
                end,
                has_glob,
                newlines,
            } => {
                let bytes = &raw[*start..*end];
                for _ in 0..*newlines {
                    ctx.inc_lineno();
                }
                output.push_literal_with_glob(bytes, *has_glob);
            }
            WordPart::QuotedLiteral { bytes, newlines } => {
                for _ in 0..*newlines {
                    ctx.inc_lineno();
                }
                output.push_quoted(bytes);
            }
            WordPart::TildeLiteral {
                tilde_pos,
                user_end,
                end,
            } => {
                let user = &raw[tilde_pos + 1..*user_end];
                let slash_follows = *user_end < *end && raw[*user_end] == b'/';
                expand_tilde(ctx, user, slash_follows, output);
                if *user_end < *end {
                    output.push_literal(&raw[*user_end..*end]);
                }
            }
            WordPart::Expansion { kind, quoted: q } => {
                let effective_quoted = quoted || *q;
                expand_kind(ctx, raw, kind, ifs, effective_quoted, output)?;
            }
        }
    }
    Ok(())
}

fn expand_tilde<C: Context>(
    ctx: &mut C,
    user: &[u8],
    slash_follows: bool,
    output: &mut ExpandOutput,
) {
    if user.is_empty() {
        match ctx.env_var(b"HOME") {
            Some(home) if !home.is_empty() => {
                let h = if slash_follows && home.ends_with(b"/") {
                    &home[..home.len() - 1]
                } else {
                    &home
                };
                output.push_quoted(h);
            }
            Some(_) => {
                output.push_quoted(b"");
            }
            None => {
                output.push_literal(b"~");
            }
        }
    } else if let Some(dir) = ctx.home_dir_for_user(user) {
        let d = if slash_follows && dir.ends_with(b"/") {
            &dir[..dir.len() - 1]
        } else {
            &dir
        };
        output.push_quoted(d);
    } else {
        output.push_literal(b"~");
        output.push_literal(user);
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
            output.push_value(value.as_bytes(), quoted, ifs);
        }
        ExpansionKind::Positional { index } => {
            let idx = *index as usize;
            let value = ctx.positional_param(idx);
            let value = require_set_parameter(ctx, &[b'0' + index], value)?;
            output.push_value(value.as_bytes(), quoted, ifs);
        }
        ExpansionKind::ShellName => {
            let name = ctx.shell_name();
            output.push_value(name, quoted, ifs);
        }
        ExpansionKind::SpecialVar { ch } => {
            expand_special_var(ctx, *ch, ifs, quoted, output)?;
        }
        ExpansionKind::Braced { name, op, parts } => {
            expand_braced(ctx, raw, name, *op, parts, ifs, quoted, output)?;
        }
        ExpansionKind::Command { program } => {
            let out = ctx.command_substitute(program)?;
            let trimmed = trim_trailing_newlines(&out);
            output.push_value(trimmed, quoted, ifs);
        }
        ExpansionKind::Arithmetic { parts } => {
            expand_arithmetic(ctx, raw, parts, ifs, quoted, output)?;
        }
        ExpansionKind::ArithmeticLiteral { start, end } => {
            let saved_line = ctx.lineno();
            let value = eval_arithmetic(ctx, &raw[*start..*end])?;
            ctx.set_lineno(saved_line);
            let buf = bstr::I64Buf::new(value);
            output.push_value(buf.as_bytes(), quoted, ifs);
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
                output.push_at_fields(ctx.positional_params());
            } else {
                let joined = Cow::Owned(bstr::join_bstrings(ctx.positional_params(), b" "));
                let value = require_set_parameter(ctx, b"@", Some(joined))?;
                output.push_expanded(value.as_bytes(), ifs);
            }
        }
        b'*' => {
            let ifs_cow = ctx.env_var(b"IFS");
            let sep: &[u8] = match &ifs_cow {
                None => b" ",
                Some(s) if s.is_empty() => b"",
                Some(s) => &s[..crate::sys::locale::first_char_len(s)],
            };
            let value = bstr::join_bstrings(ctx.positional_params(), sep);
            output.push_value(&value, quoted, ifs);
        }
        _ => {
            let value = ctx.special_param(ch);
            let value = require_set_parameter(ctx, &[ch], value)?;
            output.push_value(value.as_bytes(), quoted, ifs);
        }
    }
    Ok(())
}

fn lookup_braced_param<'a, C: Context>(
    ctx: &'a C,
    raw: &[u8],
    braced_name: &BracedName,
) -> Option<Cow<'a, [u8]>> {
    match braced_name {
        BracedName::Var { start, end } => {
            let name = &raw[*start..*end];
            lookup_param(ctx, name)
        }
        BracedName::Positional { index, .. } => ctx.positional_param(*index as usize),
        BracedName::Special { ch, .. } => {
            if *ch == b'#' {
                ctx.special_param(*ch)
            } else {
                ctx.special_param(*ch)
            }
        }
    }
}

fn braced_name_bytes<'a>(raw: &'a [u8], braced_name: &BracedName) -> &'a [u8] {
    let (start, end) = braced_name.name_range();
    &raw[start..end]
}

fn expand_braced<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    braced_name: &BracedName,
    op: BracedOp,
    word_parts: &[WordPart],
    ifs: &[u8],
    quoted: bool,
    output: &mut ExpandOutput,
) -> Result<(), ExpandError> {
    let name = braced_name_bytes(raw, braced_name);
    if name.is_empty() {
        return Err(ExpandError {
            message: b"bad substitution".as_ref().into(),
        });
    }
    match op {
        BracedOp::Length => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?;
            let len = bstr::u64_to_bytes(crate::sys::locale::count_chars(&value));
            output.push_value(&len, quoted, ifs);
        }
        BracedOp::None => {
            if !word_parts.is_empty() {
                return Err(ExpandError {
                    message: b"bad substitution".as_ref().into(),
                });
            }
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?;
            output.push_value(value.as_bytes(), quoted, ifs);
        }
        BracedOp::Default | BracedOp::DefaultColon => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let use_word = match &value {
                None => true,
                Some(v) if op == BracedOp::DefaultColon && v.is_empty() => true,
                _ => false,
            };
            if value.is_none() && ctx.nounset_enabled() && name != b"@" && name != b"*" {
                // nounset side-effect only; default word will be used
            }
            if use_word {
                expand_braced_word(ctx, raw, word_parts, ifs, quoted, output)?;
            } else {
                let val = value.unwrap();
                output.push_value(val.as_bytes(), quoted, ifs);
            }
        }
        BracedOp::Assign | BracedOp::AssignColon => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let use_word = match &value {
                None => true,
                Some(v) if op == BracedOp::AssignColon && v.is_empty() => true,
                _ => false,
            };
            if use_word {
                if !is_name(name) {
                    let mut msg = name.to_vec();
                    msg.extend_from_slice(b": cannot assign in this way");
                    return Err(ExpandError {
                        message: msg.into(),
                    });
                }
                let expanded = expand_braced_word_text(ctx, raw, word_parts)?;
                ctx.set_var(name, &expanded)?;
                output.push_value(&expanded, quoted, ifs);
            } else {
                let val = value.unwrap();
                output.push_value(val.as_bytes(), quoted, ifs);
            }
        }
        BracedOp::Error | BracedOp::ErrorColon => {
            let value = lookup_braced_param(ctx, raw, braced_name);
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
            output.push_value(val.as_bytes(), quoted, ifs);
        }
        BracedOp::Alt | BracedOp::AltColon => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let use_word = match &value {
                None => false,
                Some(v) if op == BracedOp::AltColon && v.is_empty() => false,
                _ => true,
            };
            if use_word {
                expand_braced_word(ctx, raw, word_parts, ifs, quoted, output)?;
            } else if quoted {
                output.push_quoted(b"");
            }
        }
        BracedOp::TrimSuffix | BracedOp::TrimSuffixLong => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?.into_owned();
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts)?;
            let trimmed = trim_suffix(&value, &pattern, op == BracedOp::TrimSuffixLong);
            output.push_value(trimmed, quoted, ifs);
        }
        BracedOp::TrimPrefix | BracedOp::TrimPrefixLong => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?.into_owned();
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts)?;
            let trimmed = trim_prefix(&value, &pattern, op == BracedOp::TrimPrefixLong);
            output.push_value(trimmed, quoted, ifs);
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
    Ok(out.drain_single_vec())
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
            WordPart::Literal { start, end, .. } => {
                let bytes = &raw[*start..*end];
                segments.push(Segment::Text(bytes.to_vec(), QuoteState::Literal));
            }
            WordPart::QuotedLiteral { bytes, .. } => {
                segments.push(Segment::Text(bytes.to_vec(), QuoteState::Quoted));
            }
            WordPart::Expansion { kind, quoted } => {
                let mut temp = ExpandOutput::new();
                expand_kind(ctx, raw, kind, b"", *quoted, &mut temp)?;
                let text = temp.drain_single_vec();
                let state = if *quoted {
                    QuoteState::Quoted
                } else {
                    QuoteState::Expanded
                };
                segments.push(Segment::Text(text, state));
            }
            WordPart::TildeLiteral { .. } => {}
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
            WordPart::Literal { start, end, .. } => {
                expr_text.extend_from_slice(&raw[*start..*end]);
            }
            WordPart::QuotedLiteral { bytes, .. } => {
                expr_text.extend_from_slice(bytes);
            }
            WordPart::Expansion { kind, .. } => {
                let mut temp = ExpandOutput::new();
                expand_kind(ctx, raw, kind, b"", true, &mut temp)?;
                let flat = temp.drain_single_vec();
                expr_text.extend_from_slice(&flat);
            }
            WordPart::TildeLiteral { .. } => {}
        }
    }
    let saved_line = ctx.lineno();
    let value = eval_arithmetic(ctx, &expr_text)?;
    ctx.set_lineno(saved_line);
    let buf = bstr::I64Buf::new(value);
    output.push_value(buf.as_bytes(), quoted, ifs);
    Ok(())
}

pub(super) fn char_boundary_offsets(value: &[u8]) -> Vec<usize> {
    let mut offsets = vec![0usize];
    let mut i = 0;
    while i < value.len() {
        let (_, len) = crate::sys::locale::decode_char(&value[i..]);
        let step = if len == 0 { 1 } else { len };
        i += step;
        offsets.push(i);
    }
    offsets
}

fn trim_suffix<'a>(value: &'a [u8], pattern: &[u8], longest: bool) -> &'a [u8] {
    let offsets = char_boundary_offsets(value);
    if longest {
        for &i in offsets.iter() {
            if pattern_matches(&value[i..], pattern) {
                return &value[..i];
            }
        }
    } else {
        for &i in offsets.iter().rev() {
            if pattern_matches(&value[i..], pattern) {
                return &value[..i];
            }
        }
    }
    value
}

fn trim_prefix<'a>(value: &'a [u8], pattern: &[u8], longest: bool) -> &'a [u8] {
    let offsets = char_boundary_offsets(value);
    if longest {
        for &i in offsets.iter().rev() {
            if pattern_matches(&value[..i], pattern) {
                return &value[i..];
            }
        }
    } else {
        for &i in offsets.iter() {
            if pattern_matches(&value[..i], pattern) {
                return &value[i..];
            }
        }
    }
    value
}

fn is_ifs_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\n'
}

pub(super) struct IfsChar {
    pub byte_seq: Vec<u8>,
    pub is_ws: bool,
}

pub(super) fn decompose_ifs(ifs: &[u8]) -> Vec<IfsChar> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < ifs.len() {
        let (_, len) = crate::sys::locale::decode_char(&ifs[i..]);
        let step = if len == 0 { 1 } else { len };
        let byte_seq = ifs[i..i + step].to_vec();
        let is_ws = step == 1 && is_ifs_whitespace(ifs[i]);
        result.push(IfsChar { byte_seq, is_ws });
        i += step;
    }
    result
}

pub(super) fn find_ifs_char_at<'a>(
    ifs_chars: &'a [IfsChar],
    bytes: &[u8],
) -> Option<(u32, &'a [u8], bool)> {
    for ic in ifs_chars {
        if bytes.len() >= ic.byte_seq.len() && bytes[..ic.byte_seq.len()] == *ic.byte_seq {
            let (wc, _) = crate::sys::locale::decode_char(&ic.byte_seq);
            return Some((wc, &ic.byte_seq, ic.is_ws));
        }
    }
    None
}

trait AsBytes {
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for Cow<'_, [u8]> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}
