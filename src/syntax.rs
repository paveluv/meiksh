use std::collections::{HashMap, VecDeque};
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<ListItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub and_or: AndOr,
    pub asynchronous: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndOr {
    pub first: Pipeline,
    pub rest: Vec<(LogicalOp, Pipeline)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pipeline {
    pub negated: bool,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Simple(SimpleCommand),
    Subshell(Program),
    Group(Program),
    FunctionDef(FunctionDef),
    If(IfCommand),
    Loop(LoopCommand),
    For(ForCommand),
    Case(CaseCommand),
    Redirected(Box<Command>, Vec<Redirection>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCommand {
    pub assignments: Vec<Assignment>,
    pub words: Vec<Word>,
    pub redirections: Vec<Redirection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub name: String,
    pub value: Word,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirection {
    pub fd: Option<i32>,
    pub kind: RedirectionKind,
    pub target: Word,
    pub here_doc: Option<HereDoc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionDef {
    pub name: String,
    pub body: Box<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfCommand {
    pub condition: Program,
    pub then_branch: Program,
    pub elif_branches: Vec<ElifBranch>,
    pub else_branch: Option<Program>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElifBranch {
    pub condition: Program,
    pub body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopCommand {
    pub kind: LoopKind,
    pub condition: Program,
    pub body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForCommand {
    pub name: String,
    pub items: Option<Vec<Word>>,
    pub body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCommand {
    pub word: Word,
    pub arms: Vec<CaseArm>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseArm {
    pub patterns: Vec<Word>,
    pub body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HereDoc {
    pub delimiter: String,
    pub body: String,
    pub expand: bool,
    pub strip_tabs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopKind {
    While,
    Until,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectionKind {
    Read,
    Write,
    ClobberWrite,
    Append,
    HereDoc,
    ReadWrite,
    DupInput,
    DupOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TokenKind {
    Word(String),
    Newline,
    Semi,
    DSemi,
    Amp,
    Pipe,
    AndIf,
    OrIf,
    LParen,
    RParen,
    Less,
    Greater,
    DGreat,
    DLess,
    DLessDash,
    LessAnd,
    GreatAnd,
    LessGreat,
    Clobber,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
}

struct Tokenized {
    tokens: Vec<Token>,
    here_docs: VecDeque<HereDoc>,
}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    parse_with_aliases(source, &HashMap::new())
}

pub fn parse_with_aliases(
    source: &str,
    aliases: &HashMap<String, String>,
) -> Result<Program, ParseError> {
    let tokenized = tokenize(source)?;
    Parser::new(tokenized.tokens, tokenized.here_docs, aliases.clone()).parse_program_until(
        false,
        &[],
        false,
    )
}

pub struct ParseSession {
    parser: Parser,
}

impl ParseSession {
    pub fn new(source: &str) -> Result<Self, ParseError> {
        let tokenized = tokenize(source)?;
        Ok(Self {
            parser: Parser::new(tokenized.tokens, tokenized.here_docs, HashMap::new()),
        })
    }

    pub fn next_item(
        &mut self,
        aliases: &HashMap<String, String>,
    ) -> Result<Option<ListItem>, ParseError> {
        self.parser.aliases = aliases.clone();
        self.parser.skip_separators();
        self.parser.expand_alias_at_command_start()?;
        if self.parser.is_eof() {
            return Ok(None);
        }
        let and_or = self.parser.parse_and_or()?;
        let asynchronous = self.parser.consume_amp();
        self.parser.skip_separators();
        Ok(Some(ListItem {
            and_or,
            asynchronous,
        }))
    }
}

/// Scan a `$'...'` (dollar-single-quote) token, preserving it raw for
/// expansion to handle later.  Backslash escapes inside `$'...'` are
/// meaningful (unlike regular single-quotes), so `\'` does NOT terminate
/// the string.
fn scan_dollar_single_quote(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    current.push('$');
    current.push('\'');
    *index += 2; // skip $'
    while *index < chars.len() {
        let ch = chars[*index];
        if ch == '\'' {
            current.push('\'');
            *index += 1;
            return Ok(());
        }
        if ch == '\\' && *index + 1 < chars.len() {
            current.push('\\');
            current.push(chars[*index + 1]);
            *index += 2;
            continue;
        }
        current.push(ch);
        *index += 1;
    }
    Err(ParseError {
        message: "unterminated dollar-single-quotes".to_string(),
    })
}

fn scan_dollar_construct(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    current.push('$');
    let next = chars[*index + 1];
    if next == '(' {
        if chars.get(*index + 2) == Some(&'(') {
            current.push('(');
            current.push('(');
            *index += 3;
            let mut depth = 1usize;
            while *index < chars.len() {
                let ch = chars[*index];
                current.push(ch);
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    if depth == 1 && chars.get(*index + 1) == Some(&')') {
                        current.push(')');
                        *index += 2;
                        return Ok(());
                    }
                    depth = depth.saturating_sub(1);
                }
                *index += 1;
            }
            return Err(ParseError {
                message: "unterminated arithmetic expansion".to_string(),
            });
        }
        current.push('(');
        *index += 2;
        scan_paren_body(chars, index, current)?;
        return Ok(());
    }
    current.push('{');
    *index += 2;
    scan_brace_body(chars, index, current)
}

fn scan_paren_body(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    let mut depth = 1usize;
    while *index < chars.len() {
        let ch = chars[*index];
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
                *index += 1;
            }
            ')' => {
                depth -= 1;
                current.push(ch);
                *index += 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            '\'' => {
                current.push(ch);
                *index += 1;
                while *index < chars.len() {
                    current.push(chars[*index]);
                    if chars[*index] == '\'' {
                        *index += 1;
                        break;
                    }
                    *index += 1;
                }
            }
            '"' => {
                current.push(ch);
                *index += 1;
                scan_dquote_body(chars, index, current)?;
            }
            '\\' => {
                current.push(ch);
                *index += 1;
                if *index < chars.len() {
                    current.push(chars[*index]);
                    *index += 1;
                }
            }
            '$' if matches!(chars.get(*index + 1), Some('(' | '{')) => {
                scan_dollar_construct(chars, index, current)?;
            }
            '`' => {
                scan_backtick_body(chars, index, current)?;
            }
            _ => {
                current.push(ch);
                *index += 1;
            }
        }
    }
    Err(ParseError {
        message: "unterminated command substitution".to_string(),
    })
}

fn scan_brace_body(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    while *index < chars.len() {
        let ch = chars[*index];
        match ch {
            '}' => {
                current.push(ch);
                *index += 1;
                return Ok(());
            }
            '\\' => {
                current.push(ch);
                *index += 1;
                if *index < chars.len() {
                    current.push(chars[*index]);
                    *index += 1;
                }
            }
            '\'' => {
                current.push(ch);
                *index += 1;
                while *index < chars.len() {
                    current.push(chars[*index]);
                    if chars[*index] == '\'' {
                        *index += 1;
                        break;
                    }
                    *index += 1;
                }
            }
            '"' => {
                current.push(ch);
                *index += 1;
                scan_dquote_body(chars, index, current)?;
            }
            '$' if matches!(chars.get(*index + 1), Some('(' | '{')) => {
                scan_dollar_construct(chars, index, current)?;
            }
            '`' => {
                scan_backtick_body(chars, index, current)?;
            }
            _ => {
                current.push(ch);
                *index += 1;
            }
        }
    }
    Err(ParseError {
        message: "unterminated parameter expansion".to_string(),
    })
}

fn scan_backtick_body(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    current.push('`');
    *index += 1;
    while *index < chars.len() {
        let next = chars[*index];
        current.push(next);
        if next == '\\' && *index + 1 < chars.len() {
            current.push(chars[*index + 1]);
            *index += 2;
            continue;
        }
        *index += 1;
        if next == '`' {
            return Ok(());
        }
    }
    Err(ParseError {
        message: "unterminated backquote".to_string(),
    })
}

fn scan_dquote_body(
    chars: &[char],
    index: &mut usize,
    current: &mut String,
) -> Result<(), ParseError> {
    while *index < chars.len() {
        let ch = chars[*index];
        current.push(ch);
        match ch {
            '\\' => {
                *index += 1;
                if *index < chars.len() {
                    current.push(chars[*index]);
                    *index += 1;
                }
            }
            '"' => {
                *index += 1;
                return Ok(());
            }
            '$' if matches!(chars.get(*index + 1), Some('(' | '{')) => {
                current.pop();
                scan_dollar_construct(chars, index, current)?;
            }
            '`' => {
                *index += 1;
                while *index < chars.len() {
                    let next = chars[*index];
                    current.push(next);
                    if next == '\\' && *index + 1 < chars.len() {
                        current.push(chars[*index + 1]);
                        *index += 2;
                        continue;
                    }
                    *index += 1;
                    if next == '`' {
                        break;
                    }
                }
            }
            _ => {
                *index += 1;
            }
        }
    }
    Err(ParseError {
        message: "unterminated double quote".to_string(),
    })
}

fn tokenize(source: &str) -> Result<Tokenized, ParseError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut here_docs = VecDeque::new();
    let mut pending_here_docs = VecDeque::new();
    let mut expect_here_doc_target = false;
    let mut index = 0usize;
    let mut current = String::new();

    while index < chars.len() {
        let ch = chars[index];
        match ch {
            ' ' | '\t' | '\r' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                index += 1;
            }
            '\n' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token {
                    kind: TokenKind::Newline,
                });
                index += 1;
                while let Some((delimiter, expand, strip_tabs)) = pending_here_docs.pop_front() {
                    let body = read_here_doc_body(&chars, &mut index, &delimiter, strip_tabs)?;
                    here_docs.push_back(HereDoc {
                        delimiter,
                        body,
                        expand,
                        strip_tabs,
                    });
                }
            }
            '#' if current.is_empty() => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            '\'' => {
                current.push(ch);
                index += 1;
                while index < chars.len() {
                    current.push(chars[index]);
                    if chars[index] == '\'' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                if !current.ends_with('\'') {
                    return Err(ParseError {
                        message: "unterminated single quote".to_string(),
                    });
                }
            }
            '"' => {
                current.push(ch);
                index += 1;
                scan_dquote_body(&chars, &mut index, &mut current)?;
            }
            '\\' => {
                if index + 1 < chars.len() && chars[index + 1] == '\n' {
                    index += 2;
                } else {
                    current.push(ch);
                    index += 1;
                    if index < chars.len() {
                        current.push(chars[index]);
                        index += 1;
                    }
                }
            }
            ';' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if matches!(chars.get(index + 1), Some(';')) {
                    tokens.push(Token {
                        kind: TokenKind::DSemi,
                    });
                    index += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Semi,
                    });
                    index += 1;
                }
            }
            '&' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if matches!(chars.get(index + 1), Some('&')) {
                    tokens.push(Token {
                        kind: TokenKind::AndIf,
                    });
                    index += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Amp,
                    });
                    index += 1;
                }
            }
            '|' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if matches!(chars.get(index + 1), Some('|')) {
                    tokens.push(Token {
                        kind: TokenKind::OrIf,
                    });
                    index += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Pipe,
                    });
                    index += 1;
                }
            }
            '(' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token {
                    kind: TokenKind::LParen,
                });
                index += 1;
            }
            ')' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token {
                    kind: TokenKind::RParen,
                });
                index += 1;
            }
            '<' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if matches!(chars.get(index + 1), Some('<')) {
                    if matches!(chars.get(index + 2), Some('-')) {
                        tokens.push(Token {
                            kind: TokenKind::DLessDash,
                        });
                        expect_here_doc_target = true;
                        index += 3;
                    } else {
                        tokens.push(Token {
                            kind: TokenKind::DLess,
                        });
                        expect_here_doc_target = true;
                        index += 2;
                    }
                } else if matches!(chars.get(index + 1), Some('&')) {
                    tokens.push(Token {
                        kind: TokenKind::LessAnd,
                    });
                    index += 2;
                } else if matches!(chars.get(index + 1), Some('>')) {
                    tokens.push(Token {
                        kind: TokenKind::LessGreat,
                    });
                    index += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Less,
                    });
                    index += 1;
                }
            }
            '>' => {
                flush_word(
                    &mut current,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if matches!(chars.get(index + 1), Some('>')) {
                    tokens.push(Token {
                        kind: TokenKind::DGreat,
                    });
                    index += 2;
                } else if matches!(chars.get(index + 1), Some('&')) {
                    tokens.push(Token {
                        kind: TokenKind::GreatAnd,
                    });
                    index += 2;
                } else if matches!(chars.get(index + 1), Some('|')) {
                    tokens.push(Token {
                        kind: TokenKind::Clobber,
                    });
                    index += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Greater,
                    });
                    index += 1;
                }
            }
            '$' if matches!(chars.get(index + 1), Some('(' | '{')) => {
                scan_dollar_construct(&chars, &mut index, &mut current)?;
            }
            '$' if matches!(chars.get(index + 1), Some('\'')) => {
                scan_dollar_single_quote(&chars, &mut index, &mut current)?;
            }
            '`' => {
                current.push('`');
                index += 1;
                while index < chars.len() {
                    let next = chars[index];
                    current.push(next);
                    if next == '\\' && index + 1 < chars.len() {
                        current.push(chars[index + 1]);
                        index += 2;
                        continue;
                    }
                    index += 1;
                    if next == '`' {
                        break;
                    }
                }
                if !current.ends_with('`') {
                    return Err(ParseError {
                        message: "unterminated backquote".to_string(),
                    });
                }
            }
            _ => {
                current.push(ch);
                index += 1;
            }
        }
    }

    flush_word(
        &mut current,
        &mut tokens,
        &mut expect_here_doc_target,
        &mut pending_here_docs,
    );
    if !pending_here_docs.is_empty() {
        return Err(ParseError {
            message: "unterminated here-document".to_string(),
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
    });
    Ok(Tokenized { tokens, here_docs })
}

fn flush_word(
    current: &mut String,
    tokens: &mut Vec<Token>,
    expect_here_doc_target: &mut bool,
    pending_here_docs: &mut VecDeque<(String, bool, bool)>,
) {
    if current.is_empty() {
        return;
    }
    let word = std::mem::take(current);
    if *expect_here_doc_target {
        let strip_tabs = matches!(
            tokens.last().map(|token| &token.kind),
            Some(TokenKind::DLessDash)
        );
        let (delimiter, expand) = parse_here_doc_delimiter(&word);
        pending_here_docs.push_back((delimiter, expand, strip_tabs));
        *expect_here_doc_target = false;
    }
    tokens.push(Token {
        kind: TokenKind::Word(word),
    });
}

fn parse_here_doc_delimiter(raw: &str) -> (String, bool) {
    let mut delimiter = String::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0usize;
    let mut expand = true;

    while index < chars.len() {
        match chars[index] {
            '\'' => {
                expand = false;
                index += 1;
                while index < chars.len() && chars[index] != '\'' {
                    delimiter.push(chars[index]);
                    index += 1;
                }
                if index < chars.len() {
                    index += 1;
                }
            }
            '"' => {
                expand = false;
                index += 1;
                while index < chars.len() && chars[index] != '"' {
                    delimiter.push(chars[index]);
                    index += 1;
                }
                if index < chars.len() {
                    index += 1;
                }
            }
            '\\' => {
                expand = false;
                index += 1;
                if index < chars.len() {
                    delimiter.push(chars[index]);
                    index += 1;
                }
            }
            ch => {
                delimiter.push(ch);
                index += 1;
            }
        }
    }

    (delimiter, expand)
}

fn read_here_doc_body(
    chars: &[char],
    index: &mut usize,
    delimiter: &str,
    strip_tabs: bool,
) -> Result<String, ParseError> {
    let mut body = String::new();

    loop {
        let line_start = *index;
        while *index < chars.len() && chars[*index] != '\n' {
            *index += 1;
        }
        let mut line: String = chars[line_start..*index].iter().collect();
        let had_newline = *index < chars.len() && chars[*index] == '\n';
        if had_newline {
            *index += 1;
        }

        if strip_tabs {
            line = line.trim_start_matches('\t').to_string();
        }

        if line == delimiter {
            return Ok(body);
        }

        body.push_str(&line);
        if had_newline {
            body.push('\n');
        } else {
            return Err(ParseError {
                message: "unterminated here-document".to_string(),
            });
        }
    }
}

struct Parser {
    tokens: Vec<Token>,
    here_docs: VecDeque<HereDoc>,
    aliases: HashMap<String, String>,
    alias_expand_next_word_at: Option<usize>,
    alias_expansions_remaining: usize,
    index: usize,
}

impl Parser {
    fn new(
        tokens: Vec<Token>,
        here_docs: VecDeque<HereDoc>,
        aliases: HashMap<String, String>,
    ) -> Self {
        Self {
            tokens,
            here_docs,
            aliases,
            alias_expand_next_word_at: None,
            alias_expansions_remaining: 1024,
            index: 0,
        }
    }

    fn parse_program_until(
        &mut self,
        stop_on_closer: bool,
        stop_words: &[&str],
        stop_on_dsemi: bool,
    ) -> Result<Program, ParseError> {
        let mut items = Vec::new();
        self.skip_separators();

        loop {
            self.expand_alias_at_command_start()?;
            if self.is_eof()
                || (stop_on_closer && self.at_closer())
                || self.at_reserved_stop(stop_words)
                || (stop_on_dsemi && matches!(self.peek_kind(), TokenKind::DSemi))
            {
                break;
            }
            let and_or = self.parse_and_or()?;
            let asynchronous = self.consume_amp();
            items.push(ListItem {
                and_or,
                asynchronous,
            });
            self.skip_separators();
        }

        Ok(Program { items })
    }

    fn parse_and_or(&mut self) -> Result<AndOr, ParseError> {
        let first = self.parse_pipeline()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek_kind() {
                TokenKind::AndIf => LogicalOp::And,
                TokenKind::OrIf => LogicalOp::Or,
                _ => break,
            };
            self.index += 1;
            self.skip_linebreaks();
            let rhs = self.parse_pipeline()?;
            rest.push((op, rhs));
        }
        Ok(AndOr { first, rest })
    }

    fn parse_pipeline(&mut self) -> Result<Pipeline, ParseError> {
        self.expand_alias_at_command_start()?;
        let negated = self.consume_bang();
        self.expand_alias_at_command_start()?;
        let mut commands = vec![self.parse_command()?];
        while matches!(self.peek_kind(), TokenKind::Pipe) {
            self.index += 1;
            self.skip_linebreaks();
            commands.push(self.parse_command()?);
        }
        Ok(Pipeline { negated, commands })
    }

    fn parse_command(&mut self) -> Result<Command, ParseError> {
        self.expand_alias_at_command_start()?;
        if self.peek_bang_word() {
            return Err(ParseError {
                message: "expected command".to_string(),
            });
        }
        let command = if let Some(function_name) = self.try_peek_function_name() {
            self.index += 3;
            let body = self.parse_command()?;
            Command::FunctionDef(FunctionDef {
                name: function_name,
                body: Box::new(body),
            })
        } else if self.peek_reserved_word("if") {
            self.parse_if_command()?
        } else if self.peek_reserved_word("while") {
            self.parse_loop_command(LoopKind::While)?
        } else if self.peek_reserved_word("until") {
            self.parse_loop_command(LoopKind::Until)?
        } else if self.peek_reserved_word("for") {
            self.parse_for_command()?
        } else if self.peek_reserved_word("case") {
            self.parse_case_command()?
        } else {
            match self.peek_kind() {
                TokenKind::LParen => {
                    self.index += 1;
                    let body = self.parse_program_until(true, &[], false)?;
                    self.expect(TokenKind::RParen, "expected ')' to close subshell")?;
                    Command::Subshell(body)
                }
                TokenKind::Word(text) if text == "{" => {
                    self.index += 1;
                    let body = self.parse_program_until(true, &[], false)?;
                    self.expect_reserved_word("}")?;
                    Command::Group(body)
                }
                _ => Command::Simple(self.parse_simple_command()?),
            }
        };
        self.parse_command_redirections(command)
    }

    fn parse_command_redirections(&mut self, command: Command) -> Result<Command, ParseError> {
        if matches!(command, Command::Simple(_)) {
            return Ok(command);
        }
        let mut redirections = Vec::new();
        while let Some(redirection) = self.try_parse_redirection()? {
            redirections.push(redirection);
        }
        if redirections.is_empty() {
            Ok(command)
        } else {
            Ok(Command::Redirected(Box::new(command), redirections))
        }
    }

    fn parse_if_command(&mut self) -> Result<Command, ParseError> {
        self.expect_reserved_word("if")?;
        let condition = self.parse_program_until(false, &["then"], false)?;
        if condition.items.is_empty() {
            return Err(ParseError {
                message: "expected command list after 'if'".to_string(),
            });
        }
        self.expect_reserved_word("then")?;
        let then_branch = self.parse_program_until(false, &["elif", "else", "fi"], false)?;
        let mut elif_branches = Vec::new();

        while self.peek_reserved_word("elif") {
            self.expect_reserved_word("elif")?;
            let condition = self.parse_program_until(false, &["then"], false)?;
            if condition.items.is_empty() {
                return Err(ParseError {
                    message: "expected command list after 'elif'".to_string(),
                });
            }
            self.expect_reserved_word("then")?;
            let body = self.parse_program_until(false, &["elif", "else", "fi"], false)?;
            elif_branches.push(ElifBranch { condition, body });
        }

        let else_branch = if self.peek_reserved_word("else") {
            self.expect_reserved_word("else")?;
            Some(self.parse_program_until(false, &["fi"], false)?)
        } else {
            None
        };

        self.expect_reserved_word("fi")?;
        Ok(Command::If(IfCommand {
            condition,
            then_branch,
            elif_branches,
            else_branch,
        }))
    }

    fn parse_loop_command(&mut self, kind: LoopKind) -> Result<Command, ParseError> {
        let keyword = match kind {
            LoopKind::While => "while",
            LoopKind::Until => "until",
        };
        self.expect_reserved_word(keyword)?;
        let condition = self.parse_program_until(false, &["do"], false)?;
        if condition.items.is_empty() {
            return Err(ParseError {
                message: format!("expected command list after '{keyword}'"),
            });
        }
        self.expect_reserved_word("do")?;
        let body = self.parse_program_until(false, &["done"], false)?;
        self.expect_reserved_word("done")?;
        Ok(Command::Loop(LoopCommand {
            kind,
            condition,
            body,
        }))
    }

    fn parse_for_command(&mut self) -> Result<Command, ParseError> {
        self.expect_reserved_word("for")?;
        let name = match self.peek_kind().clone() {
            TokenKind::Word(text) if is_name(&text) => {
                self.index += 1;
                text
            }
            _ => {
                return Err(ParseError {
                    message: "expected for loop variable name".to_string(),
                });
            }
        };

        self.skip_linebreaks();
        let items = if self.peek_reserved_word("in") {
            self.index += 1;
            let mut items = Vec::new();
            while !self.is_eof()
                && !matches!(self.peek_kind(), TokenKind::Semi | TokenKind::Newline)
            {
                match self.peek_kind().clone() {
                    TokenKind::Word(text) => {
                        self.index += 1;
                        items.push(Word { raw: text });
                    }
                    _ => {
                        return Err(ParseError {
                            message: "expected for loop word list".to_string(),
                        });
                    }
                }
            }
            Some(items)
        } else {
            None
        };

        self.skip_separators();
        self.expect_reserved_word("do")?;
        let body = self.parse_program_until(false, &["done"], false)?;
        self.expect_reserved_word("done")?;
        Ok(Command::For(ForCommand { name, items, body }))
    }

    fn parse_case_command(&mut self) -> Result<Command, ParseError> {
        self.expect_reserved_word("case")?;
        let word = match self.peek_kind().clone() {
            TokenKind::Word(text) => {
                self.index += 1;
                Word { raw: text }
            }
            _ => {
                return Err(ParseError {
                    message: "expected case word".to_string(),
                });
            }
        };
        self.skip_linebreaks();
        if self.peek_reserved_word("in") {
            self.index += 1;
        } else {
            return Err(ParseError {
                message: "expected 'in'".to_string(),
            });
        }
        self.skip_linebreaks();

        let mut arms = Vec::new();
        while !self.peek_reserved_word("esac") && !self.is_eof() {
            if matches!(self.peek_kind(), TokenKind::LParen) {
                self.index += 1;
            }

            let mut patterns = Vec::new();
            loop {
                match self.peek_kind().clone() {
                    TokenKind::Word(text) => {
                        self.index += 1;
                        patterns.push(Word { raw: text });
                    }
                    _ => {
                        return Err(ParseError {
                            message: "expected case pattern".to_string(),
                        });
                    }
                }

                if matches!(self.peek_kind(), TokenKind::Pipe) {
                    self.index += 1;
                    continue;
                }
                break;
            }

            self.expect(TokenKind::RParen, "expected ')' after case pattern")?;
            self.skip_separators();
            let body = self.parse_program_until(false, &["esac"], true)?;
            arms.push(CaseArm { patterns, body });

            if matches!(self.peek_kind(), TokenKind::DSemi) {
                self.index += 1;
                self.skip_separators();
            } else if !self.peek_reserved_word("esac") {
                return Err(ParseError {
                    message: "expected ';;' or 'esac'".to_string(),
                });
            }
        }

        self.expect_reserved_word("esac")?;
        Ok(Command::Case(CaseCommand { word, arms }))
    }

    fn parse_simple_command(&mut self) -> Result<SimpleCommand, ParseError> {
        let mut command = SimpleCommand::default();

        loop {
            self.expand_alias_after_blank_in_simple_command()?;
            if let Some(redirection) = self.try_parse_redirection()? {
                command.redirections.push(redirection);
                continue;
            }

            if command.words.is_empty() {
                if let Some(text) = self.peek_word_text() {
                    if split_assignment(&text).is_some() {
                        let (name, value) =
                            split_assignment(&text).unwrap();
                        self.index += 1;
                        command.assignments.push(Assignment {
                            name,
                            value: Word { raw: value },
                        });
                        continue;
                    }
                }
                if !command.assignments.is_empty() || !command.redirections.is_empty() {
                    self.expand_alias_at_command_start()?;
                }
            }

            let word = match self.peek_kind().clone() {
                TokenKind::Word(text) => {
                    self.index += 1;
                    Word { raw: text }
                }
                _ => break,
            };

            command.words.push(word);
        }

        if command.words.is_empty()
            && command.assignments.is_empty()
            && command.redirections.is_empty()
        {
            return Err(ParseError {
                message: "expected command".to_string(),
            });
        }

        if !command.words.is_empty() && matches!(self.peek_kind(), TokenKind::LParen) {
            return Err(ParseError {
                message: "syntax error near unexpected token `('".to_string(),
            });
        }

        Ok(command)
    }

    fn try_parse_redirection(&mut self) -> Result<Option<Redirection>, ParseError> {
        let (fd, kind) = match self.peek_kind().clone() {
            TokenKind::Word(text)
                if text.chars().all(|ch| ch.is_ascii_digit())
                    && self
                        .tokens
                        .get(self.index + 1)
                        .map(|token| {
                            matches!(
                                token.kind,
                                TokenKind::Less
                                    | TokenKind::Greater
                                    | TokenKind::DGreat
                                    | TokenKind::DLess
                                    | TokenKind::LessAnd
                                    | TokenKind::GreatAnd
                                    | TokenKind::LessGreat
                                    | TokenKind::Clobber
                            )
                        })
                        .unwrap_or(false) =>
            {
                let fd = text.parse::<i32>().map_err(|_| ParseError {
                    message: "invalid redirection file descriptor".to_string(),
                })?;
                let kind = self.redirection_kind_at(self.index + 1)?;
                self.index += 1;
                (Some(fd), kind)
            }
            _ => (None, self.redirection_kind_at(self.index)?),
        };
        if kind.is_none() {
            return Ok(None);
        }
        let kind = kind.expect("checked above");
        self.index += 1;

        let target = match self.peek_kind().clone() {
            TokenKind::Word(text) => {
                self.index += 1;
                Word { raw: text }
            }
            _ => {
                return Err(ParseError {
                    message: "expected redirection target".to_string(),
                });
            }
        };

        let here_doc = if kind == RedirectionKind::HereDoc {
            Some(self.here_docs.pop_front().ok_or_else(|| ParseError {
                message: "missing here-document body".to_string(),
            })?)
        } else {
            None
        };

        Ok(Some(Redirection {
            fd,
            kind,
            target,
            here_doc,
        }))
    }

    fn redirection_kind_at(&self, index: usize) -> Result<Option<RedirectionKind>, ParseError> {
        let kind = match self.tokens.get(index).map(|token| &token.kind) {
            Some(TokenKind::Less) => RedirectionKind::Read,
            Some(TokenKind::Greater) => RedirectionKind::Write,
            Some(TokenKind::Clobber) => RedirectionKind::ClobberWrite,
            Some(TokenKind::DGreat) => RedirectionKind::Append,
            Some(TokenKind::DLess | TokenKind::DLessDash) => RedirectionKind::HereDoc,
            Some(TokenKind::LessAnd) => RedirectionKind::DupInput,
            Some(TokenKind::GreatAnd) => RedirectionKind::DupOutput,
            Some(TokenKind::LessGreat) => RedirectionKind::ReadWrite,
            Some(_) => return Ok(None),
            None => {
                return Err(ParseError {
                    message: "unexpected end of tokens".to_string(),
                });
            }
        };
        Ok(Some(kind))
    }

    fn skip_separators(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Semi) {
            self.index += 1;
        }
    }

    fn skip_linebreaks(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline) {
            self.index += 1;
        }
    }

    fn consume_amp(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::Amp) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_bang(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::Word(text) if text == "!") {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: TokenKind, message: &str) -> Result<(), ParseError> {
        if std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(&expected) {
            self.index += 1;
            Ok(())
        } else {
            Err(ParseError {
                message: message.to_string(),
            })
        }
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.index].kind
    }

    fn peek_word_text(&self) -> Option<String> {
        match &self.tokens[self.index].kind {
            TokenKind::Word(text) => Some(text.clone()),
            _ => None,
        }
    }

    fn expand_alias_at_command_start(&mut self) -> Result<(), ParseError> {
        let _ = self.expand_alias_at_current_token()?;
        Ok(())
    }

    fn expand_alias_after_blank_in_simple_command(&mut self) -> Result<(), ParseError> {
        let Some(pending_index) = self.alias_expand_next_word_at else {
            return Ok(());
        };
        if self.index < pending_index {
            return Ok(());
        }
        self.alias_expand_next_word_at = None;
        if self.index != pending_index {
            return Ok(());
        }
        let _ = self.expand_alias_at_current_token()?;
        Ok(())
    }

    fn expand_alias_at_current_token(&mut self) -> Result<bool, ParseError> {
        let Some(TokenKind::Word(text)) = self.tokens.get(self.index).map(|token| &token.kind)
        else {
            return Ok(false);
        };
        if !is_alias_word(text) {
            return Ok(false);
        }
        let Some(replacement) = self.aliases.get(text).cloned() else {
            return Ok(false);
        };
        if self.alias_expansions_remaining == 0 {
            return Err(ParseError {
                message: "alias expansion too deep".to_string(),
            });
        };
        self.alias_expansions_remaining -= 1;
        let tokenized = tokenize(&replacement)?;
        let mut replacement_tokens = tokenized.tokens;
        if replacement_tokens
            .last()
            .is_some_and(|t| matches!(t.kind, TokenKind::Eof))
        {
            replacement_tokens.pop();
        }
        let inserted_len = replacement_tokens.len();
        self.tokens
            .splice(self.index..=self.index, replacement_tokens);
        if alias_has_trailing_blank(&replacement) {
            self.alias_expand_next_word_at = Some(self.index + inserted_len);
        }
        Ok(true)
    }

    fn try_peek_function_name(&self) -> Option<String> {
        let name = match self.tokens.get(self.index)?.kind.clone() {
            TokenKind::Word(name) => name,
            _ => return None,
        };
        if is_reserved_word(&name) {
            return None;
        }
        if !is_name(&name) {
            return None;
        }
        if !matches!(self.tokens.get(self.index + 1)?.kind, TokenKind::LParen) {
            return None;
        }
        if !matches!(self.tokens.get(self.index + 2)?.kind, TokenKind::RParen) {
            return None;
        }
        Some(name)
    }

    fn peek_reserved_word(&self, word: &str) -> bool {
        matches!(self.peek_kind(), TokenKind::Word(text) if text == word)
    }

    fn peek_bang_word(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Word(text) if text == "!")
    }

    fn at_reserved_stop(&self, stop_words: &[&str]) -> bool {
        match self.peek_kind() {
            TokenKind::Word(text) => stop_words.iter().any(|word| text == word),
            _ => false,
        }
    }

    fn expect_reserved_word(&mut self, word: &str) -> Result<(), ParseError> {
        if self.peek_reserved_word(word) {
            self.index += 1;
            self.skip_separators();
            Ok(())
        } else {
            Err(ParseError {
                message: format!("expected '{word}'"),
            })
        }
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn at_closer(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::RParen) || self.peek_reserved_word("}")
    }
}

fn split_assignment(input: &str) -> Option<(String, String)> {
    let (name, value) = input.split_once('=')?;
    if !is_name(name) {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

fn is_alias_word(word: &str) -> bool {
    !word.is_empty() && !word.chars().any(|ch| matches!(ch, '\'' | '"' | '\\'))
}

fn alias_has_trailing_blank(alias: &str) -> bool {
    matches!(alias.chars().last(), Some(' ' | '\t'))
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    !chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
}

fn is_reserved_word(word: &str) -> bool {
    matches!(
        word,
        "if" | "then"
            | "else"
            | "elif"
            | "fi"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "while"
            | "until"
            | "for"
            | "in"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_pipeline() {
        let program = parse("echo hi | wc -c").expect("parse");
        assert_eq!(program.items.len(), 1);
        assert_eq!(program.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn parses_assignments_and_groups() {
        let program = parse("FOO=bar echo \"$FOO\"").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.assignments.len() == 1 && cmd.words[0].raw == "echo"
        ));
    }

    #[test]
    fn parses_logical_and_subshell_forms() {
        let program = parse("(echo ok) && echo done || echo fail").expect("parse");
        let and_or = &program.items[0].and_or;
        assert_eq!(and_or.rest.len(), 2);
        assert!(matches!(
            and_or.first.commands.first(),
            Some(Command::Subshell(_))
        ));

        let linebreak_and_or =
            parse("true &&\n echo done ||\n echo fail").expect("parse linebreak and-or");
        assert_eq!(linebreak_and_or.items[0].and_or.rest.len(), 2);
    }

    #[test]
    fn tokenizes_terminated_single_quotes() {
        let program = parse("echo 'ok'").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && cmd.words[1].raw == "'ok'"
        ));
    }

    #[test]
    fn parses_case_arm_without_trailing_dsemi_before_esac() {
        let program = parse("case x in x) esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_cmd) if case_cmd.arms.len() == 1
        ));
    }

    #[test]
    fn parses_heredoc_operator_shape() {
        let program = parse("cat <<EOF\nhello $USER\nEOF\n").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 1
                    && cmd.redirections[0].kind == RedirectionKind::HereDoc
                    && cmd.redirections[0].target.raw == "EOF"
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body.as_str()) == Some("hello $USER\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.delimiter.as_str()) == Some("EOF")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body.as_str()) == Some("one\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.strip_tabs) == Some(true)
        ));
    }

    #[test]
    fn parses_extended_redirection_forms() {
        let program = parse("cat 3<in 2>out 4>>log 5<>rw 0<&3 1>&2 2>|force").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 7
                    && cmd.redirections[0].fd == Some(3)
                    && cmd.redirections[0].kind == RedirectionKind::Read
                    && cmd.redirections[1].fd == Some(2)
                    && cmd.redirections[1].kind == RedirectionKind::Write
                    && cmd.redirections[2].fd == Some(4)
                    && cmd.redirections[2].kind == RedirectionKind::Append
                    && cmd.redirections[3].fd == Some(5)
                    && cmd.redirections[3].kind == RedirectionKind::ReadWrite
                    && cmd.redirections[4].kind == RedirectionKind::DupInput
                    && cmd.redirections[5].kind == RedirectionKind::DupOutput
                    && cmd.redirections[6].kind == RedirectionKind::ClobberWrite
        ));
    }

    #[test]
    fn parses_redirections_on_compound_commands() {
        let program = parse("{ echo hi; } >out").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Redirected(inner, redirections)
                if matches!(inner.as_ref(), Command::Group(_))
                    && redirections.len() == 1
                    && redirections[0].kind == RedirectionKind::Write
                    && redirections[0].target.raw == "out"
        ));

        let not_a_group = parse("{echo hi; }").expect("parse brace word");
        assert!(matches!(
            &not_a_group.items[0].and_or.first.commands[0],
            Command::Simple(simple) if simple.words[0].raw == "{echo"
        ));

        let closer_literal = parse("echo }").expect("parse literal closer");
        assert!(matches!(
            &closer_literal.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["echo", "}"]
        ));
    }

    #[test]
    fn parses_function_definition() {
        let program = parse("greet() { echo hi; }").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(function) if function.name == "greet"
        ));
        assert!(parse("if() { echo hi; }").is_err());
    }

    #[test]
    fn parses_negated_async_pipeline() {
        let program = parse("! echo ok | wc -c &").expect("parse");
        let item = &program.items[0];
        assert!(item.asynchronous);
        assert!(item.and_or.first.negated);
        assert_eq!(item.and_or.first.commands.len(), 2);

        let linebreak_pipeline = parse("printf ok |\n wc -c").expect("parse linebreak pipeline");
        assert_eq!(linebreak_pipeline.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn rejects_invalid_empty_command() {
        let error = parse("| wc").expect_err("parse should fail");
        assert_eq!(error.message, "expected command");

        let error = parse("echo hi | ! cat").expect_err("bang after pipe should fail");
        assert_eq!(error.message, "expected command");
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = parse("echo 'unterminated").expect_err("parse should fail");
        assert_eq!(error.message, "unterminated single quote");
    }

    #[test]
    fn rejects_unterminated_dollar_single_quote() {
        let error = parse("echo $'unterminated").expect_err("parse should fail");
        assert_eq!(error.message, "unterminated dollar-single-quotes");
        let error = parse(r"echo $'backslash at end\").expect_err("parse should fail");
        assert_eq!(error.message, "unterminated dollar-single-quotes");
    }

    #[test]
    fn parses_if_with_elif_and_else() {
        let program =
            parse("if true; then echo yes; elif false; then echo no; else echo maybe; fi")
                .expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(if_command)
                if if_command.elif_branches.len() == 1 && if_command.else_branch.is_some()
        ));

        let simple_if = parse("if true; then echo yes; fi").expect("parse");
        assert!(matches!(
            &simple_if.items[0].and_or.first.commands[0],
            Command::If(if_command) if if_command.else_branch.is_none()
        ));
    }

    #[test]
    fn parses_while_and_until_loops() {
        let while_program = parse("while true; do echo yes; done").expect("parse");
        assert!(matches!(
            while_program.items[0].and_or.first.commands[0],
            Command::Loop(LoopCommand {
                kind: LoopKind::While,
                ..
            })
        ));

        let until_program = parse("until false; do echo yes; done").expect("parse");
        assert!(matches!(
            until_program.items[0].and_or.first.commands[0],
            Command::Loop(LoopCommand {
                kind: LoopKind::Until,
                ..
            })
        ));
    }

    #[test]
    fn parses_for_loops() {
        let explicit = parse("for item in a b c; do echo $item; done").expect("parse");
        assert!(matches!(
            &explicit.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.name == "item" && for_command.items.as_ref().map(Vec::len) == Some(3)
        ));

        let positional = parse("for item; do echo $item; done").expect("parse");
        assert!(matches!(
            &positional.items[0].and_or.first.commands[0],
            Command::For(for_command) if for_command.name == "item" && for_command.items.is_none()
        ));

        let linebreak_before_in =
            parse("for item\nin a b; do echo $item; done").expect("parse linebreak before in");
        assert!(matches!(
            &linebreak_before_in.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.name == "item"
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>())
                        == Some(vec!["a", "b"])
        ));

        let reserved_words_as_items = parse("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>())
                    == Some(vec!["do", "done"])
        ));
    }

    #[test]
    fn parses_case_commands() {
        let program =
            parse("case $name in foo|bar) echo hit ;; baz) echo miss ;; esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_command)
                if case_command.word.raw == "$name"
                    && case_command.arms.len() == 2
                    && case_command.arms[0].patterns.len() == 2
        ));

        let with_optional_paren = parse("case x in (x) echo ok ;; esac").expect("parse");
        assert!(matches!(
            &with_optional_paren.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let with_linebreak_before_in =
            parse("case x\nin\nx) echo ok ;;\nesac").expect("parse case linebreak");
        assert!(matches!(
            &with_linebreak_before_in.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let empty_after_linebreak =
            parse("case x\nin\nesac").expect("parse empty case after linebreak");
        assert!(matches!(
            &empty_after_linebreak.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.is_empty()
        ));
    }

    #[test]
    fn parser_covers_misc_error_and_token_paths() {
        assert_eq!(
            format!(
                "{}",
                ParseError {
                    message: "x".into()
                }
            ),
            "x"
        );
        assert!(parse("echo \"unterminated").is_err());
        assert!(parse("cat <").is_err());
        assert!(parse("for 1 in a; do echo hi; done").is_err());
        assert!(parse("for item in ; do echo hi; done").is_ok());
        assert!(parse("for item in ) ; do echo hi; done").is_err());
        assert!(parse("case x in ; esac").is_err());
        assert!(parse("case name in foo echo hi esac").is_err());
        assert!(parse("cat <<EOF").is_err());
        assert!(parse("echo 2>&").is_err());
        assert!(parse("if true; echo hi; fi").is_err());
        assert!(parse("while true; echo hi; done").is_err());
        assert!(parse("# comment only\n").is_ok());
        assert!(parse("echo foo\\ bar").is_ok());
        assert!(parse("echo \"a\\\"b\"").is_ok());
        assert!(parse("printf ok |\n wc -c").is_ok());
        assert!(parse("true &&\n echo ok").is_ok());
        assert!(parse("false ||\n echo ok").is_ok());
    }

    #[test]
    fn parser_private_helpers_cover_remaining_branches() {
        let tokenized = tokenize("echo hi").expect("tokenize");
        let mut parser = Parser::new(tokenized.tokens.clone(), VecDeque::new(), HashMap::new());
        assert!(!parser.peek_reserved_word("if"));
        assert!(!parser.at_reserved_stop(&["then"]));
        assert!(parser.expect_reserved_word("if").is_err());
        assert!(parser.expect(TokenKind::Semi, "semi").is_err());

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Newline,
                },
                Token {
                    kind: TokenKind::Word("do".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        parser.skip_linebreaks();
        assert!(parser.peek_reserved_word("do"));

        let func_tokens = tokenize("name( x").expect("tokenize");
        let parser = Parser::new(func_tokens.tokens, VecDeque::new(), HashMap::new());
        assert_eq!(parser.try_peek_function_name(), None);

        let closer_tokens = vec![Token {
            kind: TokenKind::Word("}".into()),
        }];
        let parser = Parser::new(closer_tokens, VecDeque::new(), HashMap::new());
        assert!(parser.at_closer());

        assert_eq!(
            split_assignment("NAME=value"),
            Some(("NAME".into(), "value".into()))
        );
        assert_eq!(split_assignment("1NAME=value"), None);
        assert!(is_alias_word("alias_name"));
        assert!(!is_alias_word("'alias'"));
        assert!(is_reserved_word("if"));
        assert!(!is_reserved_word("name"));
        assert!(!is_name(""));
        assert!(!is_name("1abc"));
    }

    #[test]
    fn alias_helper_paths_cover_pending_and_depth_guard() {
        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("word".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::from([(String::from("word"), String::from("ok"))]),
        );
        parser.alias_expand_next_word_at = Some(0);
        parser
            .expand_alias_after_blank_in_simple_command()
            .expect("expand pending alias");
        assert!(matches!(parser.peek_kind(), TokenKind::Word(text) if text == "ok"));

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("loop".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::from([(String::from("loop"), String::from("loop "))]),
        );
        parser.alias_expansions_remaining = 0;
        let error = parser
            .expand_alias_at_current_token()
            .expect_err("depth guard");
        assert_eq!(error.message, "alias expansion too deep");

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("tail".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        parser.index = 1;
        parser.alias_expand_next_word_at = Some(0);
        parser
            .expand_alias_after_blank_in_simple_command()
            .expect("clear stale alias marker");
        assert_eq!(parser.alias_expand_next_word_at, None);
    }

    #[test]
    fn parse_session_uses_updated_aliases_between_items() {
        let mut session = ParseSession::new("alias setok='printf ok'; setok").expect("session");
        let first = session
            .next_item(&HashMap::new())
            .expect("first item")
            .expect("some item");
        assert!(matches!(first.and_or.first.commands[0], Command::Simple(_)));

        let second = session
            .next_item(&HashMap::from([(
                String::from("setok"),
                String::from("printf ok"),
            )]))
            .expect("second item")
            .expect("some item");
        assert!(matches!(
            &second.and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["printf", "ok"]
        ));

        assert!(session.next_item(&HashMap::new()).expect("eof").is_none());
    }

    #[test]
    fn alias_expansion_in_simple_commands() {
        let mut aliases = HashMap::new();
        aliases.insert("say".to_string(), "printf hi".to_string());
        let program = parse_with_aliases("say", &aliases).expect("parse alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["printf", "hi"]
        ));

        let mut aliases = HashMap::new();
        aliases.insert("cond".to_string(), "if".to_string());
        let program = parse_with_aliases("cond true; then echo ok; fi", &aliases)
            .expect("parse reserved alias");
        assert!(matches!(
            program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn standalone_bang_is_context_sensitive() {
        let program = parse("echo !").expect("parse echo bang");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["echo", "!"]
        ));

        let program = parse("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["!true"]
        ));

        let program = parse("! true").expect("parse negation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn trailing_blank_aliases_expand_next_simple_command_word() {
        let mut aliases = HashMap::new();
        aliases.insert("say".to_string(), "printf %s ".to_string());
        aliases.insert("word".to_string(), "ok".to_string());
        let program = parse_with_aliases("say word", &aliases).expect("parse chained alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["printf", "%s", "ok"]
        ));
    }

    #[test]
    fn self_referential_aliases_do_not_loop_indefinitely() {
        let mut aliases = HashMap::new();
        aliases.insert("loop".to_string(), "loop ".to_string());
        let program = parse_with_aliases("loop ok", &aliases).expect("self alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_str()).collect::<Vec<_>>() == vec!["loop", "ok"]
        ));
        assert!(alias_has_trailing_blank("value "));
        assert!(!alias_has_trailing_blank("value"));
    }

    #[test]
    fn alias_expansion_after_assignment_and_redirection() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".to_string(), "echo aliased".to_string());
        let program =
            parse_with_aliases("VAR=value foo", &aliases).expect("alias after assignment");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| w.raw.as_str()).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.assignments.len() == 1
        ));

        let program = parse_with_aliases("</dev/null foo", &aliases)
            .expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| w.raw.as_str()).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.redirections.len() == 1
        ));
    }

    #[test]
    fn lparen_after_simple_command_is_syntax_error() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".to_string(), "echo aliased".to_string());
        let err = parse_with_aliases("foo () { true; }", &aliases).unwrap_err();
        assert!(
            err.message.contains("("),
            "error should mention '(': {}",
            err.message
        );
    }

    #[test]
    fn parse_case_command_error_paths_are_covered() {
        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("case".into()),
                },
                Token {
                    kind: TokenKind::Semi,
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser
                .parse_case_command()
                .expect_err("missing word")
                .message,
            "expected case word"
        );

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("case".into()),
                },
                Token {
                    kind: TokenKind::Word("name".into()),
                },
                Token {
                    kind: TokenKind::Newline,
                },
                Token {
                    kind: TokenKind::Word("esac".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser.parse_case_command().expect_err("missing in").message,
            "expected 'in'"
        );

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("case".into()),
                },
                Token {
                    kind: TokenKind::Word("name".into()),
                },
                Token {
                    kind: TokenKind::Word("in".into()),
                },
                Token {
                    kind: TokenKind::RParen,
                },
                Token {
                    kind: TokenKind::Word("esac".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser
                .parse_case_command()
                .expect_err("missing pattern")
                .message,
            "expected case pattern"
        );

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("case".into()),
                },
                Token {
                    kind: TokenKind::Word("name".into()),
                },
                Token {
                    kind: TokenKind::Word("in".into()),
                },
                Token {
                    kind: TokenKind::Word("foo".into()),
                },
                Token {
                    kind: TokenKind::RParen,
                },
                Token {
                    kind: TokenKind::Word("echo".into()),
                },
                Token {
                    kind: TokenKind::Word("hi".into()),
                },
                Token {
                    kind: TokenKind::Semi,
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser
                .parse_case_command()
                .expect_err("missing terminator")
                .message,
            "expected ';;' or 'esac'"
        );
    }

    #[test]
    fn heredoc_private_helpers_cover_remaining_paths() {
        assert_eq!(parse_here_doc_delimiter("\"EOF\""), ("EOF".into(), false));
        assert_eq!(parse_here_doc_delimiter("\\EOF"), ("EOF".into(), false));

        let chars: Vec<char> = "line".chars().collect();
        let mut index = 0usize;
        assert_eq!(
            read_here_doc_body(&chars, &mut index, "EOF", false)
                .expect_err("unterminated body")
                .message,
            "unterminated here-document"
        );

        let tokenized = tokenize("cat <<A <<'B'\nfirst\nA\nsecond\nB\n").expect("tokenize");
        assert_eq!(tokenized.here_docs.len(), 2);
        assert_eq!(tokenized.here_docs[0].body, "first\n");
        assert_eq!(tokenized.here_docs[1].body, "second\n");

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::DLess,
                },
                Token {
                    kind: TokenKind::Word("EOF".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser
                .try_parse_redirection()
                .expect_err("missing body")
                .message,
            "missing here-document body"
        );

        let mut parser = Parser::new(
            vec![
                Token {
                    kind: TokenKind::Word("999999999999999999999".into()),
                },
                Token {
                    kind: TokenKind::Greater,
                },
                Token {
                    kind: TokenKind::Word("out".into()),
                },
                Token {
                    kind: TokenKind::Eof,
                },
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        assert_eq!(
            parser
                .try_parse_redirection()
                .expect_err("invalid fd")
                .message,
            "invalid redirection file descriptor"
        );

        let parser = Parser::new(
            vec![Token {
                kind: TokenKind::Eof,
            }],
            VecDeque::new(),
            HashMap::new(),
        );
        assert!(parser.redirection_kind_at(99).is_err());
    }

    #[test]
    fn tokenizer_keeps_dollar_paren_as_single_word() {
        let program = parse("echo $(cmd arg)").expect("parse dollar paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "$(cmd arg)"
        ));
    }

    #[test]
    fn tokenizer_keeps_dollar_double_paren_as_single_word() {
        let program = parse("echo $((1 + 2))").expect("parse dollar arith");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "$((1 + 2))"
        ));

        let nested = parse("echo $((1 + (2 * 3)))").expect("parse nested arith");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words[1].raw == "$((1 + (2 * 3)))"
        ));

        let error = parse("echo $((1 + 2").expect_err("unterminated arith");
        assert_eq!(error.message, "unterminated arithmetic expansion");
    }

    #[test]
    fn tokenizer_keeps_dollar_brace_as_single_word() {
        let program = parse("echo ${VAR:-default}").expect("parse dollar brace");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "${VAR:-default}"
        ));

        let nested = parse("echo ${VAR:-${INNER}}").expect("parse nested brace");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-${INNER}}"
        ));
    }

    #[test]
    fn tokenizer_keeps_backtick_as_single_word() {
        let program = parse("echo `cmd arg`").expect("parse backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "`cmd arg`"
        ));

        let error = parse("echo `unterminated").expect_err("unterminated backtick");
        assert_eq!(error.message, "unterminated backquote");
    }

    #[test]
    fn tokenizer_backtick_inside_double_quotes() {
        let program = parse("echo \"`cmd`\"").expect("parse dquote backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words[1].raw == "\"`cmd`\""
        ));
    }

    #[test]
    fn tokenizer_dollar_paren_inside_double_quotes() {
        let program = parse("echo \"$(cmd arg)\"").expect("parse dquote dollar paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words[1].raw == "\"$(cmd arg)\""
        ));
    }

    #[test]
    fn tokenizer_dollar_brace_inside_double_quotes() {
        let program = parse("echo \"${VAR:-val}\"").expect("parse dquote dollar brace");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words[1].raw == "\"${VAR:-val}\""
        ));
    }

    #[test]
    fn tokenizer_nested_constructs_in_paren_body() {
        let program = parse("echo $(echo 'hi' \"$VAR\" \\x `inner` ${B} $((1+1)))")
            .expect("parse nested paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2
        ));
    }

    #[test]
    fn tokenizer_nested_constructs_in_brace_body() {
        let program = parse("echo ${VAR:-'a}b'}").expect("parse brace sq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-'a}b'}"
        ));

        let program = parse("echo ${VAR:-\"a}b\"}").expect("parse brace dq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-\"a}b\"}"
        ));

        let program = parse("echo ${VAR:-\\}}").expect("parse brace escaped");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-\\}}"
        ));

        let program = parse("echo ${VAR:-`cmd`}").expect("parse brace backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-`cmd`}"
        ));

        let program = parse("echo ${VAR:-$(cmd)}").expect("parse brace cmdsubst");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-$(cmd)}"
        ));

        let error = parse("echo ${VAR:-unclosed").expect_err("unterminated brace body");
        assert_eq!(error.message, "unterminated parameter expansion");

        let error = parse("echo $(unclosed").expect_err("unterminated paren body");
        assert_eq!(error.message, "unterminated command substitution");
    }

    #[test]
    fn tokenizer_backtick_with_escape() {
        let program = parse("echo `echo \\$VAR`").expect("parse bt escape");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "`echo \\$VAR`"
        ));
    }

    #[test]
    fn tokenizer_dollar_brace_from_toplevel() {
        let program = parse("echo ${VAR}done").expect("parse brace toplevel");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR}done"
        ));
    }

    #[test]
    fn tokenizer_nested_paren_depth() {
        let program = parse("echo $(echo (a) (b))").expect("parse nested parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "$(echo (a) (b))"
        ));
    }

    #[test]
    fn tokenizer_backtick_body_escape() {
        let program = parse("echo ${VAR:-`echo \\`inner\\``}").expect("parse bt body escape");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn tokenizer_backtick_escape_in_dquote() {
        let program = parse("echo \"`echo \\$X`\"").expect("parse dq bt escape");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "\"`echo \\$X`\""
        ));
    }

    #[test]
    fn tokenizer_unterminated_backtick_in_brace() {
        let error = parse("echo ${VAR:-`unterminated}").expect_err("unterminated bt in brace");
        assert_eq!(error.message, "unterminated backquote");
    }

    #[test]
    fn backslash_newline_continuation() {
        let program = parse("echo hel\\\nlo").expect("parse continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && cmd.words[1].raw == "hello"
        ));
    }

    #[test]
    fn if_empty_condition_is_parse_error() {
        let error = parse("if then fi").expect_err("empty if condition");
        assert!(error.message.contains("expected command list after 'if'"));
    }

    #[test]
    fn elif_empty_condition_is_parse_error() {
        let error =
            parse("if true; then true; elif then true; fi").expect_err("empty elif condition");
        assert!(error.message.contains("expected command list after 'elif'"));
    }

    #[test]
    fn while_empty_condition_is_parse_error() {
        let error = parse("while do true; done").expect_err("empty while condition");
        assert!(error.message.contains("expected command list after 'while'"));
    }

    #[test]
    fn until_empty_condition_is_parse_error() {
        let error = parse("until do true; done").expect_err("empty until condition");
        assert!(error.message.contains("expected command list after 'until'"));
    }
}
