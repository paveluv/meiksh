use std::borrow::Cow;

use crate::bstr;
use crate::syntax::word_part::{BracedName, BracedOp, ExpansionKind, WordPart};

use super::arithmetic::eval_arithmetic;
use super::core::{Context, ExpandError};
use super::glob::pattern_matches_with_offsets;
use super::model::{QuoteState, Segment, render_pattern_from_segments};
use super::parameter::{lookup_param, require_set_parameter};
use super::scratch::ExpandScratch;
use super::word::trim_trailing_newlines;
use crate::syntax::byte_class::{is_glob_char, is_name};

#[derive(Clone, Debug)]
pub(crate) struct FieldEntry {
    pub(crate) text: Vec<u8>,
    pub(crate) has_glob: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExpandOutput {
    pub(super) fields: Vec<FieldEntry>,
    pub(super) current: Vec<u8>,
    current_has_glob: bool,
    has_any_glob: bool,
    had_quoted_content: bool,
    has_at_expansion: bool,
    had_quoted_null_outside_at: bool,
    at_field_breaks: Vec<usize>,
    at_empty: bool,
}

impl ExpandOutput {
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

    fn push_current_field(&mut self) {
        let glob = self.current_has_glob;
        self.has_any_glob |= glob;
        self.fields.push(FieldEntry {
            text: std::mem::take(&mut self.current),
            has_glob: glob,
        });
        self.current_has_glob = false;
    }

    pub(super) fn push_expanded(&mut self, bytes: &[u8], ifs_chars: &[IfsChar]) {
        if ifs_chars.is_empty() {
            self.current.extend_from_slice(bytes);
            return;
        }

        let mut i = 0;
        while i < bytes.len() {
            if let Some((_, byte_seq, is_ws)) = find_ifs_char_at(ifs_chars, &bytes[i..]) {
                if is_ws {
                    if !self.current.is_empty() {
                        self.push_current_field();
                    }
                } else {
                    self.push_current_field();
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

    pub(super) fn push_value(&mut self, bytes: &[u8], quoted: bool, ifs_chars: &[IfsChar]) {
        if quoted {
            self.push_quoted(bytes);
        } else {
            self.push_expanded(bytes, ifs_chars);
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
                    self.push_current_field();
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
            self.fields.iter().map(|f| f.text.len()).sum::<usize>() + self.current.len();
        let mut result = Vec::with_capacity(total);
        for f in &self.fields {
            result.extend_from_slice(&f.text);
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
        self.has_any_glob = false;
        self.had_quoted_content = false;
        self.has_at_expansion = false;
        self.had_quoted_null_outside_at = false;
        self.at_field_breaks.clear();
        self.at_empty = false;
    }

    /// Drain expansion results directly into the caller-owned `argv`,
    /// inlining the pathname-expansion decision per field. Preserves
    /// `self.fields` / `self.current` capacity so the `ExpandOutput` can
    /// be reused for the next word without reallocating the wrapper Vecs.
    pub(super) fn finish_into<C: Context>(
        &mut self,
        ctx: &C,
        argv: &mut Vec<Vec<u8>>,
    ) -> Result<(), ExpandError> {
        self.finish_into_impl(argv, ctx.pathname_expansion_enabled())
    }

    /// Same as [`finish_into`] but with pathname expansion unconditionally
    /// disabled. Used for contexts where POSIX prohibits pathname
    /// expansion on the expanded bytes (e.g. redirection target words in
    /// a non-interactive shell).
    pub(super) fn finish_into_no_glob(
        &mut self,
        argv: &mut Vec<Vec<u8>>,
    ) -> Result<(), ExpandError> {
        self.finish_into_impl(argv, false)
    }

    fn finish_into_impl(
        &mut self,
        argv: &mut Vec<Vec<u8>>,
        pathname_expansion: bool,
    ) -> Result<(), ExpandError> {
        if self.has_at_expansion {
            return self.finish_at_expansion_into(argv);
        }

        if self.current.is_empty() && self.fields.is_empty() {
            if self.had_quoted_content {
                argv.push(Vec::new());
            }
            return Ok(());
        }

        if !self.current.is_empty() || self.fields.is_empty() {
            self.push_current_field();
        }

        argv.reserve(self.fields.len());
        if !self.has_any_glob || !pathname_expansion {
            for entry in self.fields.drain(..) {
                argv.push(entry.text);
            }
        } else {
            for entry in self.fields.drain(..) {
                if entry.has_glob {
                    let before = argv.len();
                    super::pathname::expand_pathname_into(&entry.text, argv);
                    if argv.len() == before {
                        argv.push(entry.text);
                    }
                } else {
                    argv.push(entry.text);
                }
            }
        }
        Ok(())
    }

    fn finish_at_expansion_into(&mut self, argv: &mut Vec<Vec<u8>>) -> Result<(), ExpandError> {
        if self.at_empty && self.at_field_breaks.is_empty() {
            if !self.current.is_empty() || self.had_quoted_null_outside_at {
                argv.push(self.drain_single_vec());
            }
            return Ok(());
        }

        if self.at_field_breaks.is_empty() {
            argv.push(self.drain_single_vec());
            return Ok(());
        }

        if !self.current.is_empty() {
            self.fields.push(FieldEntry {
                text: std::mem::take(&mut self.current),
                has_glob: false,
            });
            self.current_has_glob = false;
        }
        argv.reserve(self.fields.len());
        for entry in self.fields.drain(..) {
            argv.push(entry.text);
        }
        Ok(())
    }
}

pub(super) fn expand_parts_into<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    quoted: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    expand_parts_into_mode(ctx, raw, parts, quoted, quoted, output, scratch)
}

/// Like [`expand_parts_into`] but allows the caller to distinguish
/// "outer quoted" (propagated to each part's effective quote state) from
/// "single field mode" (the result will be collapsed to a single byte
/// vector by the caller). In single-field mode `$@` is joined with the
/// first character of `IFS` (space by default) instead of producing
/// separate argv fields; this matches POSIX semantics for `$@` inside
/// assignments, brace defaults, case patterns, and other single-value
/// contexts.
pub(super) fn expand_parts_into_mode<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    quoted: bool,
    single_field: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    for part in parts {
        match part {
            WordPart::Literal {
                start,
                end,
                has_glob,
                newlines,
                ..
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
                expand_kind(
                    ctx,
                    raw,
                    kind,
                    effective_quoted,
                    single_field,
                    output,
                    scratch,
                )?;
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
    quoted: bool,
    single_field: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    match kind {
        ExpansionKind::SimpleVar { start, end } => {
            let name = &raw[*start..*end];
            let value = lookup_param(ctx, name);
            let value = require_set_parameter(ctx, name, value)?;
            output.push_value(value.as_bytes(), quoted, &scratch.ifs_chars);
        }
        ExpansionKind::Positional { index } => {
            let idx = *index as usize;
            let value = ctx.positional_param(idx);
            let value = require_set_parameter(ctx, &[b'0' + index], value)?;
            output.push_value(value.as_bytes(), quoted, &scratch.ifs_chars);
        }
        ExpansionKind::ShellName => {
            let name = ctx.shell_name();
            output.push_value(name, quoted, &scratch.ifs_chars);
        }
        ExpansionKind::SpecialVar { ch } => {
            expand_special_var(ctx, *ch, quoted, single_field, output, scratch)?;
        }
        ExpansionKind::Braced { name, op, parts } => {
            expand_braced(
                ctx,
                raw,
                name,
                *op,
                parts,
                quoted,
                single_field,
                output,
                scratch,
            )?;
        }
        ExpansionKind::Command { program } => {
            let out = ctx.command_substitute(program)?;
            let trimmed = trim_trailing_newlines(&out);
            output.push_value(trimmed, quoted, &scratch.ifs_chars);
        }
        ExpansionKind::Arithmetic { parts } => {
            expand_arithmetic(ctx, raw, parts, quoted, output, scratch)?;
        }
        ExpansionKind::ArithmeticLiteral { start, end } => {
            let saved_line = ctx.lineno();
            let value = eval_arithmetic(ctx, &raw[*start..*end])?;
            ctx.set_lineno(saved_line);
            let buf = bstr::I64Buf::new(value);
            output.push_value(buf.as_bytes(), quoted, &scratch.ifs_chars);
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
    quoted: bool,
    single_field: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    match ch {
        b'@' => {
            if single_field {
                // Single-field contexts (assignment values, brace defaults,
                // case patterns, etc.) collapse `$@` to a single string by
                // joining its elements with IFS[0]. This matches bash:
                // `v=$@` is equivalent to `v=$*`.
                let ifs_cow = ctx.env_var(b"IFS");
                let sep: &[u8] = match &ifs_cow {
                    None => b" ",
                    Some(s) if s.is_empty() => b"",
                    Some(s) => &s[..crate::sys::locale::first_char_len(s)],
                };
                let value = bstr::join_bstrings(ctx.positional_params(), sep);
                output.push_value(&value, quoted, &scratch.ifs_chars);
            } else if quoted {
                output.push_at_fields(ctx.positional_params());
            } else {
                let joined = Cow::Owned(bstr::join_bstrings(ctx.positional_params(), b" "));
                let value = require_set_parameter(ctx, b"@", Some(joined))?;
                output.push_expanded(value.as_bytes(), &scratch.ifs_chars);
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
            output.push_value(&value, quoted, &scratch.ifs_chars);
        }
        _ => {
            let value = ctx.special_param(ch);
            let value = require_set_parameter(ctx, &[ch], value)?;
            output.push_value(value.as_bytes(), quoted, &scratch.ifs_chars);
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
    quoted: bool,
    single_field: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
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
            output.push_value(&len, quoted, &scratch.ifs_chars);
        }
        BracedOp::None => {
            if !word_parts.is_empty() {
                return Err(ExpandError {
                    message: b"bad substitution".as_ref().into(),
                });
            }
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?;
            output.push_value(value.as_bytes(), quoted, &scratch.ifs_chars);
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
                expand_braced_word(ctx, raw, word_parts, quoted, single_field, output, scratch)?;
            } else {
                let val = value.unwrap();
                output.push_value(val.as_bytes(), quoted, &scratch.ifs_chars);
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
                let expanded = expand_braced_word_text(ctx, raw, word_parts, scratch)?;
                ctx.set_var(name, &expanded)?;
                output.push_value(&expanded, quoted, &scratch.ifs_chars);
            } else {
                let val = value.unwrap();
                output.push_value(val.as_bytes(), quoted, &scratch.ifs_chars);
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
                    let expanded = expand_braced_word_text(ctx, raw, word_parts, scratch)?;
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
            output.push_value(val.as_bytes(), quoted, &scratch.ifs_chars);
        }
        BracedOp::Alt | BracedOp::AltColon => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let use_word = match &value {
                None => false,
                Some(v) if op == BracedOp::AltColon && v.is_empty() => false,
                _ => true,
            };
            if use_word {
                expand_braced_word(ctx, raw, word_parts, quoted, single_field, output, scratch)?;
            } else if quoted {
                output.push_quoted(b"");
            }
        }
        BracedOp::TrimSuffix | BracedOp::TrimSuffixLong => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?.into_owned();
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts, scratch)?;
            let mut offsets = std::mem::take(&mut scratch.char_offsets);
            let trimmed = trim_suffix(
                &value,
                &pattern,
                op == BracedOp::TrimSuffixLong,
                &mut offsets,
            );
            output.push_value(trimmed, quoted, &scratch.ifs_chars);
            scratch.char_offsets = offsets;
        }
        BracedOp::TrimPrefix | BracedOp::TrimPrefixLong => {
            let value = lookup_braced_param(ctx, raw, braced_name);
            let value = require_set_parameter(ctx, name, value)?.into_owned();
            let pattern = expand_braced_word_pattern(ctx, raw, word_parts, scratch)?;
            let mut offsets = std::mem::take(&mut scratch.char_offsets);
            let trimmed = trim_prefix(
                &value,
                &pattern,
                op == BracedOp::TrimPrefixLong,
                &mut offsets,
            );
            output.push_value(trimmed, quoted, &scratch.ifs_chars);
            scratch.char_offsets = offsets;
        }
    }
    Ok(())
}

fn expand_braced_word<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
    quoted: bool,
    single_field: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    expand_parts_into_mode(ctx, raw, word_parts, quoted, single_field, output, scratch)
}

fn expand_braced_word_text<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
    scratch: &mut ExpandScratch,
) -> Result<Vec<u8>, ExpandError> {
    // Borrow the pooled nested-output slot for the duration of this
    // call. Taken out so it doesn't alias `scratch` passed alongside;
    // restored before returning so future calls keep the allocated
    // capacity.
    let mut temp = std::mem::take(&mut scratch.output_nested);
    temp.clear();
    let result = expand_parts_into_mode(ctx, raw, word_parts, true, true, &mut temp, scratch);
    let text = match &result {
        Ok(()) => temp.drain_single_vec(),
        Err(_) => Vec::new(),
    };
    scratch.output_nested = temp;
    result.map(|()| text)
}

fn expand_braced_word_pattern<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    word_parts: &[WordPart],
    scratch: &mut ExpandScratch,
) -> Result<Vec<u8>, ExpandError> {
    // Borrow the pooled pattern-segment buffer for the duration of
    // this call. Cleared before use; restored afterwards.
    let mut segments = std::mem::take(&mut scratch.pattern_segments);
    segments.clear();
    let result = build_pattern_segments(ctx, raw, word_parts, &mut segments, scratch);
    let pattern = match &result {
        Ok(()) => render_pattern_from_segments(&segments),
        Err(_) => Vec::new(),
    };
    scratch.pattern_segments = segments;
    result.map(|()| pattern)
}

pub(super) fn build_pattern_segments<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    segments: &mut Vec<Segment>,
    scratch: &mut ExpandScratch,
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
                let mut temp = std::mem::take(&mut scratch.output_nested);
                temp.clear();
                // Pattern segments are always produced as a single flat
                // string per expansion, so route through `single_field=true`.
                let result = expand_kind(ctx, raw, kind, *quoted, true, &mut temp, scratch);
                let text = match &result {
                    Ok(()) => temp.drain_single_vec(),
                    Err(_) => Vec::new(),
                };
                scratch.output_nested = temp;
                result?;
                let state = if *quoted {
                    QuoteState::Quoted
                } else {
                    QuoteState::Expanded
                };
                segments.push(Segment::Text(text, state));
            }
            WordPart::TildeLiteral {
                tilde_pos,
                user_end,
                end,
            } => {
                let user = &raw[tilde_pos + 1..*user_end];
                let slash_follows = *user_end < *end && raw[*user_end] == b'/';
                expand_tilde_into_segments(ctx, user, slash_follows, segments);
                if *user_end < *end {
                    segments.push(Segment::Text(
                        raw[*user_end..*end].to_vec(),
                        QuoteState::Literal,
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Pattern-segment equivalent of `expand_tilde`. Mirrors the argv-side
/// tilde logic exactly but appends to a `Segment` list instead of an
/// `ExpandOutput`: an expanded home directory is added as `Quoted`
/// (so its bytes are matched literally by the pattern matcher), and the
/// unresolved fallback (`~user` with no such user) is added as
/// `Literal`. Any `~` that collapsed to an empty home directory pushes
/// an empty quoted segment so the surrounding pattern structure stays
/// intact.
fn expand_tilde_into_segments<C: Context>(
    ctx: &mut C,
    user: &[u8],
    slash_follows: bool,
    segments: &mut Vec<Segment>,
) {
    if user.is_empty() {
        match ctx.env_var(b"HOME") {
            Some(home) if !home.is_empty() => {
                let h = if slash_follows && home.ends_with(b"/") {
                    &home[..home.len() - 1]
                } else {
                    &home
                };
                segments.push(Segment::Text(h.to_vec(), QuoteState::Quoted));
            }
            Some(_) => {
                segments.push(Segment::Text(Vec::new(), QuoteState::Quoted));
            }
            None => {
                segments.push(Segment::Text(b"~".to_vec(), QuoteState::Literal));
            }
        }
    } else if let Some(dir) = ctx.home_dir_for_user(user) {
        let d = if slash_follows && dir.ends_with(b"/") {
            &dir[..dir.len() - 1]
        } else {
            &dir
        };
        segments.push(Segment::Text(d.to_vec(), QuoteState::Quoted));
    } else {
        segments.push(Segment::Text(b"~".to_vec(), QuoteState::Literal));
        segments.push(Segment::Text(user.to_vec(), QuoteState::Literal));
    }
}

fn expand_arithmetic<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    quoted: bool,
    output: &mut ExpandOutput,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    let mut expr_text = std::mem::take(&mut scratch.arith_expr);
    expr_text.clear();
    let result = build_arithmetic_expr_text(ctx, raw, parts, &mut expr_text, scratch);
    let eval_result = result.and_then(|()| {
        let saved_line = ctx.lineno();
        let value = eval_arithmetic(ctx, &expr_text)?;
        ctx.set_lineno(saved_line);
        Ok(value)
    });
    scratch.arith_expr = expr_text;
    let value = eval_result?;
    let buf = bstr::I64Buf::new(value);
    output.push_value(buf.as_bytes(), quoted, &scratch.ifs_chars);
    Ok(())
}

fn build_arithmetic_expr_text<C: Context>(
    ctx: &mut C,
    raw: &[u8],
    parts: &[WordPart],
    expr_text: &mut Vec<u8>,
    scratch: &mut ExpandScratch,
) -> Result<(), ExpandError> {
    for part in parts {
        match part {
            WordPart::Literal { start, end, .. } => {
                expr_text.extend_from_slice(&raw[*start..*end]);
            }
            WordPart::QuotedLiteral { bytes, .. } => {
                expr_text.extend_from_slice(bytes);
            }
            WordPart::Expansion { kind, .. } => {
                let mut temp = std::mem::take(&mut scratch.output_nested);
                temp.clear();
                // Arithmetic expression bytes are a single string, so
                // route through `single_field=true`.
                let result = expand_kind(ctx, raw, kind, true, true, &mut temp, scratch);
                if result.is_ok() {
                    // Append the flat expansion directly into `expr_text`
                    // without materializing an intermediate `Vec<u8>`.
                    append_drained_single(&mut temp, expr_text);
                }
                scratch.output_nested = temp;
                result?;
            }
            WordPart::TildeLiteral { .. } => {
                // Parser invariant: `build_word_parts_impl` is called with
                // `allow_tilde=false` for the body of `$((...))`, so the
                // arithmetic parts slice never contains a `TildeLiteral`.
                // Keep the arm to satisfy exhaustiveness, but surface an
                // invariant break loudly rather than silently dropping
                // bytes from the arithmetic expression.
                unreachable!("parser invariant: arithmetic body never contains TildeLiteral");
            }
        }
    }
    Ok(())
}

/// Append the single-field flat form of `temp` into `out`, then reset
/// `temp` in place. Avoids the per-call `Vec<u8>` materialization that
/// `ExpandOutput::drain_single_vec` would otherwise produce.
fn append_drained_single(temp: &mut ExpandOutput, out: &mut Vec<u8>) {
    if temp.fields.is_empty() {
        out.extend_from_slice(&temp.current);
        temp.current.clear();
    } else {
        for f in &temp.fields {
            out.extend_from_slice(&f.text);
        }
        out.extend_from_slice(&temp.current);
        temp.fields.clear();
        temp.current.clear();
    }
}

/// Build the list of character-boundary byte offsets within `value`.
///
/// Always includes `0` and `value.len()`. For ASCII bytes we skip the
/// `sys::locale::decode_char` dispatch entirely (one per byte would
/// otherwise dominate parameter-expansion pattern stripping on ASCII
/// inputs). We preallocate with the maximum possible capacity — one
/// boundary per byte, plus the starting `0` — which is exact for ASCII
/// and a slight overshoot for multibyte input.
pub(super) fn char_boundary_offsets(value: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(value.len() + 1);
    char_boundary_offsets_into(value, &mut offsets);
    offsets
}

/// Append per-character boundary byte offsets of `value` into `out`. Caller
/// clears `out` first if a fresh buffer is desired. Always pushes the
/// starting `0` and a final `value.len()`. See [`char_boundary_offsets`]
/// for details on the ASCII fast path and preallocation behavior.
pub(super) fn char_boundary_offsets_into(value: &[u8], out: &mut Vec<usize>) {
    out.reserve(value.len() + 1);
    out.push(0);
    let mut i = 0;
    while i < value.len() {
        let step = if value[i] < 0x80 {
            1
        } else {
            let (_, len) = crate::sys::locale::decode_char(&value[i..]);
            if len == 0 { 1 } else { len }
        };
        i += step;
        out.push(i);
    }
}

fn trim_suffix<'a>(
    value: &'a [u8],
    pattern: &[u8],
    longest: bool,
    offsets: &mut Vec<usize>,
) -> &'a [u8] {
    offsets.clear();
    char_boundary_offsets_into(value, offsets);
    let try_match =
        |k: usize, i: usize| pattern_matches_with_offsets(&value[i..], &offsets[k..], i, pattern);
    if longest {
        for (k, &i) in offsets.iter().enumerate() {
            if try_match(k, i) {
                return &value[..i];
            }
        }
    } else {
        for (k, &i) in offsets.iter().enumerate().rev() {
            if try_match(k, i) {
                return &value[..i];
            }
        }
    }
    value
}

fn trim_prefix<'a>(
    value: &'a [u8],
    pattern: &[u8],
    longest: bool,
    offsets: &mut Vec<usize>,
) -> &'a [u8] {
    offsets.clear();
    char_boundary_offsets_into(value, offsets);
    let try_match = |k: usize| {
        let end = offsets[k];
        pattern_matches_with_offsets(&value[..end], &offsets[..=k], 0, pattern)
    };
    if longest {
        for k in (0..offsets.len()).rev() {
            if try_match(k) {
                return &value[offsets[k]..];
            }
        }
    } else {
        for k in 0..offsets.len() {
            if try_match(k) {
                return &value[offsets[k]..];
            }
        }
    }
    value
}

fn is_ifs_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\n'
}

#[derive(Clone, Debug)]
pub(crate) struct IfsChar {
    pub byte_seq: Vec<u8>,
    pub is_ws: bool,
}

/// Append the decomposed `IfsChar` entries for `ifs` into `out`.
/// Caller clears `out` first if a fresh buffer is desired.
pub(super) fn decompose_ifs_into(ifs: &[u8], out: &mut Vec<IfsChar>) {
    let mut i = 0;
    while i < ifs.len() {
        let step = if ifs[i] < 0x80 {
            1
        } else {
            let (_, len) = crate::sys::locale::decode_char(&ifs[i..]);
            if len == 0 { 1 } else { len }
        };
        let byte_seq = ifs[i..i + step].to_vec();
        let is_ws = step == 1 && is_ifs_whitespace(ifs[i]);
        out.push(IfsChar { byte_seq, is_ws });
        i += step;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expand::test_support::FakeContext;
    use crate::sys::test_support::{assert_no_syscalls, set_test_locale_c, set_test_locale_utf8};

    #[test]
    fn decompose_ifs_c_vs_utf8() {
        assert_no_syscalls(|| {
            // U+00E9 = 0xC3 0xA9
            set_test_locale_c();
            let mut ifs = Vec::new();
            decompose_ifs_into(b"\xc3\xa9", &mut ifs);
            assert_eq!(ifs.len(), 2);
            assert_eq!(ifs[0].byte_seq, vec![0xc3]);
            assert_eq!(ifs[1].byte_seq, vec![0xa9]);

            set_test_locale_utf8();
            let mut ifs = Vec::new();
            decompose_ifs_into(b"\xc3\xa9", &mut ifs);
            assert_eq!(ifs.len(), 1);
            assert_eq!(ifs[0].byte_seq, vec![0xc3, 0xa9]);
        });
    }

    #[test]
    fn expand_tilde_known_user_with_trailing_slash_in_home_is_trimmed() {
        // `FakeContext::home_dir_for_user(b"slashuser")` intentionally
        // returns "/home/slashuser/" with a trailing slash.  When the
        // source word also has a `/` immediately after the user name
        // (`slash_follows=true`), `expand_tilde` must drop the trailing
        // slash on the home so that the final joined path isn't
        // "/home/slashuser//rest".  This test exercises the otherwise-
        // uncovered slash-trim branch in the WordPart-driven path, which
        // is distinct from the legacy `expand_raw` tilde code.
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let raw = b"~slashuser/rest";
            // `build_word_parts_impl` emits a single TildeLiteral that
            // covers the whole word up to the next word break (or end):
            // `tilde_pos = 0`, `user_end = 10` (end of "slashuser"),
            // `end = 15` (covers the `/rest` tail, which drives
            // `slash_follows=true` inside `expand_tilde`).  There is no
            // trailing `Literal` part for the slashed tail.
            let parts = [WordPart::TildeLiteral {
                tilde_pos: 0,
                user_end: 10,
                end: 15,
            }];
            let mut output = ExpandOutput::default();
            let mut scratch = ExpandScratch::default();
            expand_parts_into(&mut ctx, raw, &parts, false, &mut output, &mut scratch)
                .expect("expand");
            let mut argv: Vec<Vec<u8>> = Vec::new();
            output.finish_into_no_glob(&mut argv).expect("finish");
            assert_eq!(argv, vec![b"/home/slashuser/rest".to_vec()]);
        });
    }

    #[test]
    fn expand_tilde_unknown_user_writes_literal_bytes_with_glob_detection() {
        // When the user lookup fails, `expand_tilde` falls back to writing
        // the literal `~` followed by the user bytes through the per-byte
        // `push_literal` path.  If the user portion contains an active
        // glob metacharacter (`*` is not a tilde-user break character), the
        // per-byte `is_glob_char` check must flip `has_any_glob` so the
        // resulting field takes the pathname-expansion branch on finish.
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            let raw = b"~no*such*user";
            let parts = [WordPart::TildeLiteral {
                tilde_pos: 0,
                user_end: raw.len(),
                end: raw.len(),
            }];
            let mut output = ExpandOutput::default();
            let mut scratch = ExpandScratch::default();
            expand_parts_into(&mut ctx, raw, &parts, false, &mut output, &mut scratch)
                .expect("expand");
            // The field buffer must have absorbed the literal bytes…
            assert_eq!(output.current.as_slice(), b"~no*such*user");
            // …and `push_literal`'s per-byte scan must have marked the
            // field as glob-bearing because of the `*` characters.
            assert!(
                output.current_has_glob,
                "push_literal should observe `*` and flip current_has_glob",
            );
        });
    }
}
