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
    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &str;
    fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError>;
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
    if expanded.only_quoted {
        return Ok(vec![flatten_pieces(&expanded.pieces)]);
    }

    let fields = split_fields_from_pieces(&expanded.pieces, &ctx.env_var("IFS").unwrap_or_else(|| " \t\n".to_string()));
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

pub fn expand_word_text<C: Context>(ctx: &mut C, word: &Word) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, &word.raw)?;
    Ok(flatten_pieces(&expanded.pieces))
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

fn expand_raw<C: Context>(ctx: &mut C, raw: &str) -> Result<ExpandedWord, ExpandError> {
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0usize;
    let mut pieces = Vec::new();
    let mut only_quoted = true;

    while index < chars.len() {
        match chars[index] {
            '\'' => {
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
                push_piece(&mut pieces, chars[start..index].iter().collect(), true);
                index += 1;
            }
            '"' => {
                index += 1;
                let mut buffer = String::new();
                while index < chars.len() && chars[index] != '"' {
                    match chars[index] {
                        '\\' => {
                            index += 1;
                            if index < chars.len() {
                                buffer.push(chars[index]);
                                index += 1;
                            }
                        }
                        '$' => {
                            if !buffer.is_empty() {
                                push_piece(&mut pieces, std::mem::take(&mut buffer), true);
                            }
                            let (value, consumed) = expand_dollar(ctx, &chars[index..], true)?;
                            push_piece(&mut pieces, value, true);
                            index += consumed;
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
                    push_piece(&mut pieces, buffer, true);
                }
                index += 1;
            }
            '\\' => {
                only_quoted = false;
                index += 1;
                if index < chars.len() {
                    push_piece(&mut pieces, chars[index].to_string(), true);
                    index += 1;
                }
            }
            '$' => {
                let dollar_single_quotes = chars.get(index + 1) == Some(&'\'');
                if !dollar_single_quotes {
                    only_quoted = false;
                }
                let (value, consumed) = expand_dollar(ctx, &chars[index..], false)?;
                push_piece(&mut pieces, value, dollar_single_quotes);
                index += consumed;
            }
            '~' if index == 0 => {
                only_quoted = false;
                let home = ctx.env_var("HOME").unwrap_or_else(|| "~".to_string());
                push_piece(&mut pieces, home, false);
                index += 1;
            }
            ch => {
                only_quoted = false;
                push_piece(&mut pieces, ch.to_string(), false);
                index += 1;
            }
        }
    }

    Ok(ExpandedWord { pieces, only_quoted })
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
                let (value, consumed) = expand_dollar(ctx, &chars[index..], false)?;
                result.push_str(&value);
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
) -> Result<(String, usize), ExpandError> {
    if chars.len() < 2 {
        return Ok(("$".to_string(), 1));
    }

    match chars[1] {
        '\'' if !quoted => parse_dollar_single_quoted(chars),
        '{' => {
            let mut index = 2usize;
            while index < chars.len() && chars[index] != '}' {
                index += 1;
            }
            if index >= chars.len() {
                return Err(ExpandError {
                    message: "unterminated parameter expansion".to_string(),
                });
            }
            let expr: String = chars[2..index].iter().collect();
            let value = expand_braced_parameter(ctx, &expr, quoted)?;
            Ok((value, index + 1))
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
                            let value = eval_arithmetic(&expression)?;
                            return Ok((value.to_string(), index + 2));
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
                            return Ok((trimmed, index + 1));
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
        '?' | '$' | '!' | '#' | '*' | '@' | '-' | '0' => {
            let ch = chars[1];
            let value = if ch == '0' {
                require_set_parameter(ctx, "0", Some(ctx.shell_name().to_string()))?
            } else {
                require_set_parameter(ctx, &ch.to_string(), ctx.special_param(ch))?
            };
            let _ = quoted;
            Ok((value, 2))
        }
        next if next.is_ascii_digit() => {
            Ok((
                require_set_parameter(
                    ctx,
                    &next.to_string(),
                    ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize),
                )?,
                2,
            ))
        }
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            let mut name = String::new();
            while index < chars.len() && (chars[index] == '_' || chars[index].is_ascii_alphanumeric()) {
                name.push(chars[index]);
                index += 1;
            }
            Ok((require_set_parameter(ctx, &name, lookup_param(ctx, &name))?, index))
        }
        _ => Ok(("$".to_string(), 1)),
    }
}

fn expand_parameter_dollar<C: Context>(ctx: &mut C, chars: &[char]) -> Result<(String, usize), ExpandError> {
    if chars.len() < 2 {
        return Ok(("$".to_string(), 1));
    }

    match chars[1] {
        '\'' => parse_dollar_single_quoted(chars),
        '{' => {
            let mut index = 2usize;
            while index < chars.len() && chars[index] != '}' {
                index += 1;
            }
            if index >= chars.len() {
                return Err(ExpandError {
                    message: "unterminated parameter expansion".to_string(),
                });
            }
            let expr: String = chars[2..index].iter().collect();
            let value = expand_braced_parameter_text(ctx, &expr)?;
            Ok((value, index + 1))
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
            while index < chars.len() && (chars[index] == '_' || chars[index].is_ascii_alphanumeric()) {
                name.push(chars[index]);
                index += 1;
            }
            Ok((require_set_parameter(ctx, &name, lookup_param(ctx, &name))?, index))
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
                        result.push(control_escape(chars[index]));
                    }
                    'x' => {
                        let (value, consumed) = parse_variable_base_escape(&chars[(index + 1)..], 16, 2);
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
    while consumed < max_digits
        && consumed < chars.len()
        && chars[consumed].is_digit(base)
    {
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

fn expand_braced_parameter<C: Context>(ctx: &mut C, expr: &str, quoted: bool) -> Result<String, ExpandError> {
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
                let message = expand_parameter_word(ctx, &word.unwrap_or_else(|| format!("{name}: parameter null or not set")), quoted)?;
                Err(ExpandError { message })
            } else {
                Ok(value.unwrap_or_default())
            }
        }
        Some("?") => {
            if !is_set {
                let message = expand_parameter_word(ctx, &word.unwrap_or_else(|| format!("{name}: parameter not set")), quoted)?;
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

fn expand_braced_parameter_text<C: Context>(ctx: &mut C, expr: &str) -> Result<String, ExpandError> {
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
                let message = expand_parameter_error_text(ctx, name.as_str(), word, "parameter null or not set")?;
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

fn assign_parameter_text<C: Context>(ctx: &mut C, name: &str, raw_word: &str) -> Result<String, ExpandError> {
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

fn expand_parameter_word<C: Context>(ctx: &mut C, raw: &str, _quoted: bool) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_pieces(&expanded.pieces))
}

fn expand_parameter_pattern_word<C: Context>(ctx: &mut C, raw: &str) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(render_pattern_from_pieces(&expanded.pieces))
}

fn parse_parameter_expression(expr: &str) -> Result<(String, Option<String>, Option<String>), ExpandError> {
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
    for op in [":-", ":=", ":?", ":+", "%%", "##", "-", "=", "?", "+", "%", "#"] {
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
        return name.parse::<usize>().ok().and_then(|index| ctx.positional_param(index));
    }
    let mut chars = name.chars();
    if let (Some(ch), None) = (chars.next(), chars.next()) {
        if let Some(value) = ctx.special_param(ch) {
            return Some(value);
        }
    }
    ctx.env_var(name)
}

fn require_set_parameter<C: Context>(ctx: &C, name: &str, value: Option<String>) -> Result<String, ExpandError> {
    if value.is_none() && ctx.nounset_enabled() && name != "@" && name != "*" {
        return Err(ExpandError {
            message: format!("{name}: parameter not set"),
        });
    }
    Ok(value.unwrap_or_default())
}

#[derive(Debug)]
struct ExpandedWord {
    pieces: Vec<(String, bool)>,
    only_quoted: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Field {
    text: String,
    has_unquoted_glob: bool,
}

fn split_fields_from_pieces(pieces: &[(String, bool)], ifs: &str) -> Vec<Field> {
    if ifs.is_empty() {
        return vec![Field {
            text: flatten_pieces(pieces),
            has_unquoted_glob: pieces
                .iter()
                .any(|(text, quoted)| !quoted && text.chars().any(is_glob_char)),
        }];
    }

    let ifs_ws: Vec<char> = ifs.chars().filter(|ch| matches!(ch, ' ' | '\t' | '\n')).collect();
    let ifs_other: Vec<char> = ifs.chars().filter(|ch| !matches!(ch, ' ' | '\t' | '\n')).collect();
    let chars = flatten_piece_chars(pieces);

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

fn push_piece(segments: &mut Vec<(String, bool)>, text: String, quoted: bool) {
    if text.is_empty() {
        return;
    }
    if let Some((last, last_quoted)) = segments.last_mut() {
        if *last_quoted == quoted {
            last.push_str(&text);
            return;
        }
    }
    segments.push((text, quoted));
}

fn flatten_pieces(pieces: &[(String, bool)]) -> String {
    pieces.iter().map(|(part, _)| part).cloned().collect()
}

fn flatten_piece_chars(pieces: &[(String, bool)]) -> Vec<(char, bool)> {
    let mut chars = Vec::new();
    for (text, quoted) in pieces {
        for ch in text.chars() {
            chars.push((ch, *quoted));
        }
    }
    chars
}

fn render_pattern_from_pieces(pieces: &[(String, bool)]) -> String {
    let mut pattern = String::new();
    for (ch, quoted) in flatten_piece_chars(pieces) {
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

fn remove_parameter_pattern(value: String, pattern: &str, mode: PatternRemoval) -> Result<String, ExpandError> {
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
    let segments: Vec<&str> = pattern.split('/').filter(|segment| !segment.is_empty()).collect();
    let base = if absolute { PathBuf::from("/") } else { PathBuf::from(".") };
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
        matches.push(if text.is_empty() { ".".to_string() } else { text });
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
            Some((matched, next_pi)) => matched && pattern_matches_inner(text, ti + 1, pattern, next_pi),
            None => ti < text.len() && text[ti] == '[' && pattern_matches_inner(text, ti + 1, pattern, pi + 1),
        },
        '\\' if pi + 1 < pattern.len() => {
            ti < text.len() && text[ti] == pattern[pi + 1] && pattern_matches_inner(text, ti + 1, pattern, pi + 2)
        }
        ch => ti < text.len() && text[ti] == ch && pattern_matches_inner(text, ti + 1, pattern, pi + 1),
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

    let mut matched = false;
    let mut saw_closer = false;
    while index < pattern.len() {
        if pattern[index] == ']' {
            saw_closer = true;
            index += 1;
            break;
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

fn eval_arithmetic(expression: &str) -> Result<i64, ExpandError> {
    let mut parser = ArithmeticParser::new(expression);
    let value = parser.parse_expression()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ExpandError {
            message: "unexpected trailing arithmetic tokens".to_string(),
        });
    }
    Ok(value)
}

struct ArithmeticParser<'a> {
    chars: Vec<char>,
    index: usize,
    _raw: &'a str,
}

impl<'a> ArithmeticParser<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            chars: raw.chars().collect(),
            index: 0,
            _raw: raw,
        }
    }

    fn parse_expression(&mut self) -> Result<i64, ExpandError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_multiplicative()?;
        loop {
            self.skip_ws();
            if self.consume('+') {
                value += self.parse_multiplicative()?;
            } else if self.consume('-') {
                value -= self.parse_multiplicative()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<i64, ExpandError> {
        let mut value = self.parse_primary()?;
        loop {
            self.skip_ws();
            if self.consume('*') {
                value *= self.parse_primary()?;
            } else if self.consume('/') {
                let rhs = self.parse_primary()?;
                if rhs == 0 {
                    return Err(ExpandError {
                        message: "division by zero".to_string(),
                    });
                }
                value /= rhs;
            } else if self.consume('%') {
                let rhs = self.parse_primary()?;
                if rhs == 0 {
                    return Err(ExpandError {
                        message: "division by zero".to_string(),
                    });
                }
                value %= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_primary(&mut self) -> Result<i64, ExpandError> {
        self.skip_ws();
        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_ws();
            if !self.consume(')') {
                return Err(ExpandError {
                    message: "missing ')'".to_string(),
                });
            }
            return Ok(value);
        }
        if self.consume('-') {
            return Ok(-self.parse_primary()?);
        }

        let start = self.index;
        while self.index < self.chars.len() && self.chars[self.index].is_ascii_digit() {
            self.index += 1;
        }
        if start == self.index {
            return Err(ExpandError {
                message: "expected arithmetic operand".to_string(),
            });
        }
        let value = self.chars[start..self.index]
            .iter()
            .collect::<String>()
            .parse::<i64>()
            .map_err(|_| ExpandError {
                message: "invalid arithmetic operand".to_string(),
            })?;
        Ok(value)
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

    fn is_eof(&self) -> bool {
        self.index >= self.chars.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::sys::test_support::{run_trace, assert_no_syscalls, t, TraceResult, ArgMatcher};

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
                '*' => Some(
                    self.positional.join(
                        &self
                            .env
                            .get("IFS")
                            .cloned()
                            .unwrap_or_else(|| " \t\n".to_string())
                            .chars()
                            .next()
                            .map(|ch| ch.to_string())
                            .unwrap_or_default(),
                    ),
                ),
                '@' => Some(self.positional.join(" ")),
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
    }

    #[test]
    fn expands_home_and_params() {
        let mut ctx = FakeContext::new();
        let fields = expand_word(&mut ctx, &Word { raw: "~/$USER".to_string() }).expect("expand");
        assert_eq!(fields, vec!["/tmp/home/meiksh".to_string()]);
    }

    #[test]
    fn expands_arithmetic_and_command_substitution() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$((1 + 2 * 3))".to_string() }).expect("expand"),
            vec!["7".to_string()]
        );
        assert_eq!(
            expand_words(
                &mut ctx,
                &[
                    Word { raw: "$WORDS".to_string() },
                    Word { raw: "$(printf hi)".to_string() },
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
            expand_word(&mut ctx, &Word { raw: "\"$0 $1\"".to_string() }).expect("expand"),
            vec!["meiksh alpha".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "\\$HOME".to_string() }).expect("expand"),
            vec!["$HOME".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "a\\ b".to_string() }).expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "'literal text'".to_string() }).expect("expand"),
            vec!["literal text".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "\"cost:\\$USER\"".to_string() }).expect("expand"),
            vec!["cost:$USER".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$'a b'".to_string() }).expect("expand"),
            vec!["a b".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$'line\\nnext'".to_string() }).expect("expand"),
            vec!["line\nnext".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "\"$'a b'\"".to_string() }).expect("expand"),
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
            let error = expand_word(&mut ctx, &Word { raw: raw.to_string() }).expect_err("error");
            assert!(!error.message.is_empty());
        }
    }

    #[test]
    fn dollar_single_quote_helpers_cover_escape_matrix() {
        let chars: Vec<char> = "$'\\\"\\'\\\\\\a\\b\\e\\f\\n\\r\\t\\v\\cA\\c\\\\\\x41\\101Z'".chars().collect();
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
        let (value, _) = parse_dollar_single_quoted(&unspecified_escape).expect("parse unspecified");
        assert_eq!(value, "z");

        assert_eq!(control_escape('\\'), '\u{001c}');
        assert_eq!(control_escape('?'), '\u{007f}');
        assert_eq!(control_escape('A'), '\u{0001}');
        assert_eq!(parse_variable_base_escape(&['4', '1', '2'], 16, 2), (0x41, 2));
        assert_eq!(parse_variable_base_escape(&['1', '0', '1', '7'], 8, 3), (0o101, 3));
        assert_eq!(parse_variable_base_escape(&['Z'], 16, 2), (0, 0));
    }

    #[test]
    fn rejects_bad_arithmetic() {
        let mut ctx = FakeContext::new();
        for raw in ["$((1 / 0))", "$((1 + ))", "$((1 1))"] {
            let error = expand_word(&mut ctx, &Word { raw: raw.to_string() }).expect_err("error");
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

        assert_eq!(expand_word(&mut ctx, &Word { raw: "${10}".into() }).expect("expand"), vec!["j".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "$10".into() }).expect("expand"), vec!["a0".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${#10}".into() }).expect("expand"), vec!["1".to_string()]);
        ctx.env.insert("IFS".into(), ":".into());
        assert_eq!(expand_word(&mut ctx, &Word { raw: "$*".into() }).expect("expand"), vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string(), "f".to_string(), "g".to_string(), "h".to_string(), "i".to_string(), "j".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "\"$*\"".into() }).expect("expand"), vec!["a:b:c:d:e:f:g:h:i:j".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${UNSET-word}".into() }).expect("expand"), vec!["word".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${UNSET:-word}".into() }).expect("expand"), vec!["word".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${EMPTY-word}".into() }).expect("expand"), Vec::<String>::new());
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${EMPTY:-word}".into() }).expect("expand"), vec!["word".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${USER:+alt}".into() }).expect("expand"), vec!["alt".to_string()]);
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${UNSET+alt}".into() }).expect("expand"), Vec::<String>::new());
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${NEW:=value}".into() }).expect("expand"), vec!["value".to_string()]);
        assert_eq!(ctx.env.get("NEW").map(String::as_str), Some("value"));
        assert_eq!(expand_word(&mut ctx, &Word { raw: "${#}".into() }).expect("expand"), vec!["10".to_string()]);

        let error = expand_word(&mut ctx, &Word { raw: "${UNSET:?boom}".into() }).expect_err("unset error");
        assert_eq!(error.message, "boom");
    }

    #[test]
    fn performs_field_splitting_more_like_posix() {
        let mut ctx = FakeContext::new();
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$WORDS".into() }).expect("expand"),
            vec!["one".to_string(), "two".to_string(), "three".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$DELIMS".into() }).expect("expand"),
            vec![String::new(), String::new(), String::new()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "$EMPTY".into() }).expect("expand"),
            Vec::<String>::new()
        );
        assert!(split_fields_from_pieces(&[], " \t\n").is_empty());
    }

    #[test]
    fn expands_text_without_field_splitting_or_pathname_expansion() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("WORDS".into(), "one two".into());
        assert_eq!(
            expand_word_text(&mut ctx, &Word { raw: "$WORDS".into() }).expect("expand"),
            "one two"
        );
        assert_eq!(
            expand_word_text(&mut ctx, &Word { raw: "*".into() }).expect("expand"),
            "*"
        );
    }

    #[test]
    fn performs_pathname_expansion() {
        let dir_entries = || vec![
            t("readdir", vec![ArgMatcher::Any], TraceResult::DirEntry("a.txt".into())),
            t("readdir", vec![ArgMatcher::Any], TraceResult::DirEntry("b.txt".into())),
            t("readdir", vec![ArgMatcher::Any], TraceResult::DirEntry(".hidden.txt".into())),
            t("readdir", vec![ArgMatcher::Any], TraceResult::Int(0)),
        ];
        let mut trace = vec![
            t("access", vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any], TraceResult::Int(0)),
            t("opendir", vec![ArgMatcher::Str("/testdir".into())], TraceResult::Int(1)),
        ];
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        trace.push(t("access", vec![ArgMatcher::Str("/testdir".into()), ArgMatcher::Any], TraceResult::Int(0)));
        trace.push(t("opendir", vec![ArgMatcher::Str("/testdir".into())], TraceResult::Int(1)));
        trace.extend(dir_entries());
        trace.push(t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)));
        run_trace(trace, || {
            let mut ctx = FakeContext::new();
            assert_eq!(
                expand_word(&mut ctx, &Word { raw: "/testdir/*.txt".into() }).expect("glob"),
                vec!["/testdir/a.txt".to_string(), "/testdir/b.txt".to_string()]
            );
            assert_eq!(
                expand_word(&mut ctx, &Word { raw: "\\*.txt".into() }).expect("escaped glob"),
                vec!["*.txt".to_string()]
            );
            assert_eq!(
                expand_word(&mut ctx, &Word { raw: "/testdir/.*.txt".into() }).expect("hidden glob"),
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
                expand_word(&mut ctx, &Word { raw: pattern.clone() }).expect("noglob"),
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
        assert_eq!(ctx.special_param('*'), Some("alpha beta".to_string()));
        assert_eq!(ctx.positional_param(0), Some("meiksh".to_string()));

        let mut segments = Vec::new();
        push_piece(&mut segments, "a".into(), false);
        push_piece(&mut segments, String::new(), false);
        push_piece(&mut segments, "b".into(), false);
        push_piece(&mut segments, "c".into(), true);
        assert_eq!(segments, vec![("ab".to_string(), false), ("c".to_string(), true)]);

        assert_eq!(flatten_pieces(&segments), "abc".to_string());
        assert!(pattern_matches("beta", "b*"));
        assert!(!pattern_matches("beta", "a*"));
        assert_eq!(eval_arithmetic("42").expect("direct eval"), 42);
        assert!(eval_arithmetic("(1 + 2").is_err());

        let mut parser = ArithmeticParser::new("9");
        parser.index = 99;
        assert!(parser.is_eof());
    }

    #[test]
    fn nounset_option_rejects_plain_unset_parameter_expansions() {
        let mut ctx = FakeContext::new();
        ctx.nounset_enabled = true;

        let error = expand_word(&mut ctx, &Word { raw: "$UNSET".into() }).expect_err("nounset variable");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error = expand_word(&mut ctx, &Word { raw: "${UNSET}".into() }).expect_err("nounset braced");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error = expand_word(&mut ctx, &Word { raw: "$9".into() }).expect_err("nounset positional");
        assert_eq!(error.message, "9: parameter not set");

        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${UNSET-word}".into() }).expect("default still works"),
            vec!["word".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "\"$*\"".into() }).expect("star exempt"),
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
    }

    #[test]
    fn direct_expand_dollar_covers_fallbacks_and_nesting() {
        let mut ctx = FakeContext::new();
        assert_eq!(expand_dollar(&mut ctx, &['$'], false).expect("single"), ("$".to_string(), 1));
        assert_eq!(expand_dollar(&mut ctx, &['$', '-'], false).expect("dash"), ("aC".to_string(), 2));
        assert_eq!(expand_dollar(&mut ctx, &['$', '$'], false).expect("pid default"), ("".to_string(), 2));
        assert_eq!(
            expand_dollar(&mut ctx, &['$', '@'], true).expect("quoted at"),
            ("alpha beta".to_string(), 2)
        );

        let arithmetic_chars: Vec<char> = "$((1 + (2 * 3)))".chars().collect();
        assert_eq!(
            expand_dollar(&mut ctx, &arithmetic_chars, false).expect("nested arithmetic"),
            ("7".to_string(), arithmetic_chars.len())
        );

        let command_chars: Vec<char> = "$(printf (hi))".chars().collect();
        assert_eq!(
            expand_dollar(&mut ctx, &command_chars, false).expect("nested command"),
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
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$'],).expect("single"), ("$".to_string(), 1));
        let unterminated: Vec<char> = "${HOME".chars().collect();
        assert!(expand_parameter_dollar(&mut ctx, &unterminated).is_err());
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$', '0']).expect("zero"), ("meiksh".to_string(), 2));
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$', '?']).expect("special"), ("0".to_string(), 2));
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$', '1']).expect("positional"), ("alpha".to_string(), 2));
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$', 'H', 'O', 'M', 'E']).expect("name"), ("/tmp/home".to_string(), 5));
        assert_eq!(expand_parameter_dollar(&mut ctx, &['$', '-']).expect("dash"), ("aC".to_string(), 2));
    }

    #[test]
    fn parameter_text_assignment_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        assert_eq!(expand_braced_parameter_text(&mut ctx, "#").expect("hash"), "2");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "#HOME").expect("length"), "9");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME-word").expect("dash set"), "/tmp/home");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "UNSET-word").expect("dash unset"), "word");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME:=value").expect("colon assign set"), "/tmp/home");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "UNSET:=value").expect("assign unset"), "value");
        assert_eq!(ctx.env.get("UNSET").map(String::as_str), Some("value"));
        assert_eq!(expand_braced_parameter_text(&mut ctx, "MISSING3=value").expect("assign equals unset"), "value");
        assert_eq!(ctx.env.get("MISSING3").map(String::as_str), Some("value"));
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME=value").expect("assign set"), "/tmp/home");
        assert!(assign_parameter_text(&mut ctx, "1", "value").is_err());
    }

    #[test]
    fn nounset_option_rejects_length_and_pattern_expansions_of_unset_parameters() {
        let mut ctx = DefaultPathContext::new();
        ctx.nounset_enabled = true;

        let error = expand_braced_parameter_text(&mut ctx, "#UNSET").expect_err("nounset length");
        assert_eq!(error.message, "UNSET: parameter not set");

        let error = expand_braced_parameter_text(&mut ctx, "UNSET%.*").expect_err("nounset pattern");
        assert_eq!(error.message, "UNSET: parameter not set");
    }

    #[test]
    fn parameter_text_question_operator_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("EMPTY".into(), String::new());
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME:?boom").expect("colon question set"), "/tmp/home");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME?boom").expect("question set"), "/tmp/home");
        let colon_question = expand_braced_parameter_text(&mut ctx, "EMPTY:?boom").expect_err("colon question unset");
        assert_eq!(colon_question.message, "boom");
        let question = expand_braced_parameter_text(&mut ctx, "MISSING?boom").expect_err("question unset");
        assert_eq!(question.message, "boom");
        let colon_default = expand_braced_parameter_text(&mut ctx, "EMPTY:?").expect_err("colon default");
        assert_eq!(colon_default.message, "");
        let question_default = expand_braced_parameter_text(&mut ctx, "MISSING?").expect_err("question default");
        assert_eq!(question_default.message, "");
    }

    #[test]
    fn parameter_text_plus_and_pattern_paths_are_split_out() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("HOME".into(), "/tmp/home".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta".into());
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME:+alt").expect("colon plus"), "alt");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "MISSING2:+alt").expect("colon plus unset"), "");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "HOME+alt").expect("plus set"), "alt");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "MISSING2+alt").expect("plus unset"), "");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "DOTTED%.*").expect("suffix"), "alpha");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "DOTTED%%.*").expect("largest suffix"), "alpha");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "DOTTED#*.").expect("prefix"), "beta");
        assert_eq!(expand_braced_parameter_text(&mut ctx, "DOTTED##*.").expect("largest prefix"), "beta");
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
        let error = expand_braced_parameter(&mut ctx, "UNSET?boom", false).expect_err("question unset");
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
            ("USER".to_string(), Some("%%".to_string()), Some("tail".to_string()))
        );
        assert_eq!(
            parse_parameter_expression("USER/tail").expect("unknown operator"),
            ("USER".to_string(), Some("/".to_string()), Some("tail".to_string()))
        );

        let error = expand_braced_parameter(&mut ctx, "USER/tail", false).expect_err("unsupported expr");
        assert_eq!(error.message, "unsupported parameter expansion");
    }

    #[test]
    fn field_and_pattern_helpers_cover_corner_cases() {
        let pieces = vec![("*.txt".to_string(), false)];
        assert_eq!(
            split_fields_from_pieces(&pieces, ""),
            vec![Field {
                text: "*.txt".to_string(),
                has_unquoted_glob: true,
            }]
        );

        assert_eq!(
            split_fields_from_pieces(&[("alpha,  beta".to_string(), false)], " ,"),
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
        expand_path_segments(Path::new("/definitely/not/a/real/dir"), &["*.txt"], 0, false, &mut matches);
        assert!(matches.is_empty());

        assert!(pattern_matches("x", "?"));
        assert!(pattern_matches("[", "["));
        assert!(pattern_matches("]", r"\]"));
        assert!(pattern_matches("b", "[a-c]"));
        assert!(pattern_matches("d", "[!a-c]"));
        assert_eq!(match_bracket(None, &['[', 'a', ']'], 0), None);
        assert_eq!(match_bracket(Some('a'), &['['], 0), None);
        assert_eq!(match_bracket(Some(']'), &['[', '\\', ']', ']'], 0), Some((true, 4)));
        assert_eq!(render_pattern_from_pieces(&[("*".to_string(), true)]), "\\*".to_string());
    }

    #[test]
    fn supports_pattern_removal_parameter_expansions() {
        let mut ctx = FakeContext::new();
        ctx.env.insert("PATHNAME".into(), "src/bin/main.rs".into());
        ctx.env.insert("DOTTED".into(), "alpha.beta.gamma".into());

        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${PATHNAME#*/}".into() }).expect("small prefix"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${PATHNAME##*/}".into() }).expect("large prefix"),
            vec!["main.rs".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${PATHNAME%/*}".into() }).expect("small suffix"),
            vec!["src/bin".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${PATHNAME%%/*}".into() }).expect("large suffix"),
            vec!["src".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${PATHNAME#\"src/\"}".into() }).expect("quoted pattern"),
            vec!["bin/main.rs".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED#*.}".into() }).expect("wildcard prefix"),
            vec!["beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED##*.}".into() }).expect("largest wildcard prefix"),
            vec!["gamma".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED%.*}".into() }).expect("wildcard suffix"),
            vec!["alpha.beta".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED%%.*}".into() }).expect("largest wildcard suffix"),
            vec!["alpha".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED#\"*.\"}".into() }).expect("quoted wildcard"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${DOTTED%}".into() }).expect("empty suffix pattern"),
            vec!["alpha.beta.gamma".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "${MISSING%%*.}".into() }).expect("unset value"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn arithmetic_parser_covers_more_operators() {
        assert_eq!(eval_arithmetic("9 - 2 - 1").expect("subtract"), 6);
        assert_eq!(eval_arithmetic("8 / 2").expect("divide"), 4);
        assert_eq!(eval_arithmetic("9 % 4").expect("modulo"), 1);
        assert_eq!(eval_arithmetic("(1 + 2)").expect("parens"), 3);
        assert_eq!(eval_arithmetic("-5").expect("negation"), -5);

        let error = eval_arithmetic("5 % 0").expect_err("mod zero");
        assert_eq!(error.message, "division by zero");

        let error = eval_arithmetic("999999999999999999999999999999999999999").expect_err("overflow");
        assert_eq!(error.message, "invalid arithmetic operand");
    }

    #[test]
    fn default_pathname_context_and_unmatched_glob_are_covered() {
        let mut ctx = DefaultPathContext::new();
        assert_eq!(ctx.special_param('?'), None);
        assert_eq!(ctx.positional_param(0), Some("meiksh".to_string()));
        assert_eq!(ctx.positional_param(1), None);
        ctx.set_var("NAME", "value".to_string()).expect("set var");
        assert_eq!(ctx.env_var("NAME"), Some("value".to_string()));
        assert_eq!(ctx.shell_name(), "meiksh");
        assert_eq!(ctx.command_substitute("printf ok").expect("substitute"), "printf ok\n");
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "*.definitely-no-match".to_string() }).expect("unmatched glob"),
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
}
