use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::syntax::Word;

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
        if field.has_unquoted_glob {
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
                only_quoted = false;
                let (value, consumed) = expand_dollar(ctx, &chars[index..], false)?;
                push_piece(&mut pieces, value, false);
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
        '?' | '$' | '!' | '#' | '*' | '@' | '0' => {
            let ch = chars[1];
            let value = if ch == '0' {
                ctx.shell_name().to_string()
            } else {
                ctx.special_param(ch).unwrap_or_default()
            };
            let text = if quoted && (ch == '@' || ch == '*') {
                value
            } else {
                value
            };
            Ok((text, 2))
        }
        next if next.is_ascii_digit() => {
            Ok((ctx.positional_param(next.to_digit(10).unwrap_or_default() as usize).unwrap_or_default(), 2))
        }
        next if next == '_' || next.is_ascii_alphabetic() => {
            let mut index = 1usize;
            let mut name = String::new();
            while index < chars.len() && (chars[index] == '_' || chars[index].is_ascii_alphanumeric()) {
                name.push(chars[index]);
                index += 1;
            }
            Ok((lookup_param(ctx, &name).unwrap_or_default(), index))
        }
        _ => Ok(("$".to_string(), 1)),
    }
}

fn expand_braced_parameter<C: Context>(ctx: &mut C, expr: &str, quoted: bool) -> Result<String, ExpandError> {
    if expr == "#" {
        return Ok(lookup_param(ctx, "#").unwrap_or_default());
    }
    if let Some(name) = expr.strip_prefix('#') {
        let value = lookup_param(ctx, name).unwrap_or_default();
        return Ok(value.chars().count().to_string());
    }

    let (name, op, word) = parse_parameter_expression(expr)?;
    let value = lookup_param(ctx, &name);
    let is_set = value.is_some();
    let is_null = value.as_deref().map(|s| s.is_empty()).unwrap_or(true);

    match op.as_deref() {
        None => Ok(value.unwrap_or_default()),
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

fn expand_parameter_word<C: Context>(ctx: &mut C, raw: &str, _quoted: bool) -> Result<String, ExpandError> {
    let expanded = expand_raw(ctx, raw)?;
    Ok(flatten_pieces(&expanded.pieces))
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
    for op in [":-", ":=", ":?", ":+", "-", "=", "?", "+"] {
        if let Some(word) = rest.strip_prefix(op) {
            return Ok((name, Some(op.to_string()), Some(word.to_string())));
        }
    }

    Err(ExpandError {
        message: "unsupported parameter expansion".to_string(),
    })
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

#[derive(Debug)]
struct ExpandedWord {
    pieces: Vec<(String, bool)>,
    only_quoted: bool,
}

#[derive(Debug)]
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

fn is_glob_char(ch: char) -> bool {
    matches!(ch, '*' | '?' | '[')
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
        if next.exists() {
            expand_path_segments(&next, segments, index + 1, absolute, matches);
        }
        return;
    }

    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    let mut names = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeContext {
        env: HashMap<String, String>,
        positional: Vec<String>,
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

        fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
            self.env.insert(name.to_string(), value);
            Ok(())
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
    }

    #[test]
    fn rejects_unterminated_quotes_and_expansions() {
        let mut ctx = FakeContext::new();
        for raw in ["'oops", "\"oops", "${USER", "$(echo", "$((1 + 2)"] {
            let error = expand_word(&mut ctx, &Word { raw: raw.to_string() }).expect_err("error");
            assert!(!error.message.is_empty());
        }
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
    fn performs_pathname_expansion() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("meiksh-expand-{unique}"));
        fs::create_dir(&dir).expect("mkdir");
        fs::write(dir.join("a.txt"), "").expect("write a");
        fs::write(dir.join("b.txt"), "").expect("write b");
        fs::write(dir.join(".hidden.txt"), "").expect("write hidden");

        let mut ctx = FakeContext::new();
        let visible_pattern = format!("{}/*.txt", dir.display());
        let hidden_pattern = format!("{}/.*.txt", dir.display());
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: visible_pattern }).expect("glob"),
            vec![
                format!("{}/a.txt", dir.display()),
                format!("{}/b.txt", dir.display()),
            ]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: "\\*.txt".into() }).expect("escaped glob"),
            vec!["*.txt".to_string()]
        );
        assert_eq!(
            expand_word(&mut ctx, &Word { raw: hidden_pattern }).expect("hidden glob"),
            vec![format!("{}/.hidden.txt", dir.display())]
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn helper_paths_cover_remaining_branches() {
        let ctx = FakeContext::new();
        assert_eq!(lookup_param(&ctx, "?"), Some("0".to_string()));
        assert_eq!(lookup_param(&ctx, "X"), Some("fallback".to_string()));
        assert_eq!(lookup_param(&ctx, "99"), None);

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
