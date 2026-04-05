use std::fmt;
use std::path::{Path, PathBuf};

use crate::syntax::Word;
use crate::sys;

#[derive(Debug)]
pub struct ExpandError {
    pub message: String,
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExpandError {}

pub trait Context {
    fn env_var(&self, name: &str) -> Option<String>;
    fn special_param(&self, name: char) -> Option<String>;
    fn positional_param(&self, index: usize) -> Option<String>;
    fn positional_params(&self) -> Vec<String>;
    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &str;
    fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError>;
    fn home_dir_for_user(&self, name: &str) -> Option<String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String, bool),
    AtBreak,
    AtEmpty,
}

enum Expansion {
    One(String),
    AtFields(Vec<String>),
}

pub fn expand_words<C: Context>(ctx: &mut C, words: &[Word]) -> Result<Vec<String>, ExpandError> {
    let mut result = Vec::new();
    for word in words {
        result.extend(expand_word(ctx, word)?);
    }
    Ok(result)
}

pub fn expand_word<C: Context>(ctx: &mut C, word: &Word) -> Result<Vec<String>, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;

    if expanded.has_at_expansion {
        return expand_word_with_at_fields(&expanded);
    }

    if expanded.segments.is_empty() {
        if expanded.had_quoted_content {
            return Ok(vec![String::new()]);
        }
        return Ok(Vec::new());
    }

    let all_quoted = !expanded
        .segments
        .iter()
        .any(|seg| matches!(seg, Segment::Text(_, false)));
    if all_quoted {
        return Ok(vec![flatten_segments(&expanded.segments)]);
    }

    let fields = split_fields_from_segments(
        &expanded.segments,
        &ctx.env_var("IFS").unwrap_or_else(|| " \t\n".to_string()),
    );
    if fields.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for field in fields {
        if field.has_unquoted_glob && ctx.pathname_expansion_enabled() {
            let matches = expand_pathname(&field.text);
            if matches.is_empty() {
                result.push(field.text);
            } else {
                result.extend(matches);
            }
        } else {
            result.push(field.text);
        }
    }
    Ok(result)
}

fn expand_word_with_at_fields(expanded: &ExpandedWord) -> Result<Vec<String>, ExpandError> {
    let has_at_empty = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtEmpty));
    let has_at_break = expanded
        .segments
        .iter()
        .any(|s| matches!(s, Segment::AtBreak));

    if has_at_empty && !has_at_break {
        return Ok(Vec::new());
    }

    if !has_at_break {
        return Ok(vec![flatten_segments(&expanded.segments)]);
    }

    let mut fields = Vec::new();
    let mut current = String::new();

    for seg in &expanded.segments {
        if let Segment::Text(text, _) = seg {
            current.push_str(text);
        } else if matches!(seg, Segment::AtBreak) {
            fields.push(std::mem::take(&mut current));
        }
    }
    fields.push(current);

    Ok(fields)
}

pub fn expand_word_text<C: Context>(ctx: &mut C, word: &Word) -> Result<String, ExpandError> {
    expand_word_text_assignment(ctx, word, false)
}

pub fn expand_word_pattern<C: Context>(ctx: &mut C, word: &Word) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
}

pub fn expand_assignment_value<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<String, ExpandError> {
    expand_word_text_assignment(ctx, word, true)
}

fn expand_word_text_assignment<C: Context>(
    ctx: &mut C,
    word: &Word,
    assignment_rhs: bool,
) -> Result<String, ExpandError> {
    if !assignment_rhs {
        let expanded = expand_raw(ctx, &word.raw)?;
        return Ok(flatten_segments(&expanded.segments));
    }
    let raw = &word.raw;
    let mut result = String::new();
    let mut first = true;
    for part in split_on_unquoted_colons(raw) {
        if !first {
            result.push(':');
        }
        first = false;
        let expanded = expand_raw(ctx, &part)?;
        result.push_str(&flatten_segments(&expanded.segments));
    }
    Ok(result)
}

fn split_on_unquoted_colons(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    while i < chars.len() {
        match chars[i] {
            '\'' if brace_depth == 0 && paren_depth == 0 => {
                current.push(chars[i]);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    current.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '"' if brace_depth == 0 && paren_depth == 0 => {
                current.push(chars[i]);
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        current.push(chars[i]);
                        i += 1;
                    }
                    current.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                current.push(chars[i]);
                i += 1;
                if i < chars.len() {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '$' if i + 1 < chars.len() && chars[i + 1] == '{' => {
                current.push(chars[i]);
                current.push(chars[i + 1]);
                brace_depth += 1;
                i += 2;
            }
            '}' if brace_depth > 0 => {
                brace_depth -= 1;
                current.push(chars[i]);
                i += 1;
            }
            '$' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                current.push(chars[i]);
                current.push(chars[i + 1]);
                paren_depth += 1;
                i += 2;
            }
            ')' if paren_depth > 0 => {
                paren_depth -= 1;
                current.push(chars[i]);
                i += 1;
            }
            ':' if brace_depth == 0 && paren_depth == 0 => {
                parts.push(std::mem::take(&mut current));
                i += 1;
            }
            ch => {
                current.push(ch);
                i += 1;
            }
        }
    }
    parts.push(current);
    parts
}

pub fn expand_parameter_text<C: Context>(ctx: &mut C, raw: &str) -> Result<String, ExpandError> {
    let chars: Vec<char> = raw.chars().collect();
    let mut result = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '$' {
            let (value, consumed) = expand_parameter_dollar(ctx, &chars[index..])?;
            result.push_str(&value);
            index += consumed;
        } else {
            result.push(chars[index]);
            index += 1;
        }
    }

    Ok(result)
}

fn flatten_expansion(expansion: Expansion) -> String {
    match expansion {
        Expansion::One(s) => s,
        Expansion::AtFields(fields) => fields.join(" "),
    }
}

fn apply_expansion(
    segments: &mut Vec<Segment>,
    expansion: Expansion,
    quoted: bool,
    has_at: &mut bool,
) {
    match expansion {
        Expansion::One(s) => push_segment(segments, s, quoted),
        Expansion::AtFields(params) => {
            *has_at = true;
            if params.is_empty() {
                segments.push(Segment::AtEmpty);
            } else {
                for (i, param) in params.into_iter().enumerate() {
                    if i > 0 {
                        segments.push(Segment::AtBreak);
                    }
                    push_segment(segments, param, true);
                }
            }
        }
    }
}

fn expand_raw<C: Context>(ctx: &mut C, raw: &str) -> Result<ExpandedWord, ExpandError> {
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0usize;
    let mut segments = Vec::new();
    let mut had_quoted_content = false;
    let mut has_at_expansion = false;

    while index < chars.len() {
        match chars[index] {
            '\'' => {
                had_quoted_content = true;
                index += 1;
                let start = index;
                while index < chars.len() && chars[index] != '\'' {
                    index += 1;
                }
                if index >= chars.len() {
                    return Err(ExpandError {
                        message: "unterminated single quote".to_string(),
                    });
                }
                push_segment(&mut segments, chars[start..index].iter().collect(), true);
                index += 1;
            }
            '"' => {
                had_quoted_content = true;
                index += 1;
                let mut buffer = String::new();
                while index < chars.len() && chars[index] != '"' {
                    match chars[index] {
                        '\\' => {
                            if index + 1 < chars.len() {
                                let next = chars[index + 1];
                                if matches!(next, '$' | '`' | '"' | '\\' | '\n' | '}') {
                                    index += 1;
                                    if next != '\n' {
                                        buffer.push(next);
                                    }
                                    index += 1;
                                } else {
                                    buffer.push('\\');
                                    index += 1;
                                }
                            } else {
                                buffer.push('\\');
                                index += 1;
                            }
                        }
                        '$' => {
                            if !buffer.is_empty() {
                                push_segment(&mut segments, std::mem::take(&mut buffer), true);
                            }
                            let (expansion, consumed) = expand_dollar(ctx, &chars[index..], true)?;
                            apply_expansion(&mut segments, expansion, true, &mut has_at_expansion);
                            index += consumed;
                        }
                        '`' => {
                            if !buffer.is_empty() {
                                push_segment(&mut segments, std::mem::take(&mut buffer), true);
                            }
                            index += 1;
                            let command = scan_backtick_command(&chars, &mut index, true)?;
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = output.trim_end_matches('\n').to_string();
                            push_segment(&mut segments, trimmed, true);
                        }
                        ch => {
                            buffer.push(ch);
                            index += 1;
                        }
                    }
                }
                if index >= chars.len() {
                    return Err(ExpandError {
                        message: "unterminated double quote".to_string(),
                    });
                }
                if !buffer.is_empty() {
                    push_segment(&mut segments, buffer, true);
                }
                index += 1;
            }
            '\\' => {
                index += 1;
                if index < chars.len() {
                    push_segment(&mut segments, chars[index].to_string(), true);
                    index += 1;
                }
            }
            '$' => {
                let dollar_single_quotes = chars.get(index + 1) == Some(&'\'');
                if dollar_single_quotes {
                    had_quoted_content = true;
                }
                let (expansion, consumed) = expand_dollar(ctx, &chars[index..], false)?;
                apply_expansion(
                    &mut segments,
                    expansion,
                    dollar_single_quotes,
                    &mut has_at_expansion,
                );
                index += consumed;
            }
            '`' => {
                index += 1;
                let command = scan_backtick_command(&chars, &mut index, false)?;
                let output = ctx.command_substitute(&command)?;
                let trimmed = output.trim_end_matches('\n').to_string();
                push_segment(&mut segments, trimmed, false);
            }
            '~' if index == 0 => {
                index += 1;
                let mut user = String::new();
                while index < chars.len() && chars[index] != '/' {
                    if chars[index] == '\''
                        || chars[index] == '"'
                        || chars[index] == '\\'
                        || chars[index] == '$'
                        || chars[index] == '`'
                    {
                        break;
                    }
                    user.push(chars[index]);
                    index += 1;
                }
                if user.is_empty() {
                    let home = ctx.env_var("HOME").unwrap_or_else(|| "~".to_string());
                    push_segment(&mut segments, home, true);
                } else if let Some(dir) = ctx.home_dir_for_user(&user) {
                    push_segment(&mut segments, dir, true);
                } else {
                    let mut literal = String::from('~');
                    literal.push_str(&user);
                    push_segment(&mut segments, literal, false);
                }
            }
            ch => {
                push_segment(&mut segments, ch.to_string(), false);
                index += 1;
            }
        }
    }

    Ok(ExpandedWord {
        segments,
        had_quoted_content,
        has_at_expansion,
    })
}

fn scan_backtick_command(
    chars: &[char],
    index: &mut usize,
    in_double_quotes: bool,
) -> Result<String, ExpandError> {
    let mut command = String::new();
    while *index < chars.len() {
        let ch = chars[*index];
        if ch == '`' {
            *index += 1;
            return Ok(command);
        }
        if ch == '\\' && *index + 1 < chars.len() {
            let next = chars[*index + 1];
            let special = if in_double_quotes {
                matches!(next, '$' | '`' | '\\' | '"' | '\n')
            } else {
                matches!(next, '$' | '`' | '\\')
            };
            if special {
                command.push(next);
                *index += 2;
                continue;
            }
        }
        command.push(ch);
        *index += 1;
    }
    Err(ExpandError {
        message: "unterminated backquote".to_string(),
    })
}

pub fn expand_here_document<C: Context>(ctx: &mut C, text: &str) -> Result<String, ExpandError> {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        match chars[index] {
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    result.push('\\');
                    break;
                }
                match chars[index] {
                    '$' | '\\' => {
                        result.push(chars[index]);
                        index += 1;
                    }
                    '\n' => {
                        index += 1;
                    }
                    ch => {
                        result.push('\\');
                        result.push(ch);
                        index += 1;
                    }
                }
            }
            '$' => {
                let (expansion, consumed) = expand_dollar(ctx, &chars[index..], false)?;
                result.push_str(&flatten_expansion(expansion));
                index += consumed;
            }
            ch => {
                result.push(ch);
                index += 1;
            }
        }
    }

    Ok(result)
}

fn expand_dollar<C: Context>(
    ctx: &mut C,
    chars: &[char],
    quoted: bool,
) -> Result<(Expansion, usize), ExpandError> {
    if chars.len() < 2 {
        return Ok((Expansion::One("$".to_string()), 1));
    }

    match chars[1] {
        '\'' if !quoted => {
            let (s, n) = parse_dollar_single_quoted(chars)?;
            Ok((Expansion::One(s), n))
        }
        '{' => {
            let end = scan_to_closing_brace(chars, 2)?;
            let expr: String = chars[2..end].iter().collect();
            let value = expand_braced_parameter(ctx, &expr, quoted)?;
            Ok((Expansion::One(value), end + 1))
        }
        '(' => {
            if chars.get(2) == Some(&'(') {
                let mut index = 3usize;
                let mut depth = 1usize;
                let mut expression = String::new();
                while index < chars.len() {
                    let ch = chars[index];
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        if depth == 1 && chars.get(index + 1) == Some(&')') {
                            let pre_expanded = expand_arithmetic_expression(ctx, &expression)?;
                            let value = eval_arithmetic(ctx, &pre_expanded)?;
                            return Ok((Expansion::One(value.to_string()), index + 2));
                        }
                        depth = depth.saturating_sub(1);
                    }
                    expression.push(ch);
                    index += 1;
                }
                Err(ExpandError {
                    message: "unterminated arithmetic expansion".to_string(),
                })
            } else {
                let mut index = 2usize;
                let mut depth = 1usize;
                let mut command = String::new();
                while index < chars.len() {
                    let ch = chars[index];
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth == 0 {
                            let output = ctx.command_substitute(&command)?;
                            let trimmed = output.trim_end_matches('\n').to_string();
                            return Ok((Expansion::One(trimmed), index + 1));
                        }
                    }
                    command.push(ch);
                    index += 1;
                }
                Err(ExpandError {
                    message: "unterminated command substitution".to_string(),
                })
            }
        }
        '@' => {
            if quoted {
                let params = ctx.positional_params();
                Ok((Expansion::AtFields(params), 2))
            } else {
                let value =
                    require_set_parameter(ctx, "@", Some(ctx.positional_params().join(" ")))?;
                Ok((Expansion::One(value), 2))
            }
        }
        '*' => {
            let ifs = ctx.env_var("IFS");
            let sep = match ifs.as_deref() {
                None => " ".to_string(),
                Some("") => String::new(),
                Some(s) => s.chars().next().unwrap().to_string(),
            };
            let value = ctx.positional_params().join(&sep);
            Ok((Expansion::One(value), 2))
        }
        '?' | '$' | '!' | '#' | '-' | '0' => {
            let ch = chars[1];
            let value = if ch == '0' {
                require_set_parameter(ctx, "0", Some(ctx.shell_name().to_string()))?
            } else {
                require_set_parameter(ctx, &ch.to_string(), ctx.special_param(ch))?
            };
            Ok((Expansion::One(value), 2))
        }
        next if next.is_ascii_digit() => Ok((
            Expansion::One(require_set_parameter(
                ctx,
                &next.to_string(),
                ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize),
            )?),
            2,
        )),
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            let mut name = String::new();
            while index < chars.len()
                && (chars[index] == '_' || chars[index].is_ascii_alphanumeric())
            {
                name.push(chars[index]);
                index += 1;
            }
            Ok((
                Expansion::One(require_set_parameter(ctx, &name, lookup_param(ctx, &name))?),
                index,
            ))
        }
        _ => Ok((Expansion::One("$".to_string()), 1)),
    }
}

fn expand_parameter_dollar<C: Context>(
    ctx: &mut C,
    chars: &[char],
) -> Result<(String, usize), ExpandError> {
    if chars.len() < 2 {
        return Ok(("$".to_string(), 1));
    }

    match chars[1] {
        '\'' => parse_dollar_single_quoted(chars),
        '{' => {
            let end = scan_to_closing_brace(chars, 2)?;
            let expr: String = chars[2..end].iter().collect();
            let value = expand_braced_parameter_text(ctx, &expr)?;
            Ok((value, end + 1))
        }
        '?' | '$' | '!' | '#' | '*' | '@' | '-' | '0' => {
            let ch = chars[1];
            let value = if ch == '0' {
                require_set_parameter(ctx, "0", Some(ctx.shell_name().to_string()))?
            } else {
                require_set_parameter(ctx, &ch.to_string(), ctx.special_param(ch))?
            };
            Ok((value, 2))
        }
        next if next.is_ascii_digit() => {
            let name = next.to_string();
            let value = ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize);
            Ok((require_set_parameter(ctx, &name, value)?, 2))
        }
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            let mut name = String::new();
            while index < chars.len()
                && (chars[index] == '_' || chars[index].is_ascii_alphanumeric())
            {
                name.push(chars[index]);
                index += 1;
            }
            Ok((
                require_set_parameter(ctx, &name, lookup_param(ctx, &name))?,
                index,
            ))
        }
        _ => Ok(("$".to_string(), 1)),
    }
}

fn parse_dollar_single_quoted(chars: &[char]) -> Result<(String, usize), ExpandError> {
    let mut index = 2usize;
    let mut result = String::new();
    while index < chars.len() {
        match chars[index] {
            '\'' => return Ok((result, index + 1)),
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    return Err(ExpandError {
                        message: "unterminated dollar-single-quotes".to_string(),
                    });
                }
                let ch = chars[index];
                match ch {
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    '\\' => result.push('\\'),
                    'a' => result.push('\u{0007}'),
                    'b' => result.push('\u{0008}'),
                    'e' => result.push('\u{001b}'),
                    'f' => result.push('\u{000c}'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'v' => result.push('\u{000b}'),
                    'c' => {
                        index += 1;
                        if index >= chars.len() {
                            return Err(ExpandError {
                                message: "unterminated dollar-single-quotes".to_string(),
                            });
                        }
                        if chars[index] == '\\' && index + 1 < chars.len() {
                            index += 1;
                            result.push(control_escape(chars[index]));
                        } else {
                            result.push(control_escape(chars[index]));
                        }
                    }
                    'x' => {
                        let (value, consumed) =
                            parse_variable_base_escape(&chars[(index + 1)..], 16, 2);
                        if consumed == 0 {
                            result.push('x');
                        } else {
                            result.push(char::from(value));
                            index += consumed;
                        }
                    }
                    '0'..='7' => {
                        let mut digits = String::from(ch);
                        let mut consumed = 0usize;
                        while consumed < 2
                            && index + 1 + consumed < chars.len()
                            && matches!(chars[index + 1 + consumed], '0'..='7')
                        {
                            digits.push(chars[index + 1 + consumed]);
                            consumed += 1;
                        }
                        let value = u8::from_str_radix(&digits, 8).unwrap_or_default();
                        result.push(char::from(value));
                        index += consumed;
                    }
                    other => result.push(other),
                }
                index += 1;
            }
            ch => {
                result.push(ch);
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: "unterminated dollar-single-quotes".to_string(),
    })
}

fn scan_to_closing_brace(chars: &[char], start: usize) -> Result<usize, ExpandError> {
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '}' => return Ok(index),
            '\\' => {
                index += 2;
            }
            '\'' => {
                index += 1;
                while index < chars.len() && chars[index] != '\'' {
                    index += 1;
                }
                if index < chars.len() {
                    index += 1;
                }
            }
            '"' => {
                index += 1;
                while index < chars.len() && chars[index] != '"' {
                    if chars[index] == '\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < chars.len() {
                    index += 1;
                }
            }
            '$' if matches!(chars.get(index + 1), Some(&'{')) => {
                index += 2;
                let inner = scan_to_closing_brace(chars, index)?;
                index = inner + 1;
            }
            '$' if matches!(chars.get(index + 1), Some(&'(')) => {
                if chars.get(index + 2) == Some(&'(') {
                    index += 3;
                    let mut depth = 1usize;
                    while index < chars.len() {
                        if chars[index] == '(' {
                            depth += 1;
                        } else if chars[index] == ')' {
                            if depth == 1 && chars.get(index + 1) == Some(&')') {
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
                    while index < chars.len() {
                        if chars[index] == '(' {
                            depth += 1;
                        } else if chars[index] == ')' {
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
            '`' => {
                index += 1;
                while index < chars.len() && chars[index] != '`' {
                    if chars[index] == '\\' {
                        index += 1;
                    }
                    index += 1;
                }
                if index < chars.len() {
                    index += 1;
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Err(ExpandError {
        message: "unterminated parameter expansion".to_string(),
    })
}

fn control_escape(ch: char) -> char {
    match ch {
        '\\' => '\u{001c}',
        '?' => '\u{007f}',
        other => char::from((other as u8) & 0x1f),
    }
}

fn parse_variable_base_escape(chars: &[char], base: u32, max_digits: usize) -> (u8, usize) {
    let mut digits = String::new();
    let mut consumed = 0usize;
    while consumed < max_digits && consumed < chars.len() && chars[consumed].is_digit(base) {
        digits.push(chars[consumed]);
        consumed += 1;
    }
    if digits.is_empty() {
        return (0, 0);
    }
    (
        u8::from_str_radix(&digits, base).unwrap_or_default(),
        consumed,
    )
}

fn expand_braced_parameter<C: Context>(
    ctx: &mut C,
    expr: &str,
    quoted: bool,
) -> Result<String, ExpandError> {
    if expr == "#" {
        return Ok(lookup_param(ctx, "#").unwrap_or_default());
    }
    if let Some(name) = expr.strip_prefix('#') {
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(value.chars().count().to_string());
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, &name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    match op.as_deref() {
        None => require_set_parameter(ctx, &name, value),
        Some(":-") => {
            if !is_set || is_null {
                expand_parameter_word(ctx, &word.unwrap_or_default(), quoted)
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("-") => {
            if !is_set {
                expand_parameter_word(ctx, &word.unwrap_or_default(), quoted)
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":=") => {
            if !is_set || is_null {
                assign_parameter(ctx, &name, &word.unwrap_or_default(), quoted)
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("=") => {
            if !is_set {
                assign_parameter(ctx, &name, &word.unwrap_or_default(), quoted)
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":?") => {
            if !is_set || is_null {
                let message = expand_parameter_word(
                    ctx,
                    &word.unwrap_or_else(|| format!("{name}: parameter null or not set")),
                    quoted,
                )?;
                Err(ExpandError { message })
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("?") => {
            if !is_set {
                let message = expand_parameter_word(
                    ctx,
                    &word.unwrap_or_else(|| format!("{name}: parameter not set")),
                    quoted,
                )?;
                Err(ExpandError { message })
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":+") => {
            if is_set && !is_null {
                expand_parameter_word(ctx, &word.unwrap_or_default(), quoted)
            } else {
                Ok(String::new())
            }
        }
        Some("+") => {
            if is_set {
                expand_parameter_word(ctx, &word.unwrap_or_default(), quoted)
            } else {
                Ok(String::new())
            }
        }
        Some("%") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_pattern_word(ctx, &word.unwrap_or_default())?,
            PatternRemoval::SmallestSuffix,
        ),
        Some("%%") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_pattern_word(ctx, &word.unwrap_or_default())?,
            PatternRemoval::LargestSuffix,
        ),
        Some("#") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_pattern_word(ctx, &word.unwrap_or_default())?,
            PatternRemoval::SmallestPrefix,
        ),
        Some("##") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_pattern_word(ctx, &word.unwrap_or_default())?,
            PatternRemoval::LargestPrefix,
        ),
        Some(_) => Err(ExpandError {
            message: "unsupported parameter expansion".to_string(),
        }),
    }
}

fn expand_braced_parameter_text<C: Context>(
    ctx: &mut C,
    expr: &str,
) -> Result<String, ExpandError> {
    if expr == "#" {
        return Ok(lookup_param(ctx, "#").unwrap_or_default());
    }
    if let Some(name) = expr.strip_prefix('#') {
        let value = require_set_parameter(ctx, name, lookup_param(ctx, name))?;
        return Ok(value.chars().count().to_string());
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, &name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    match op.as_deref() {
        None => require_set_parameter(ctx, &name, value),
        Some(":-") => {
            if !is_set || is_null {
                expand_parameter_text(ctx, &word.unwrap_or_default())
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("-") => {
            if !is_set {
                expand_parameter_text(ctx, &word.unwrap_or_default())
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":=") => {
            if !is_set || is_null {
                assign_parameter_text(ctx, &name, &word.unwrap_or_default())
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("=") => {
            if !is_set {
                assign_parameter_text(ctx, &name, &word.unwrap_or_default())
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":?") => {
            if !is_set || is_null {
                let message = expand_parameter_error_text(
                    ctx,
                    name.as_str(),
                    word,
                    "parameter null or not set",
                )?;
                Err(ExpandError { message })
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("?") => {
            if !is_set {
                let message =
                    expand_parameter_error_text(ctx, name.as_str(), word, "parameter not set")?;
                Err(ExpandError { message })
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some(":+") => {
            if is_set && !is_null {
                expand_parameter_text(ctx, &word.unwrap_or_default())
            } else {
                Ok(String::new())
            }
        }
        Some("+") => {
            if is_set {
                expand_parameter_text(ctx, &word.unwrap_or_default())
            } else {
                Ok(String::new())
            }
        }
        Some("%") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_text(ctx, &word.unwrap_or_default())?,
            PatternRemoval::SmallestSuffix,
        ),
        Some("%%") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_text(ctx, &word.unwrap_or_default())?,
            PatternRemoval::LargestSuffix,
        ),
        Some("#") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_text(ctx, &word.unwrap_or_default())?,
            PatternRemoval::SmallestPrefix,
        ),
        Some("##") => remove_parameter_pattern(
            require_set_parameter(ctx, &name, value)?,
            &expand_parameter_text(ctx, &word.unwrap_or_default())?,
            PatternRemoval::LargestPrefix,
        ),
        Some(_) => Err(ExpandError {
            message: "unsupported parameter expansion".to_string(),
        }),
    }
}

fn assign_parameter<C: Context>(
    ctx: &mut C,
    name: &str,
    raw_word: &str,
    quoted: bool,
) -> Result<String, ExpandError> {
    if !is_name(name) {
        return Err(ExpandError {
            message: format!("{name}: cannot assign in parameter expansion"),
        });
    }
    let value = expand_parameter_word(ctx, raw_word, quoted)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn assign_parameter_text<C: Context>(
    ctx: &mut C,
    name: &str,
    raw_word: &str,
) -> Result<String, ExpandError> {
    if !is_name(name) {
        return Err(ExpandError {
            message: format!("{name}: cannot assign in parameter expansion"),
        });
    }
    let value = expand_parameter_text(ctx, raw_word)?;
    ctx.set_var(name, value.clone())?;
    Ok(value)
}

fn expand_parameter_error_text<C: Context>(
    ctx: &mut C,
    name: &str,
    word: Option<String>,
    default_message: &str,
) -> Result<String, ExpandError> {
    let raw = word.unwrap_or_else(|| format!("{name}: {default_message}"));
    expand_parameter_text(ctx, &raw)
}

fn expand_parameter_word<C: Context>(
    ctx: &mut C,
    raw: &str,
    _quoted: bool,
) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_segments(&expanded.segments))
}

fn expand_parameter_pattern_word<C: Context>(
    ctx: &mut C,
    raw: &str,
) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(render_pattern_from_segments(&expanded.segments))
}

fn parse_parameter_expression(
    expr: &str,
) -> Result<(String, Option<String>, Option<String>), ExpandError> {
    let chars: Vec<char> = expr.chars().collect();
    if chars.is_empty() {
        return Err(ExpandError {
            message: "empty parameter expansion".to_string(),
        });
    }
    let mut index = 0usize;
    let name = if chars[0].is_ascii_digit() {
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        chars[..index].iter().collect()
    } else if matches!(chars[0], '?' | '$' | '!' | '#' | '*' | '@') {
        index = 1;
        chars[..index].iter().collect()
    } else if chars[0] == '_' || chars[0].is_ascii_alphabetic() {
        while index < chars.len() && (chars[index] == '_' || chars[index].is_ascii_alphanumeric()) {
            index += 1;
        }
        chars[..index].iter().collect()
    } else {
        return Err(ExpandError {
            message: "invalid parameter expansion".to_string(),
        });
    };

    if index == chars.len() {
        return Ok((name, None, None));
    }

    let rest: String = chars[index..].iter().collect();
    for op in [
        ":-", ":=", ":?", ":+", "%%", "##", "-", "=", "?", "+", "%", "#",
    ] {
        if let Some(word) = rest.strip_prefix(op) {
            return Ok((name, Some(op.to_string()), Some(word.to_string())));
        }
    }
    Ok((
        name,
        Some(rest.chars().next().unwrap_or_default().to_string()),
        Some(rest.chars().skip(1).collect()),
    ))
}

fn lookup_param<C: Context>(ctx: &C, name: &str) -> Option<String> {
    if name == "0" {
        return Some(ctx.shell_name().to_string());
    }
    if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_digit()) {
        return name
            .parse::<usize>()
            .ok()
            .and_then(|index| ctx.positional_param(index));
    }
    let mut chars = name.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        if let Some(value) = ctx.special_param(ch) {
            return Some(value);
        }
    }
    ctx.env_var(name)
}

fn require_set_parameter<C: Context>(
    ctx: &C,
    name: &str,
    value: Option<String>,
) -> Result<String, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != "@" && name != "*" {
        return Err(ExpandError {
            message: format!("{name}: parameter not set"),
        });
    }
    Ok(value.unwrap_or_default())
}

#[derive(Debug)]
struct ExpandedWord {
    segments: Vec<Segment>,
    had_quoted_content: bool,
    has_at_expansion: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Field {
    text: String,
    has_unquoted_glob: bool,
}

fn split_fields_from_segments(segments: &[Segment], ifs: &str) -> Vec<Field> {
    if ifs.is_empty() {
        return vec![Field {
            text: flatten_segments(segments),
            has_unquoted_glob: segments.iter().any(
                |seg| matches!(seg, Segment::Text(text, false) if text.chars().any(is_glob_char)),
            ),
        }];
    }

    let ifs_ws: Vec<char> = ifs
        .chars()
        .filter(|ch| matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let ifs_other: Vec<char> = ifs
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n'))
        .collect();
    let chars = flatten_segment_chars(segments);

    let mut fields = Vec::new();
    let mut current = String::new();
    let mut current_glob = false;
    let mut index = 0usize;

    while index < chars.len() {
        let (ch, quoted) = chars[index];
        if !quoted && ifs_other.contains(&ch) {
            fields.push(Field {
                text: std::mem::take(&mut current),
                has_unquoted_glob: current_glob,
            });
            current_glob = false;
            index += 1;
            while index < chars.len() && !chars[index].1 && ifs_ws.contains(&chars[index].0) {
                index += 1;
            }
            continue;
        }
        if !quoted && ifs_ws.contains(&ch) {
            if !current.is_empty() {
                fields.push(Field {
                    text: std::mem::take(&mut current),
                    has_unquoted_glob: current_glob,
                });
                current_glob = false;
            }
            while index < chars.len() && !chars[index].1 && ifs_ws.contains(&chars[index].0) {
                index += 1;
            }
            continue;
        }
        current_glob |= !quoted && is_glob_char(ch);
        current.push(ch);
        index += 1;
    }

    if !current.is_empty() {
        fields.push(Field {
            text: current,
            has_unquoted_glob: current_glob,
        });
    }

    fields
}

fn push_segment(segments: &mut Vec<Segment>, text: String, quoted: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Text(last, last_quoted)) = segments.last_mut() {
        if *last_quoted == quoted {
            last.push_str(&text);
            return;
        }
    }
    segments.push(Segment::Text(text, quoted));
}

fn flatten_segments(segments: &[Segment]) -> String {
    let mut result = String::new();
    for seg in segments {
        if let Segment::Text(part, _) = seg {
            result.push_str(part);
        }
    }
    result
}

fn flatten_segment_chars(segments: &[Segment]) -> Vec<(char, bool)> {
    let mut chars = Vec::new();
    for seg in segments {
        if let Segment::Text(text, quoted) = seg {
            for ch in text.chars() {
                chars.push((ch, *quoted));
            }
        }
    }
    chars
}

fn render_pattern_from_segments(segments: &[Segment]) -> String {
    let mut pattern = String::new();
    for (ch, quoted) in flatten_segment_chars(segments) {
        if quoted {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern
}

fn is_glob_char(ch: char) -> bool {
    matches!(ch, '*' | '?' | '[')
}

#[derive(Clone, Copy)]
enum PatternRemoval {
    SmallestSuffix,
    LargestSuffix,
    SmallestPrefix,
    LargestPrefix,
}

fn remove_parameter_pattern(
    value: String,
    pattern: &str,
    mode: PatternRemoval,
) -> Result<String, ExpandError> {
    let chars: Vec<char> = value.chars().collect();
    match mode {
        PatternRemoval::SmallestPrefix => {
            for end in 0..=chars.len() {
                if pattern_matches(&chars[..end].iter().collect::<String>(), pattern) {
                    return Ok(chars[end..].iter().collect());
                }
            }
        }
        PatternRemoval::LargestPrefix => {
            for end in (0..=chars.len()).rev() {
                if pattern_matches(&chars[..end].iter().collect::<String>(), pattern) {
                    return Ok(chars[end..].iter().collect());
                }
            }
        }
        PatternRemoval::SmallestSuffix => {
            for start in (0..=chars.len()).rev() {
                if pattern_matches(&chars[start..].iter().collect::<String>(), pattern) {
                    return Ok(chars[..start].iter().collect());
                }
            }
        }
        PatternRemoval::LargestSuffix => {
            for start in 0..=chars.len() {
                if pattern_matches(&chars[start..].iter().collect::<String>(), pattern) {
                    return Ok(chars[..start].iter().collect());
                }
            }
        }
    }
    Ok(value)
}

fn expand_pathname(pattern: &str) -> Vec<String> {
    if !pattern.chars().any(is_glob_char) {
        return vec![pattern.to_string()];
    }
    let absolute = pattern.starts_with('/');
    let segments: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let base = if absolute {
        PathBuf::from("/")
    } else {
        PathBuf::from(".")
    };
    let mut matches = Vec::new();
    expand_path_segments(&base, &segments, 0, absolute, &mut matches);
    matches.sort();
    matches
}

fn expand_path_segments(
    base: &Path,
    segments: &[&str],
    index: usize,
    absolute: bool,
    matches: &mut Vec<String>,
) {
    if index == segments.len() {
        let text = if absolute {
            base.display().to_string()
        } else {
            base.strip_prefix(".").unwrap_or(base).display().to_string()
        };
        matches.push(if text.is_empty() {
            ".".to_string()
        } else {
            text
        });
        return;
    }

    let segment = segments[index];
    if !segment.chars().any(is_glob_char) {
        let next = base.join(segment);
        if sys::file_exists(&next.display().to_string()) {
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
        return;
    }

    let Ok(mut names) = sys::read_dir_entries(&base.display().to_string()) else {
        return;
    };
    names.sort();
    for name in names {
        if name.starts_with('.') && !segment.starts_with('.') {
            continue;
        }
        if pattern_matches(&name, segment) {
            expand_path_segments(&base.join(&name), segments, index + 1, absolute, matches);
        }
    }
}

fn pattern_matches(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    pattern_matches_inner(&text, 0, &pattern, 0)
}

fn pattern_matches_inner(text: &[char], ti: usize, pattern: &[char], pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    match pattern[pi] {
        '*' => (ti..=text.len()).any(|next| pattern_matches_inner(text, next, pattern, pi + 1)),
        '?' => ti < text.len() && pattern_matches_inner(text, ti + 1, pattern, pi + 1),
        '[' => match match_bracket(text.get(ti).copied(), pattern, pi) {
            Some((matched, next_pi)) => {
                matched && pattern_matches_inner(text, ti + 1, pattern, next_pi)
            }
            None => {
                ti < text.len()
                    && text[ti] == '['
                    && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
            }
        },
        '\\' if pi + 1 < pattern.len() => {
            ti < text.len()
                && text[ti] == pattern[pi + 1]
                && pattern_matches_inner(text, ti + 1, pattern, pi + 2)
        }
        ch => {
            ti < text.len()
                && text[ti] == ch
                && pattern_matches_inner(text, ti + 1, pattern, pi + 1)
        }
    }
}

fn match_charclass(class: &str, ch: char) -> bool {
    match class {
        "alnum" => ch.is_ascii_alphanumeric(),
        "alpha" => ch.is_ascii_alphabetic(),
        "blank" => ch == ' ' || ch == '\t',
        "cntrl" => ch.is_ascii_control(),
        "digit" => ch.is_ascii_digit(),
        "graph" => ch.is_ascii_graphic(),
        "lower" => ch.is_ascii_lowercase(),
        "print" => ch.is_ascii_graphic() || ch == ' ',
        "punct" => ch.is_ascii_punctuation(),
        "space" => ch.is_ascii_whitespace(),
        "upper" => ch.is_ascii_uppercase(),
        "xdigit" => ch.is_ascii_hexdigit(),
        _ => false,
    }
}

fn match_bracket(current: Option<char>, pattern: &[char], start: usize) -> Option<(bool, usize)> {
    let current = current?;
    let mut index = start + 1;
    if index >= pattern.len() {
        return None;
    }

    let mut negate = false;
    if matches!(pattern.get(index), Some('!') | Some('^')) {
        negate = true;
        index += 1;
    }

    let first_elem = true;
    let mut matched = false;
    let mut saw_closer = false;
    let mut first_elem = first_elem;
    while index < pattern.len() {
        if pattern[index] == ']' && !first_elem {
            saw_closer = true;
            index += 1;
            break;
        }

        first_elem = false;

        if pattern[index] == '[' && index + 1 < pattern.len() && pattern[index + 1] == ':' {
            if let Some(end) = pattern[index + 2..]
                .iter()
                .zip(pattern[index + 3..].iter())
                .position(|(&a, &b)| a == ':' && b == ']')
            {
                let class_name: String = pattern[index + 2..index + 2 + end].iter().collect();
                matched |= match_charclass(&class_name, current);
                index = index + 2 + end + 2;
                continue;
            }
        }

        let first = if pattern[index] == '\\' && index + 1 < pattern.len() {
            index += 1;
            pattern[index]
        } else {
            pattern[index]
        };
        if index + 2 < pattern.len() && pattern[index + 1] == '-' && pattern[index + 2] != ']' {
            let last = pattern[index + 2];
            matched |= first <= current && current <= last;
            index += 3;
        } else {
            matched |= current == first;
            index += 1;
        }
    }

    if saw_closer {
        Some((if negate { !matched } else { matched }, index))
    } else {
        None
    }
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn expand_arithmetic_expression<C: Context>(
    ctx: &mut C,
    expression: &str,
) -> Result<String, ExpandError> {
    let chars: Vec<char> = expression.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            let (expansion, consumed) = expand_dollar(ctx, &chars[i..], true)?;
            match expansion {
                Expansion::One(s) => result.push_str(&s),
                Expansion::AtFields(fields) => {
                    result.push_str(&fields.join(" "));
                }
            }
            i += consumed;
        } else if chars[i] == '`' {
            i += 1;
            let command = scan_backtick_command(&chars, &mut i, true)?;
            let output = ctx.command_substitute(&command)?;
            result.push_str(output.trim_end_matches('\n'));
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    Ok(result)
}

fn eval_arithmetic<C: Context>(ctx: &mut C, expression: &str) -> Result<i64, ExpandError> {
    let mut parser = ArithmeticParser::new(ctx, expression);
    let value = parser.parse_assignment()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ExpandError {
            message: "unexpected trailing arithmetic tokens".to_string(),
        });
    }
    Ok(value)
}

struct ArithmeticParser<'a, C> {
    chars: Vec<char>,
    index: usize,
    ctx: &'a mut C,
}

fn arith_err(msg: &str) -> ExpandError {
    ExpandError {
        message: msg.to_string(),
    }
}

impl<'a, C: Context> ArithmeticParser<'a, C> {
    fn new(ctx: &'a mut C, raw: &str) -> Self {
        Self {
            chars: raw.chars().collect(),
            index: 0,
            ctx,
        }
    }

    fn parse_assignment(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        let save = self.index;
        if let Some(name) = self.try_scan_name() {
            self.skip_ws();
            if let Some(op) = self.try_consume_assign_op() {
                let rhs = self.parse_assignment()?;
                let value = if op == "=" {
                    rhs
                } else {
                    let lhs = self.resolve_var(&name)?;
                    apply_compound_assign(&op, lhs, rhs)?
                };
                self.ctx
                    .set_var(&name, value.to_string())
                    .map_err(|e| arith_err(&e.message))?;
                return Ok(value);
            }
            self.index = save;
        }
        self.parse_ternary()
    }

    fn try_consume_assign_op(&mut self) -> Option<String> {
        let remaining: String = self.chars[self.index..].iter().collect();
        for op in &[
            "<<=", ">>=", "&=", "^=", "|=", "*=", "/=", "%=", "+=", "-=", "=",
        ] {
            if remaining.starts_with(op) {
                if *op == "=" && remaining.starts_with("==") {
                    return None;
                }
                self.index += op.len();
                return Some(op.to_string());
            }
        }
        None
    }

    fn parse_ternary(&mut self) -> Result<i64, ExpandError> {
        let cond = self.parse_logical_or()?;
        self.skip_ws();
        if self.consume('?') {
            let then_val = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(':') {
                return Err(arith_err("expected ':' in ternary expression"));
            }
            let else_val = self.parse_assignment()?;
            Ok(if cond != 0 { then_val } else { else_val })
        } else {
            Ok(cond)
        }
    }

    fn parse_logical_or(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_logical_and()?;
        loop {
            self.skip_ws();
            if self.consume_str("||") {
                let rhs = self.parse_logical_and()?;
                value = i64::from(value != 0 || rhs != 0);
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
            if self.consume_str("&&") {
                let rhs = self.parse_bitwise_or()?;
                value = i64::from(value != 0 && rhs != 0);
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
            if self.peek() == Some('|')
                && self.peek_at(1) != Some('|')
                && self.peek_at(1) != Some('=')
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
            if self.peek() == Some('^') && self.peek_at(1) != Some('=') {
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
            if self.peek() == Some('&')
                && self.peek_at(1) != Some('&')
                && self.peek_at(1) != Some('=')
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
            if self.consume_str("==") {
                value = i64::from(value == self.parse_relational()?);
            } else if self.consume_str("!=") {
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
            if self.consume_str("<=") {
                value = i64::from(value <= self.parse_shift()?);
            } else if self.consume_str(">=") {
                value = i64::from(value >= self.parse_shift()?);
            } else if self.peek() == Some('<') && self.peek_at(1) != Some('<') {
                self.index += 1;
                value = i64::from(value < self.parse_shift()?);
            } else if self.peek() == Some('>') && self.peek_at(1) != Some('>') {
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
            if self.peek() == Some('<')
                && self.peek_at(1) == Some('<')
                && self.peek_at(2) != Some('=')
            {
                self.index += 2;
                let rhs = self.parse_additive()?;
                value = value.wrapping_shl(rhs as u32);
            } else if self.peek() == Some('>')
                && self.peek_at(1) == Some('>')
                && self.peek_at(2) != Some('=')
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
            if self.peek() == Some('+') && self.peek_at(1) != Some('=') {
                self.index += 1;
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.peek() == Some('-') && self.peek_at(1) != Some('=') {
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
            if self.peek() == Some('*') && self.peek_at(1) != Some('=') {
                self.index += 1;
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.peek() == Some('/') && self.peek_at(1) != Some('=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(arith_err("division by zero"));
                }
                value /= rhs;
            } else if self.peek() == Some('%') && self.peek_at(1) != Some('=') {
                self.index += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(arith_err("division by zero"));
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
        if self.consume('+') {
            return self.parse_unary();
        }
        if self.consume('-') {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.consume('~') {
            return Ok(!self.parse_unary()?);
        }
        if self.peek() == Some('!') && self.peek_at(1) != Some('=') {
            self.index += 1;
            return Ok(i64::from(self.parse_unary()? == 0));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume('(') {
            let value = self.parse_assignment()?;
            self.skip_ws();
            if !self.consume(')') {
                return Err(arith_err("missing ')'"));
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
        if self.peek() == Some('0') {
            self.index += 1;
            if self.peek() == Some('x') || self.peek() == Some('X') {
                self.index += 1;
                let hex_start = self.index;
                while self.index < self.chars.len() && self.chars[self.index].is_ascii_hexdigit() {
                    self.index += 1;
                }
                if self.index == hex_start {
                    return Err(arith_err("invalid hex constant"));
                }
                let hex_str: String = self.chars[hex_start..self.index].iter().collect();
                return i64::from_str_radix(&hex_str, 16)
                    .map_err(|_| arith_err("invalid hex constant"));
            }
            if self.peek().map_or(false, |c| c.is_ascii_digit()) {
                while self.index < self.chars.len() && self.chars[self.index].is_ascii_digit() {
                    self.index += 1;
                }
                let oct_str: String = self.chars[start + 1..self.index].iter().collect();
                return i64::from_str_radix(&oct_str, 8)
                    .map_err(|_| arith_err("invalid octal constant"));
            }
            return Ok(0);
        }

        while self.index < self.chars.len() && self.chars[self.index].is_ascii_digit() {
            self.index += 1;
        }
        if start == self.index {
            return Err(arith_err("expected arithmetic operand"));
        }
        self.chars[start..self.index]
            .iter()
            .collect::<String>()
            .parse::<i64>()
            .map_err(|_| arith_err("invalid arithmetic operand"))
    }

    fn try_scan_name(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.index;
        if self.index < self.chars.len()
            && (self.chars[self.index].is_ascii_alphabetic() || self.chars[self.index] == '_')
        {
            self.index += 1;
            while self.index < self.chars.len()
                && (self.chars[self.index].is_ascii_alphanumeric() || self.chars[self.index] == '_')
            {
                self.index += 1;
            }
            Some(self.chars[start..self.index].iter().collect())
        } else {
            None
        }
    }

    fn resolve_var(&mut self, name: &str) -> Result<i64, ExpandError> {
        let val_str = self.ctx.env_var(name).unwrap_or_default();
        if val_str.is_empty() {
            return Ok(0);
        }
        let trimmed = val_str.trim();
        if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            i64::from_str_radix(&trimmed[2..], 16)
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        } else if trimmed.starts_with('0')
            && trimmed.len() > 1
            && trimmed[1..].chars().all(|c| c.is_ascii_digit())
        {
            i64::from_str_radix(&trimmed[1..], 8)
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        } else {
            trimmed
                .parse::<i64>()
                .map_err(|_| arith_err(&format!("invalid variable value for '{name}'")))
        }
    }

    fn skip_ws(&mut self) {
        while self.index < self.chars.len() && self.chars[self.index].is_whitespace() {
            self.index += 1;
        }
    }

    fn consume(&mut self, ch: char) -> bool {
        if self.chars.get(self.index) == Some(&ch) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_str(&mut self, s: &str) -> bool {
        let s_chars: Vec<char> = s.chars().collect();
        if self.index + s_chars.len() <= self.chars.len()
            && self.chars[self.index..self.index + s_chars.len()] == s_chars[..]
        {
            self.index += s_chars.len();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.index + offset).copied()
    }

    fn is_eof(&self) -> bool {
        self.index >= self.chars.len()
    }
}

fn apply_compound_assign(op: &str, lhs: i64, rhs: i64) -> Result<i64, ExpandError> {
    match op {
        "+=" => Ok(lhs.wrapping_add(rhs)),
        "-=" => Ok(lhs.wrapping_sub(rhs)),
        "*=" => Ok(lhs.wrapping_mul(rhs)),
        "/=" => {
            if rhs == 0 {
                return Err(arith_err("division by zero"));
            }
            Ok(lhs / rhs)
        }
        "%=" => {
            if rhs == 0 {
                return Err(arith_err("division by zero"));
            }
            Ok(lhs % rhs)
        }
        "<<=" => Ok(lhs.wrapping_shl(rhs as u32)),
        ">>=" => Ok(lhs.wrapping_shr(rhs as u32)),
        "&=" => Ok(lhs & rhs),
        "^=" => Ok(lhs ^ rhs),
        "|=" => Ok(lhs | rhs),
        _ => Err(arith_err(&format!("unknown assignment operator '{op}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use std::collections::HashMap;

    struct FakeContext {
        env: HashMap<String, String>,
        positional: Vec<String>,
        pathname_expansion_enabled: bool,
        nounset_enabled: bool,
    }

    impl FakeContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert("HOME".into(), "/tmp/home".into());
            env.insert("USER".into(), "meiksh".into());
            env.insert("IFS".into(), " \t\n,".into());
            env.insert("WORDS".into(), "one,two three".into());
            env.insert("DELIMS".into(), ",,,".into());
            env.insert("EMPTY".into(), String::new());
            env.insert("X".into(), "fallback".into());
            Self {
                env,
                positional: vec!["alpha".into(), "beta".into()],
                pathname_expansion_enabled: true,
                nounset_enabled: false,
            }
        }
    }

    impl Context for FakeContext {
        fn env_var(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
        }

        fn special_param(&self, name: char) -> Option<String> {
            match name {
                '?' => Some("0".to_string()),
                '#' => Some(self.positional.len().to_string()),
                '-' => Some("aC".to_string()),
                '*' | '@' => Some(self.positional.join(" ")),
                _ => None,
            }
        }

        fn positional_param(&self, index: usize) -> Option<String> {
            if index == 0 {
                Some("meiksh".to_string())
            } else {
                self.positional.get(index - 1).cloned()
            }
        }

        fn positional_params(&self) -> Vec<String> {
            self.positional.clone()
        }

        fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
            self.env.insert(name.to_string(), value);
            Ok(())
        }

        fn pathname_expansion_enabled(&self) -> bool {
            self.pathname_expansion_enabled
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &str {
            "meiksh"
        }

        fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError> {
            Ok(format!("{command}\n"))
        }

        fn home_dir_for_user(&self, name: &str) -> Option<String> {
            match name {
                "testuser" => Some("/home/testuser".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn expands_home_and_params() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~/$USER".to_string(),
            },
        )
        .expect("expand");
        assert_eq!(fields, vec!["/tmp/home/meiksh".to_string()]);
    }

    #[test]
    fn expands_arithmetic_expressions() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 + 2 * 3))".to_string()
                }
            )
            .expect("expand"),
            vec!["7".to_string()]
        );
    }

    #[test]
    fn expands_command_substitution() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_words(
                &mut ctx,
                &[
                    Word {
                        raw: "$WORDS".to_string()
                    },
                    Word {
                        raw: "$(printf hi)".to_string()
                    },
                ],
            )
            .expect("expand"),
            vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
                "printf".to_string(),
                "hi".to_string(),
            ]
        );
    }

    #[test]
    fn preserves_quoted_and_escaped_characters() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$0 $1\"".to_string()
                }
            )
            .expect("expand"),
            vec!["meiksh alpha".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\\$HOME".to_string()
                }
            )
            .expect("expand"),
            vec!["$HOME".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "a\\ b".to_string()
                }
            )
            .expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "'literal text'".to_string()
                }
            )
            .expect("expand"),
            vec!["literal text".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"cost:\\$USER\"".to_string()
                }
            )
            .expect("expand"),
            vec!["cost:$USER".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$'a b'".to_string()
                }
            )
            .expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$'line\\nnext'".to_string()
                }
            )
            .expect("expand"),
            vec!["line\nnext".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$'a b'\"".to_string()
                }
            )
            .expect("expand"),
            vec!["$'a b'".to_string()]
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "$'tab\\tstop'").expect("parameter text"),
            "tab\tstop".to_string()
        );
    }

    #[test]
    fn rejects_unterminated_quotes_and_expansions() {
        let mut ctx = FakeContext::new();
        for raw in ["'oops", "\"oops", "${USER", "$(echo", "$((1 + 2)", "$'oops"] {
            let error = expand_word(
                &mut ctx,
                &Word {
                    raw: raw.to_string(),
                },
            )
            .expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn dollar_single_quote_helpers_cover_escape_matrix() {
        let chars: Vec<char> = "$'\\\"\\'\\\\\\a\\b\\e\\f\\n\\r\\t\\v\\cA\\c\\\\\\x41\\101Z'"
            .chars()
            .collect();
        let (value, consumed) = parse_dollar_single_quoted(&chars).expect("parse");
        assert_eq!(consumed, chars.len());
        assert_eq!(
            value,
            format!(
                "\"'\\{}\u{0008}\u{001b}\u{000c}\n\r\t\u{000b}\u{0001}\u{001c}\\x41AZ",
                '\u{0007}'
            )
        );

        let unterminated_backslash: Vec<char> = "$'\\".chars().collect();
        assert!(parse_dollar_single_quoted(&unterminated_backslash).is_err());

        let unterminated_control: Vec<char> = "$'\\c".chars().collect();
        assert!(parse_dollar_single_quoted(&unterminated_control).is_err());

        let no_hex_digits: Vec<char> = "$'\\xZ'".chars().collect();
        let (value, _) = parse_dollar_single_quoted(&no_hex_digits).expect("parse no hex");
        assert_eq!(value, "xZ");

        let hex_digits: Vec<char> = "$'\\x41'".chars().collect();
        let (value, _) = parse_dollar_single_quoted(&hex_digits).expect("parse hex");
        assert_eq!(value, "A");

        let unspecified_escape: Vec<char> = "$'\\z'".chars().collect();
        let (value, _) =
            parse_dollar_single_quoted(&unspecified_escape).expect("parse unspecified");
        assert_eq!(value, "z");

        assert_eq!(control_escape('\\'), '\u{001c}');
        assert_eq!(control_escape('?'), '\u{007f}');
        assert_eq!(control_escape('A'), '\u{0001}');
        assert_eq!(
            parse_variable_base_escape(&['4', '1', '2'], 16, 2),
            (0x41, 2)
        );
        assert_eq!(
            parse_variable_base_escape(&['1', '0', '1', '7'], 8, 3),
            (0o101, 3)
        );
        assert_eq!(parse_variable_base_escape(&['Z'], 16, 2), (0, 0));
    }

    #[test]
    fn rejects_bad_arithmetic() {
        let mut ctx = FakeContext::new();
        for raw in ["$((1 / 0))", "$((1 + ))", "$((1 1))"] {
            let error = expand_word(
                &mut ctx,
                &Word {
                    raw: raw.to_string(),
                },
            )
            .expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn supports_parameter_operators_and_positionals() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "e".into(),
            "f".into(),
            "g".into(),
            "h".into(),
            "i".into(),
            "j".into(),
        ];

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${10}".into()
                }
            )
            .expect("expand"),
            vec!["j".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$10".into() }).expect("expand"),
            vec!["a0".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${#10}".into()
                }
            )
            .expect("expand"),
            vec!["1".to_string()]
        );
        ctx.env.insert("IFS".into(), ":".into());
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$*".into() }).expect("expand"),
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string(),
                "g".to_string(),
                "h".to_string(),
                "i".to_string(),
                "j".to_string()
            ]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                }
            )
            .expect("expand"),
            vec!["a:b:c:d:e:f:g:h:i:j".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET-word}".into()
                }
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET:-word}".into()
                }
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${EMPTY-word}".into()
                }
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${EMPTY:-word}".into()
                }
            )
            .expect("expand"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${USER:+alt}".into()
                }
            )
            .expect("expand"),
            vec!["alt".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET+alt}".into()
                }
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${NEW:=value}".into()
                }
            )
            .expect("expand"),
            vec!["value".to_string()]
        );
        assert_eq!(ctx.env.get("NEW").map(String::as_str), Some("value"));
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${#}".into() }).expect("expand"),
            vec!["10".to_string()]
        );

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?boom}".into(),
            },
        )
        .expect_err("unset error");
        assert_eq!(error.message, "boom");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?$'unterminated}".into(),
            },
        )
        .expect_err("colon-question word expansion error");
        assert!(!error.message.is_empty());

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${MISSING?$'unterminated}".into(),
            },
        )
        .expect_err("plain-question word expansion error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn performs_field_splitting_more_like_posix() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$WORDS".into()
                }
            )
            .expect("expand"),
            vec!["one".to_string(), "two".to_string(), "three".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$DELIMS".into()
                }
            )
            .expect("expand"),
            vec![String::new(), String::new(), String::new()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$EMPTY".into()
                }
            )
            .expect("expand"),
            Vec::<String>::new()
        );
        assert!(split_fields_from_segments(&[], " \t\n").is_empty());
    }

    #[test]
    fn expands_text_without_field_splitting_or_pathname_expansion() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("WORDS".into(), "one two".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "$WORDS".into()
                }
            )
            .expect("expand"),
            "one two"
        );
        assert_eq!(
            expand_word_text(&mut ctx, &Word { raw: "*".into() }).expect("expand"),
            "*"
        );
    }

    #[test]
    fn performs_pathname_expansion() {
        let dir_entries = || {
            vec![
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry("a.txt".into()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry("b.txt".into()),
                ),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::DirEntry(".hidden.txt".into()),
                ),
                t("readdir", vec![ArgMatcher::Any], TraceResult::Int(0)),
            ]
        };
        let mut trace = vec![
            t(
                "access",
                vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t(
                "opendir",
                vec![ArgMatcher::Str("/testdir".into())],
                TraceResult::Int(1),
            ),
        ];
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        trace.push(t(
            "access",
            vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        trace.push(t(
            "opendir",
            vec![ArgMatcher::Str("/testdir".into())],
            TraceResult::Int(1),
        ));
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        run_trace(trace, || {
            let mut ctx = FakeContext::new();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "/testdir/*.txt".into()
                    }
                )
                .expect("glob"),
                vec!["/testdir/a.txt".to_string(), "/testdir/b.txt".to_string()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "\\*.txt".into()
                    }
                )
                .expect("escaped glob"),
                vec!["*.txt".to_string()]
            );
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: "/testdir/.*.txt".into()
                    }
                )
                .expect("hidden glob"),
                vec!["/testdir/.hidden.txt".to_string()]
            );
        });
    }

    #[test]
    fn can_disable_pathname_expansion_via_context() {
        assert_no_syscalls(|| {
            let mut ctx = FakeContext::new();
            ctx.pathname_expansion_enabled = false;
            let pattern = "/testdir/*.txt".to_string();
            assert_eq!(
                expand_word(
                    &mut ctx,
                    &Word {
                        raw: pattern.clone()
                    }
                )
                .expect("noglob"),
                vec![pattern]
            );
        });
    }

    #[test]
    fn helper_paths_cover_remaining_branches() {
        let ctx = FakeContext::new();
        assert_eq!(lookup_param(&ctx, "?"), Some("0".to_string()));
        assert_eq!(lookup_param(&ctx, "0"), Some("meiksh".to_string()));
        assert_eq!(lookup_param(&ctx, "X"), Some("fallback".to_string()));
        assert_eq!(lookup_param(&ctx, "99"), None);
        assert_eq!(
            ctx.positional_params(),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(ctx.positional_param(0), Some("meiksh".to_string()));

        let mut segs = Vec::new();
        push_segment(&mut segs, "a".into(), false);
        push_segment(&mut segs, String::new(), false);
        push_segment(&mut segs, "b".into(), false);
        push_segment(&mut segs, "c".into(), true);
        assert_eq!(
            segs,
            vec![
                Segment::Text("ab".to_string(), false),
                Segment::Text("c".to_string(), true)
            ]
        );

        assert_eq!(flatten_segments(&segs), "abc".to_string());
        assert!(pattern_matches("beta", "b*"));
        assert!(!pattern_matches("beta", "a*"));
        let mut ctx2 = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx2, "42").expect("direct eval"), 42);
        assert!(eval_arithmetic(&mut ctx2, "(1 + 2").is_err());

        let mut parser = ArithmeticParser::new(&mut ctx2, "9");
        parser.index = 99;
        assert!(parser.is_eof());
    }

    #[test]
    fn nounset_option_rejects_plain_unset_parameter_expansions() {
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "$UNSET".into(),
            },
        )
        .expect_err("nounset variable");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET}".into(),
            },
        )
        .expect_err("nounset braced");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error =
            expand_word(&mut ctx, &Word { raw: "$9".into() }).expect_err("nounset positional");
        assert_eq!(error.message, "9: parameter not set");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${UNSET-word}".into()
                }
            )
            .expect("default still works"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                }
            )
            .expect("star exempt"),
            vec!["alpha beta".to_string()]
        );
    }

    struct DefaultPathContext {
        env: HashMap<String, String>,
        nounset_enabled: bool,
    }

    impl DefaultPathContext {
        fn new() -> Self {
            let mut env = HashMap::new();
            env.insert("HOME".into(), "/tmp/home".into());
            Self {
                env,
                nounset_enabled: false,
            }
        }
    }

    impl Context for DefaultPathContext {
        fn env_var(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
        }

        fn special_param(&self, _name: char) -> Option<String> {
            None
        }

        fn positional_param(&self, index: usize) -> Option<String> {
            if index == 0 {
                Some("meiksh".to_string())
            } else {
                None
            }
        }

        fn positional_params(&self) -> Vec<String> {
            Vec::new()
        }

        fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
            self.env.insert(name.to_string(), value);
            Ok(())
        }

        fn nounset_enabled(&self) -> bool {
            self.nounset_enabled
        }

        fn shell_name(&self) -> &str {
            "meiksh"
        }

        fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError> {
            Ok(format!("{command}\n"))
        }

        fn home_dir_for_user(&self, _name: &str) -> Option<String> {
            None
        }
    }

    fn expect_one(result: Result<(Expansion, usize), ExpandError>) -> (String, usize) {
        let (expansion, consumed) = result.expect("expansion");
        match expansion {
            Expansion::One(s) => (s, consumed),
            Expansion::AtFields(_) => panic!("expected One, got AtFields"),
        }
    }

    #[test]
    fn direct_expand_dollar_covers_fallbacks_and_nesting() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, &['$'], false)),
            ("$".to_string(), 1)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, &['$', '-'], false)),
            ("aC".to_string(), 2)
        );
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, &['$', '$'], false)),
            ("".to_string(), 2)
        );

        let (at_expansion, at_consumed) =
            expand_dollar(&mut ctx, &['$', '@'], true).expect("quoted at");
        assert_eq!(at_consumed, 2);
        match at_expansion {
            Expansion::AtFields(fields) => {
                assert_eq!(fields, vec!["alpha".to_string(), "beta".to_string()]);
            }
            _ => panic!("expected AtFields for quoted $@"),
        }

        let arithmetic_chars: Vec<char> = "$((1 + (2 * 3)))".chars().collect();
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, &arithmetic_chars, false)),
            ("7".to_string(), arithmetic_chars.len())
        );

        let command_chars: Vec<char> = "$(printf (hi))".chars().collect();
        assert_eq!(
            expect_one(expand_dollar(&mut ctx, &command_chars, false)),
            ("printf (hi)".to_string(), command_chars.len())
        );
    }

    #[test]
    fn parameter_text_expansion_avoids_command_substitution() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("EMPTY".into(), String::new());

        assert_eq!(
            expand_parameter_text(&mut ctx, "${HOME:-/fallback}/.shrc").expect("parameter text"),
            "/tmp/home/.shrc"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "${EMPTY:-$HOME}/nested").expect("nested default"),
            "/tmp/home/nested"
        );
        assert_eq!(
            expand_parameter_text(&mut ctx, "$(printf nope)${HOME}").expect("literal command"),
            "$(printf nope)/tmp/home"
        );
    }

    #[test]
    fn parameter_text_dollar_helpers_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$'],).expect("single"),
            ("$".to_string(), 1)
        );
        let unterminated: Vec<char> = "${HOME".chars().collect();
        assert!(expand_parameter_dollar(&mut ctx, &unterminated).is_err());
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$', '0']).expect("zero"),
            ("meiksh".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$', '?']).expect("special"),
            ("0".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$', '1']).expect("positional"),
            ("alpha".to_string(), 2)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$', 'H', 'O', 'M', 'E']).expect("name"),
            ("/tmp/home".to_string(), 5)
        );
        assert_eq!(
            expand_parameter_dollar(&mut ctx, &['$', '-']).expect("dash"),
            ("aC".to_string(), 2)
        );
    }

    #[test]
    fn parameter_text_assignment_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "#").expect("hash"),
            "2"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "#HOME").expect("length"),
            "9"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME-word").expect("dash set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "UNSET-word").expect("dash unset"),
            "word"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:=value").expect("colon assign set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "UNSET:=value").expect("assign unset"),
            "value"
        );
        assert_eq!(ctx.env.get("UNSET").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING3=value").expect("assign equals unset"),
            "value"
        );
        assert_eq!(ctx.env.get("MISSING3").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME=value").expect("assign set"),
            "/tmp/home"
        );
        assert!(assign_parameter_text(&mut ctx, "1", "value").is_err());
    }

    #[test]
    fn nounset_option_rejects_length_and_pattern_expansions_of_unset_parameters() {
        let mut ctx = DefaultPathContext::new();
        ctx.nounset_enabled = true;

        let error = expand_braced_parameter_text(&mut ctx, "#UNSET").expect_err("nounset length");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error =
            expand_braced_parameter_text(&mut ctx, "UNSET%.*").expect_err("nounset pattern");
        assert_eq!(error.message, "UNSET: parameter not set");
    }

    #[test]
    fn parameter_text_question_operator_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("EMPTY".into(), String::new());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:?boom").expect("colon question set"),
            "/tmp/home"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME?boom").expect("question set"),
            "/tmp/home"
        );
        let colon_question = expand_braced_parameter_text(&mut ctx, "EMPTY:?boom")
            .expect_err("colon question unset");
        assert_eq!(colon_question.message, "boom");
        let question =
            expand_braced_parameter_text(&mut ctx, "MISSING?boom").expect_err("question unset");
        assert_eq!(question.message, "boom");
        let colon_default =
            expand_braced_parameter_text(&mut ctx, "EMPTY:?").expect_err("colon default");
        assert_eq!(colon_default.message, "");
        let question_default =
            expand_braced_parameter_text(&mut ctx, "MISSING?").expect_err("question default");
        assert_eq!(question_default.message, "");
    }

    #[test]
    fn parameter_text_question_propagates_word_expansion_error() {
        let mut ctx = FakeContext::new();
        let err = expand_braced_parameter_text(&mut ctx, "MISSING:?$'unterminated")
            .expect_err("colon-question text expansion error");
        assert!(!err.message.is_empty());
        let err = expand_braced_parameter_text(&mut ctx, "MISSING?$'unterminated")
            .expect_err("plain-question text expansion error");
        assert!(!err.message.is_empty());
    }

    #[test]
    fn parameter_text_plus_and_pattern_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta".into());
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME:+alt").expect("colon plus"),
            "alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING2:+alt").expect("colon plus unset"),
            ""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "HOME+alt").expect("plus set"),
            "alt"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "MISSING2+alt").expect("plus unset"),
            ""
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED%.*").expect("suffix"),
            "alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED%%.*").expect("largest suffix"),
            "alpha"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED#*.").expect("prefix"),
            "beta"
        );
        assert_eq!(
            expand_braced_parameter_text(&mut ctx, "DOTTED##*.").expect("largest prefix"),
            "beta"
        );
        assert!(expand_braced_parameter_text(&mut ctx, "HOME::word").is_err());
    }

    #[test]
    fn parameter_helpers_cover_more_edge_cases() {
        let mut ctx = FakeContext::new();

        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:-word", false).expect("default set"),
            "meiksh"
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:=word", false).expect("assign set"),
            "meiksh"
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "MISSING=value", false).expect("assign unset"),
            "value"
        );
        assert_eq!(ctx.env.get("MISSING").map(String::as_str), Some("value"));
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER=value", false).expect("assign set"),
            "meiksh"
        );
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER?boom", false).expect("question set"),
            "meiksh"
        );
        let error =
            expand_braced_parameter(&mut ctx, "UNSET?boom", false).expect_err("question unset");
        assert_eq!(error.message, "boom");
        assert_eq!(
            expand_braced_parameter(&mut ctx, "USER:?boom", false).expect("colon question set"),
            "meiksh"
        );

        let error = assign_parameter(&mut ctx, "1", "value", false).expect_err("invalid assign");
        assert_eq!(error.message, "1: cannot assign in parameter expansion");

        let parsed = parse_parameter_expression("@").expect("special name");
        assert_eq!(parsed, ("@".to_string(), None, None));

        let error = parse_parameter_expression("").expect_err("empty expr");
        assert_eq!(error.message, "empty parameter expansion");

        let error = parse_parameter_expression("%oops").expect_err("invalid expr");
        assert_eq!(error.message, "invalid parameter expansion");
        assert_eq!(
            parse_parameter_expression("USER%%tail").expect("largest suffix"),
            (
                "USER".to_string(),
                Some("%%".to_string()),
                Some("tail".to_string())
            )
        );
        assert_eq!(
            parse_parameter_expression("USER/tail").expect("unknown operator"),
            (
                "USER".to_string(),
                Some("/".to_string()),
                Some("tail".to_string())
            )
        );

        let error =
            expand_braced_parameter(&mut ctx, "USER/tail", false).expect_err("unsupported expr");
        assert_eq!(error.message, "unsupported parameter expansion");
    }

    #[test]
    fn field_and_pattern_helpers_cover_corner_cases() {
        let segs = vec![Segment::Text("*.txt".to_string(), false)];
        assert_eq!(
            split_fields_from_segments(&segs, ""),
            vec![Field {
                text: "*.txt".to_string(),
                has_unquoted_glob: true,
            }]
        );

        assert_eq!(
            split_fields_from_segments(&[Segment::Text("alpha,  beta".to_string(), false)], " ,"),
            vec![
                Field {
                    text: "alpha".to_string(),
                    has_unquoted_glob: false,
                },
                Field {
                    text: "beta".to_string(),
                    has_unquoted_glob: false,
                },
            ]
        );

        assert_eq!(expand_pathname("plain.txt"), vec!["plain.txt".to_string()]);

        let mut matches = Vec::new();
        expand_path_segments(
            Path::new("/definitely/not/a/real/dir"),
            &["*.txt"],
            0,
            false,
            &mut matches,
        );
        assert!(matches.is_empty());

        let mut matches = Vec::new();
        expand_path_segments(Path::new("."), &[], 0, false, &mut matches);
        assert_eq!(matches, vec![".".to_string()]);

        assert!(pattern_matches("x", "?"));
        assert!(pattern_matches("[", "["));
        assert!(pattern_matches("]", r"\]"));
        assert!(pattern_matches("b", "[a-c]"));
        assert!(pattern_matches("d", "[!a-c]"));

        assert!(pattern_matches("a", "[[:alpha:]]"));
        assert!(pattern_matches("Z", "[[:alpha:]]"));
        assert!(!pattern_matches("5", "[[:alpha:]]"));
        assert!(pattern_matches("3", "[[:alnum:]]"));
        assert!(pattern_matches("z", "[[:alnum:]]"));
        assert!(!pattern_matches("!", "[[:alnum:]]"));
        assert!(pattern_matches(" ", "[[:blank:]]"));
        assert!(pattern_matches("\t", "[[:blank:]]"));
        assert!(!pattern_matches("a", "[[:blank:]]"));
        assert!(pattern_matches("\x01", "[[:cntrl:]]"));
        assert!(!pattern_matches("a", "[[:cntrl:]]"));
        assert!(pattern_matches("9", "[[:digit:]]"));
        assert!(!pattern_matches("a", "[[:digit:]]"));
        assert!(pattern_matches("!", "[[:graph:]]"));
        assert!(!pattern_matches(" ", "[[:graph:]]"));
        assert!(pattern_matches("a", "[[:lower:]]"));
        assert!(!pattern_matches("A", "[[:lower:]]"));
        assert!(pattern_matches(" ", "[[:print:]]"));
        assert!(pattern_matches("a", "[[:print:]]"));
        assert!(!pattern_matches("\x01", "[[:print:]]"));
        assert!(pattern_matches(".", "[[:punct:]]"));
        assert!(!pattern_matches("a", "[[:punct:]]"));
        assert!(pattern_matches("\n", "[[:space:]]"));
        assert!(!pattern_matches("a", "[[:space:]]"));
        assert!(pattern_matches("A", "[[:upper:]]"));
        assert!(!pattern_matches("a", "[[:upper:]]"));
        assert!(pattern_matches("f", "[[:xdigit:]]"));
        assert!(pattern_matches("F", "[[:xdigit:]]"));
        assert!(!pattern_matches("g", "[[:xdigit:]]"));
        assert!(!pattern_matches("a", "[[:bogus:]]"));
        assert!(pattern_matches("x", "[[:x]"));

        assert_eq!(match_bracket(None, &['[', 'a', ']'], 0), None);
        assert_eq!(match_bracket(Some('a'), &['['], 0), None);
        assert_eq!(
            match_bracket(Some(']'), &['[', '\\', ']', ']'], 0),
            Some((true, 4))
        );
        assert_eq!(
            render_pattern_from_segments(&[Segment::Text("*".to_string(), true)]),
            "\\*".to_string()
        );
    }

    #[test]
    fn supports_pattern_removal_parameter_expansions() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("PATHNAME".into(), "src/bin/main.rs".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta.gamma".into());

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME#*/}".into()
                }
            )
            .expect("small prefix"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME##*/}".into()
                }
            )
            .expect("large prefix"),
            vec!["main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME%/*}".into()
                }
            )
            .expect("small suffix"),
            vec!["src/bin".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME%%/*}".into()
                }
            )
            .expect("large suffix"),
            vec!["src".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${PATHNAME#\"src/\"}".into()
                }
            )
            .expect("quoted pattern"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED#*.}".into()
                }
            )
            .expect("wildcard prefix"),
            vec!["beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED##*.}".into()
                }
            )
            .expect("largest wildcard prefix"),
            vec!["gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%.*}".into()
                }
            )
            .expect("wildcard suffix"),
            vec!["alpha.beta".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%%.*}".into()
                }
            )
            .expect("largest wildcard suffix"),
            vec!["alpha".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED#\"*.\"}".into()
                }
            )
            .expect("quoted wildcard"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${DOTTED%}".into()
                }
            )
            .expect("empty suffix pattern"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "${MISSING%%*.}".into()
                }
            )
            .expect("unset value"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn arithmetic_parser_covers_more_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(eval_arithmetic(&mut ctx, "9 - 2 - 1").expect("subtract"), 6);
        assert_eq!(eval_arithmetic(&mut ctx, "8 / 2").expect("divide"), 4);
        assert_eq!(eval_arithmetic(&mut ctx, "9 % 4").expect("modulo"), 1);
        assert_eq!(eval_arithmetic(&mut ctx, "(1 + 2)").expect("parens"), 3);
        assert_eq!(eval_arithmetic(&mut ctx, "-5").expect("negation"), -5);

        let error = eval_arithmetic(&mut ctx, "5 % 0").expect_err("mod zero");
        assert_eq!(error.message, "division by zero");

        let error = eval_arithmetic(&mut ctx, "999999999999999999999999999999999999999")
            .expect_err("overflow");
        assert_eq!(error.message, "invalid arithmetic operand");
    }

    #[test]
    fn default_pathname_context_trait_impl() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(ctx.special_param('?'), None);
        assert_eq!(ctx.positional_param(0), Some("meiksh".to_string()));
        assert_eq!(ctx.positional_param(1), None);
        ctx.set_var("NAME", "value".to_string()).expect("set var");
        assert_eq!(ctx.env_var("NAME"), Some("value".to_string()));
        assert_eq!(ctx.shell_name(), "meiksh");
        assert_eq!(
            ctx.command_substitute("printf ok").expect("substitute"),
            "printf ok\n"
        );
    }

    #[test]
    fn unmatched_glob_returns_pattern_literally() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "*.definitely-no-match".to_string()
                }
            )
            .expect("unmatched glob"),
            vec!["*.definitely-no-match".to_string()]
        );
    }

    #[test]
    fn bracket_helpers_cover_missing_closer() {
        assert_eq!(match_bracket(Some('a'), &['[', 'a'], 0), None);
    }

    #[test]
    fn expands_here_documents_without_field_splitting() {
        let mut ctx = FakeContext::new();
        let expanded = expand_here_document(&mut ctx, "hello $USER\n$(printf hi)\n$((1 + 2))\n")
            .expect("expand heredoc");
        assert_eq!(expanded, "hello meiksh\nprintf hi\n3\n");

        let escaped =
            expand_here_document(&mut ctx, "\\$USER\nline\\\ncontinued\n").expect("expand heredoc");
        assert_eq!(escaped, "$USER\nlinecontinued\n");

        let trailing = expand_here_document(&mut ctx, "keep\\").expect("expand heredoc");
        assert_eq!(trailing, "keep\\");

        let literal = expand_here_document(&mut ctx, "\\x").expect("expand heredoc");
        assert_eq!(literal, "\\x");
    }

    #[test]
    fn quoted_at_produces_separate_fields() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                }
            )
            .expect("quoted at 3"),
            vec!["a", "b", "c"]
        );

        ctx.positional = vec!["one".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                }
            )
            .expect("quoted at 1"),
            vec!["one"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@\"".into()
                }
            )
            .expect("quoted at 0"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quoted_at_with_prefix_and_suffix() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"pre$@suf\"".into()
                }
            )
            .expect("prefix suffix"),
            vec!["prea", "bsuf"]
        );

        ctx.positional = vec!["only".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"[$@]\"".into()
                }
            )
            .expect("brackets one"),
            vec!["[only]"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"pre$@suf\"".into()
                }
            )
            .expect("prefix empty"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quoted_at_at_produces_merged_fields() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$@$@\"".into()
                }
            )
            .expect("at at"),
            vec!["a", "ba", "b"]
        );
    }

    #[test]
    fn unquoted_at_undergoes_field_splitting() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a b".into(), "c".into()];
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$@".into() }).expect("unquoted at"),
            vec!["a", "b", "c"]
        );

        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$@".into() }).expect("unquoted at empty"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn quoted_star_joins_with_ifs() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into(), "c".into()];
        ctx.env.insert("IFS".into(), ":".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                }
            )
            .expect("star colon"),
            vec!["a:b:c"]
        );

        ctx.env.insert("IFS".into(), String::new());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                }
            )
            .expect("star empty ifs"),
            vec!["abc"]
        );

        ctx.env.remove("IFS");
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"$*\"".into()
                }
            )
            .expect("star unset ifs"),
            vec!["a b c"]
        );
    }

    #[test]
    fn backtick_command_substitution_in_expander() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "`echo hello`".into()
                }
            )
            .expect("backtick"),
            vec!["echo", "hello"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"`echo hello`\"".into()
                }
            )
            .expect("quoted bt"),
            vec!["echo hello"]
        );
    }

    #[test]
    fn backtick_backslash_escapes() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "`echo \\$USER`".into()
                }
            )
            .expect("escaped dollar"),
            "echo $USER"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "\"`echo \\$USER`\"".into()
                }
            )
            .expect("escaped dollar dq"),
            "echo $USER"
        );
    }

    #[test]
    fn brace_scanning_respects_quotes_and_nesting() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("VAR".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\"a}b\"}".into()
                }
            )
            .expect("quoted brace in default"),
            "a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$(echo ok)}".into()
                }
            )
            .expect("command sub in brace"),
            "echo ok"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$((1+2))}".into()
                }
            )
            .expect("arith in brace"),
            "3"
        );

        ctx.env.insert("INNER".into(), "val".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-${INNER}}".into()
                }
            )
            .expect("nested brace"),
            "val"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-`echo hi`}".into()
                }
            )
            .expect("backtick in brace"),
            "echo hi"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-'a}b'}".into()
                }
            )
            .expect("single quote in brace"),
            "a}b"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\\}}".into()
                }
            )
            .expect("escaped brace"),
            "}"
        );
    }

    #[test]
    fn here_document_expands_at_sign() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["x".into(), "y".into()];
        let result = expand_here_document(&mut ctx, "$@\n").expect("heredoc at");
        assert_eq!(result, "x y\n");
    }

    #[test]
    fn error_parameter_expansion_operators() {
        let mut ctx = FakeContext::new();
        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET:?custom error}".into(),
            },
        )
        .expect_err("colon question");
        assert_eq!(error.message, "custom error");

        let error = expand_word(
            &mut ctx,
            &Word {
                raw: "${UNSET?also error}".into(),
            },
        )
        .expect_err("question");
        assert_eq!(error.message, "also error");
    }

    #[test]
    fn flatten_segment_chars_skips_at_break() {
        let segs = vec![
            Segment::Text("a".into(), false),
            Segment::AtBreak,
            Segment::Text("b".into(), true),
        ];
        let chars = flatten_segment_chars(&segs);
        assert_eq!(chars, vec![('a', false), ('b', true)]);
    }

    #[test]
    fn scan_to_closing_brace_error_on_unterminated() {
        let chars: Vec<char> = "${var".chars().collect();
        let err = scan_to_closing_brace(&chars, 2).expect_err("unterminated");
        assert_eq!(err.message, "unterminated parameter expansion");
    }

    #[test]
    fn expand_word_empty_quoted_at_with_other_quoted() {
        let mut ctx = FakeContext::new();
        ctx.positional = Vec::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"\"\"$@\"".into()
                }
            )
            .expect("empty at dq"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn backtick_inside_double_quotes_with_buffer() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"hello `echo world`\"".into()
                }
            )
            .expect("bt dq buffer"),
            vec!["hello echo world"]
        );
    }

    #[test]
    fn scan_backtick_command_unterminated() {
        let chars: Vec<char> = "`unterminated".chars().collect();
        let mut index = 1usize;
        let err = scan_backtick_command(&chars, &mut index, false).expect_err("unterminated");
        assert_eq!(err.message, "unterminated backquote");
    }

    #[test]
    fn scan_backtick_command_escape_outside_dq() {
        let chars: Vec<char> = "`echo \\\\ok`".chars().collect();
        let mut index = 1usize;
        let result = scan_backtick_command(&chars, &mut index, false).expect("bt escape");
        assert_eq!(result, "echo \\ok");
    }

    #[test]
    fn here_document_with_at_expansion() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["a".into(), "b".into()];
        let result = expand_here_document(&mut ctx, "args: $@\n").expect("heredoc @");
        assert_eq!(result, "args: a b\n");
    }

    #[test]
    fn brace_scanning_handles_complex_nesting() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("VAR".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$((2+3))}".into()
                }
            )
            .expect("arith in brace scan"),
            "5"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-$(echo deep)}".into()
                }
            )
            .expect("cmd sub in brace scan"),
            "echo deep"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-`echo bt`}".into()
                }
            )
            .expect("backtick in brace scan"),
            "echo bt"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${VAR:-\"inside\"}".into()
                }
            )
            .expect("dq in brace scan with escape"),
            "inside"
        );
    }

    #[test]
    fn error_parameter_expansion_with_null_or_not_set() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("EMPTY".into(), String::new());

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${EMPTY:?null or unset}".into(),
            },
        )
        .expect_err("colon question null");
        assert_eq!(err.message, "null or unset");

        let ok = expand_word(
            &mut ctx,
            &Word {
                raw: "\"${EMPTY?not an error}\"".into(),
            },
        )
        .expect("question set but empty");
        assert_eq!(ok, vec![String::new()]);

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOEXIST?custom msg}".into(),
            },
        )
        .expect_err("question unset");
        assert_eq!(err.message, "custom msg");
    }

    #[test]
    fn field_splitting_empty_result_returns_empty_vec() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("WS".into(), "   ".into());
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$WS".into() }).expect("whitespace only"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn at_break_with_glob_in_at_fields() {
        let mut ctx = FakeContext::new();
        ctx.pathname_expansion_enabled = false;
        ctx.positional = vec!["*.txt".into(), "b".into()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
        )
        .expect("at with glob-like");
        assert_eq!(result, vec!["*.txt", "b"]);
    }

    #[test]
    fn flatten_expansion_covers_at_fields() {
        assert_eq!(flatten_expansion(Expansion::One("hello".into())), "hello");
        assert_eq!(
            flatten_expansion(Expansion::AtFields(vec!["a".into(), "b".into()])),
            "a b"
        );
    }

    #[test]
    fn scan_backtick_non_special_escape_in_dquote() {
        let chars: Vec<char> = "`echo \\x`".chars().collect();
        let mut index = 1usize;
        let result = scan_backtick_command(&chars, &mut index, true).expect("non-special escape");
        assert_eq!(result, "echo \\x");
    }

    #[test]
    fn at_empty_combined_with_at_break() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["x".into()];
        let result = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
        )
        .expect("at one param");
        assert_eq!(result, vec!["x"]);

        ctx.positional = Vec::new();
        let result2 = expand_word(
            &mut ctx,
            &Word {
                raw: "\"$@\"".into(),
            },
        )
        .expect("at empty");
        assert_eq!(result2, Vec::<String>::new());
    }

    #[test]
    fn brace_scanning_with_arith_and_cmd_sub_and_backtick() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("V".into(), String::new());

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-$((1+(2*3)))}".into()
                }
            )
            .expect("nested arith in scan"),
            "7"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-$(echo (hi))}".into()
                }
            )
            .expect("nested cmd sub in scan"),
            "echo (hi)"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-`echo \\\\x`}".into()
                }
            )
            .expect("bt escape in scan"),
            "echo \\x"
        );

        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${V:-\"q\\}x\"}".into()
                }
            )
            .expect("dq escape in scan"),
            "q}x"
        );
    }

    #[test]
    fn colon_question_error_with_null_value() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("NULL".into(), String::new());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NULL:?is null}".into(),
            },
        )
        .expect_err(":? with null");
        assert_eq!(err.message, "is null");

        ctx.nounset_enabled = true;
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NULL:?$NOVAR}".into(),
            },
        )
        .expect_err(":? nounset propagation");
        assert_eq!(err.message, "NOVAR: parameter not set");

        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOEXIST?$NOVAR}".into(),
            },
        )
        .expect_err("? nounset propagation");
        assert_eq!(err.message, "NOVAR: parameter not set");
    }

    #[test]
    fn question_error_with_unset_default_message() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "${NOVAR?}".into(),
            },
        )
        .expect_err("? with unset");
        assert_eq!(err.message, "");

        ctx.env.insert("SET".into(), "val".into());
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${SET:?no error}".into()
                }
            )
            .expect(":? success"),
            "val"
        );
        assert_eq!(
            expand_word_text(
                &mut ctx,
                &Word {
                    raw: "${SET?no error}".into()
                }
            )
            .expect("? success"),
            "val"
        );
    }

    #[test]
    fn dquote_backslash_preserves_literal_for_non_special_chars() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: r#""\a\b\c""#.to_string(),
            },
        )
        .expect("dquote bs");
        assert_eq!(fields, vec![r"\a\b\c"]);
    }

    #[test]
    fn dquote_backslash_escapes_special_chars() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\$""#.into()
                }
            )
            .expect("escape $"),
            vec!["$"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\\""#.into()
                }
            )
            .expect("escape bs"),
            vec!["\\"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: r#""\"""#.into()
                }
            )
            .expect("escape dq"),
            vec!["\""]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "\"\\`\"".into()
                }
            )
            .expect("escape bt"),
            vec!["`"]
        );
    }

    #[test]
    fn dquote_backslash_newline_is_line_continuation() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "\"ab\\\ncd\"".to_string(),
            },
        )
        .expect("line continuation");
        assert_eq!(fields, vec!["abcd"]);
    }

    #[test]
    fn tilde_user_expansion() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~testuser/bin".into(),
            },
        )
        .expect("tilde user");
        assert_eq!(fields, vec!["/home/testuser/bin"]);
    }

    #[test]
    fn tilde_unknown_user_preserved() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~nosuchuser/dir".into(),
            },
        )
        .expect("tilde unknown");
        assert_eq!(fields, vec!["~nosuchuser/dir"]);
    }

    #[test]
    fn tilde_user_without_slash() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~testuser".into(),
            },
        )
        .expect("tilde user no slash");
        assert_eq!(fields, vec!["/home/testuser"]);
    }

    #[test]
    fn tilde_after_colon_in_assignment() {
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: "~/bin:~testuser/lib".into(),
            },
        )
        .expect("tilde colon");
        assert_eq!(result, "/tmp/home/bin:/home/testuser/lib");
    }

    #[test]
    fn arith_variable_reference() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("count".into(), "7".into());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$((count + 3))".into(),
            },
        )
        .expect("arith var");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_dollar_variable_reference() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("n".into(), "5".into());
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$(($n * 2))".into(),
            },
        )
        .expect("arith $var");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_comparison_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 < 5))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((5 < 3))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 <= 3))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((5 > 3))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 >= 5))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 == 3))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 != 5))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
    }

    #[test]
    fn arith_bitwise_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 & 3))".into()
                }
            )
            .unwrap(),
            vec!["2"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 | 3))".into()
                }
            )
            .unwrap(),
            vec!["7"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((6 ^ 3))".into()
                }
            )
            .unwrap(),
            vec!["5"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((~0))".into()
                }
            )
            .unwrap(),
            vec!["-1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 << 4))".into()
                }
            )
            .unwrap(),
            vec!["16"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((16 >> 2))".into()
                }
            )
            .unwrap(),
            vec!["4"]
        );
    }

    #[test]
    fn arith_logical_operators() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 && 1))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 && 0))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 || 1))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 || 0))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((!0))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((!5))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_ternary_operator() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((1 ? 10 : 20))".into()
                }
            )
            .unwrap(),
            vec!["10"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0 ? 10 : 20))".into()
                }
            )
            .unwrap(),
            vec!["20"]
        );
    }

    #[test]
    fn arith_assignment_operators() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "10".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x = 5))".into()
                }
            )
            .unwrap(),
            vec!["5"]
        );
        assert_eq!(ctx.env.get("x").unwrap(), "5");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x += 3))".into()
                }
            )
            .unwrap(),
            vec!["8"]
        );
        assert_eq!(ctx.env.get("x").unwrap(), "8");

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x -= 2))".into()
                }
            )
            .unwrap(),
            vec!["6"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x *= 3))".into()
                }
            )
            .unwrap(),
            vec!["18"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x /= 6))".into()
                }
            )
            .unwrap(),
            vec!["3"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x %= 2))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );

        ctx.env.insert("x".into(), "4".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x <<= 2))".into()
                }
            )
            .unwrap(),
            vec!["16"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x >>= 1))".into()
                }
            )
            .unwrap(),
            vec!["8"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x &= 3))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );

        ctx.env.insert("x".into(), "5".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x |= 2))".into()
                }
            )
            .unwrap(),
            vec!["7"]
        );

        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x ^= 3))".into()
                }
            )
            .unwrap(),
            vec!["4"]
        );
    }

    #[test]
    fn arith_hex_and_octal_constants() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0xff))".into()
                }
            )
            .unwrap(),
            vec!["255"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0X1A))".into()
                }
            )
            .unwrap(),
            vec!["26"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((010))".into()
                }
            )
            .unwrap(),
            vec!["8"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((0))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_unary_plus() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((+5))".into()
                }
            )
            .unwrap(),
            vec!["5"]
        );
    }

    #[test]
    fn arith_unset_variable_is_zero() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((nosuch))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_nested_parens_and_precedence() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((2 + 3 * 4))".into()
                }
            )
            .unwrap(),
            vec!["14"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$(((2 + 3) * 4))".into()
                }
            )
            .unwrap(),
            vec!["20"]
        );
    }

    #[test]
    fn arith_variable_in_hex_value() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("h".into(), "0xff".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((h))".into()
                }
            )
            .unwrap(),
            vec!["255"]
        );
    }

    #[test]
    fn arith_variable_in_octal_value() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("o".into(), "010".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((o))".into()
                }
            )
            .unwrap(),
            vec!["8"]
        );
    }

    #[test]
    fn split_colons_handles_quotes_and_backslash() {
        let parts = split_on_unquoted_colons("'b:c':d");
        assert_eq!(parts, vec!["'b:c'", "d"]);

        let parts = split_on_unquoted_colons(r#""b:c":d"#);
        assert_eq!(parts, vec![r#""b:c""#, "d"]);

        let parts = split_on_unquoted_colons(r"a\:b:c");
        assert_eq!(parts, vec![r"a\:b", "c"]);

        let parts = split_on_unquoted_colons(r#""a\"b":c"#);
        assert_eq!(parts, vec![r#""a\"b""#, "c"]);

        let parts = split_on_unquoted_colons("${x:-a:b}:c");
        assert_eq!(parts, vec!["${x:-a:b}", "c"]);

        let parts = split_on_unquoted_colons("$(echo a:b):c");
        assert_eq!(parts, vec!["$(echo a:b)", "c"]);

        let parts = split_on_unquoted_colons("${a:-${b:-x:y}}:z");
        assert_eq!(parts, vec!["${a:-${b:-x:y}}", "z"]);
    }

    #[test]
    fn dquote_trailing_backslash_is_literal() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: r#""abc\"#.to_string(),
            },
        );
        assert!(fields.is_err());
    }

    #[test]
    fn tilde_with_quoted_char_breaks_tilde_prefix() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "~'user'".into(),
            },
        )
        .expect("tilde quoted");
        assert_eq!(fields, vec!["/tmp/homeuser"]);
    }

    #[test]
    fn arith_backtick_in_expression() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$((`7` + 3))".into(),
            },
        )
        .expect("arith backtick");
        assert_eq!(fields, vec!["10"]);
    }

    #[test]
    fn arith_not_equal_via_parse_unary() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((3 != 3))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_compound_assign_div_by_zero() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "5".into());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((x /= 0))".into(),
            },
        )
        .unwrap_err();
        assert_eq!(err.message, "division by zero");

        ctx.env.insert("x".into(), "5".into());
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((x %= 0))".into(),
            },
        )
        .unwrap_err();
        assert_eq!(err.message, "division by zero");
    }

    #[test]
    fn tilde_colon_assignment_with_quotes() {
        let mut ctx = FakeContext::new();
        let result = expand_assignment_value(
            &mut ctx,
            &Word {
                raw: "~/a:'literal:colon'".into(),
            },
        )
        .expect("colon assign with quotes");
        assert_eq!(result, "/tmp/home/a:literal:colon");
    }

    #[test]
    fn arith_equality_not_confused_with_assignment() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("x".into(), "5".into());
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x == 5))".into()
                }
            )
            .unwrap(),
            vec!["1"]
        );
        assert_eq!(
            expand_word(
                &mut ctx,
                &Word {
                    raw: "$((x == 3))".into()
                }
            )
            .unwrap(),
            vec!["0"]
        );
    }

    #[test]
    fn arith_ternary_missing_colon_error() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((1 ? 2 3))".into(),
            },
        )
        .unwrap_err();
        assert!(err.message.contains("':'"));
    }

    #[test]
    fn arith_invalid_hex_constant() {
        let mut ctx = FakeContext::new();
        let err = expand_word(
            &mut ctx,
            &Word {
                raw: "$((0x))".into(),
            },
        )
        .unwrap_err();
        assert!(err.message.contains("hex"));
    }

    #[test]
    fn arith_at_fields_in_expression() {
        let mut ctx = FakeContext::new();
        ctx.positional = vec!["3".into()];
        let fields = expand_word(
            &mut ctx,
            &Word {
                raw: "$(($@ + 2))".into(),
            },
        )
        .expect("at fields arith");
        assert_eq!(fields, vec!["5"]);
    }

    #[test]
    fn apply_compound_assign_unknown_op_returns_error() {
        let err = apply_compound_assign("??=", 1, 2).unwrap_err();
        assert!(err.message.contains("unknown"));
    }
}
