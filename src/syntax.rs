use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program<'src> {
    pub items: Vec<ListItem<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem<'src> {
    pub and_or: AndOr<'src>,
    pub asynchronous: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndOr<'src> {
    pub first: Pipeline<'src>,
    pub rest: Vec<(LogicalOp, Pipeline<'src>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedMode {
    Off,
    Default,
    Posix,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pipeline<'src> {
    pub negated: bool,
    pub timed: TimedMode,
    pub commands: Vec<Command<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command<'src> {
    Simple(SimpleCommand<'src>),
    Subshell(Program<'src>),
    Group(Program<'src>),
    FunctionDef(FunctionDef<'src>),
    If(IfCommand<'src>),
    Loop(LoopCommand<'src>),
    For(ForCommand<'src>),
    Case(CaseCommand<'src>),
    Redirected(Box<Command<'src>>, Vec<Redirection<'src>>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCommand<'src> {
    pub assignments: Vec<Assignment<'src>>,
    pub words: Vec<Word<'src>>,
    pub redirections: Vec<Redirection<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment<'src> {
    pub name: Cow<'src, str>,
    pub value: Word<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word<'src> {
    pub raw: Cow<'src, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirection<'src> {
    pub fd: Option<i32>,
    pub kind: RedirectionKind,
    pub target: Word<'src>,
    pub here_doc: Option<HereDoc<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionDef<'src> {
    pub name: Cow<'src, str>,
    pub body: Box<Command<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfCommand<'src> {
    pub condition: Program<'src>,
    pub then_branch: Program<'src>,
    pub elif_branches: Vec<ElifBranch<'src>>,
    pub else_branch: Option<Program<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElifBranch<'src> {
    pub condition: Program<'src>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopCommand<'src> {
    pub kind: LoopKind,
    pub condition: Program<'src>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForCommand<'src> {
    pub name: Cow<'src, str>,
    pub items: Option<Vec<Word<'src>>>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCommand<'src> {
    pub word: Word<'src>,
    pub arms: Vec<CaseArm<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseArm<'src> {
    pub patterns: Vec<Word<'src>>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HereDoc<'src> {
    pub delimiter: Cow<'src, str>,
    pub body: Cow<'src, str>,
    pub expand: bool,
    pub strip_tabs: bool,
}

impl<'src> Command<'src> {
    pub fn into_static(self) -> Command<'static> {
        match self {
            Command::Simple(cmd) => Command::Simple(SimpleCommand {
                assignments: cmd
                    .assignments
                    .into_iter()
                    .map(|a| Assignment {
                        name: Cow::Owned(a.name.into_owned()),
                        value: Word {
                            raw: Cow::Owned(a.value.raw.into_owned()),
                        },
                    })
                    .collect(),
                words: cmd
                    .words
                    .into_iter()
                    .map(|w| Word {
                        raw: Cow::Owned(w.raw.into_owned()),
                    })
                    .collect(),
                redirections: cmd.redirections.into_iter().map(redir_static).collect(),
            }),
            Command::Subshell(p) => Command::Subshell(program_static(p)),
            Command::Group(p) => Command::Group(program_static(p)),
            Command::FunctionDef(f) => Command::FunctionDef(FunctionDef {
                name: Cow::Owned(f.name.into_owned()),
                body: Box::new(f.body.into_static()),
            }),
            Command::If(c) => Command::If(IfCommand {
                condition: program_static(c.condition),
                then_branch: program_static(c.then_branch),
                elif_branches: c
                    .elif_branches
                    .into_iter()
                    .map(|b| ElifBranch {
                        condition: program_static(b.condition),
                        body: program_static(b.body),
                    })
                    .collect(),
                else_branch: c.else_branch.map(program_static),
            }),
            Command::Loop(c) => Command::Loop(LoopCommand {
                kind: c.kind,
                condition: program_static(c.condition),
                body: program_static(c.body),
            }),
            Command::For(c) => Command::For(ForCommand {
                name: Cow::Owned(c.name.into_owned()),
                items: c.items.map(|items| {
                    items
                        .into_iter()
                        .map(|w| Word {
                            raw: Cow::Owned(w.raw.into_owned()),
                        })
                        .collect()
                }),
                body: program_static(c.body),
            }),
            Command::Case(c) => Command::Case(CaseCommand {
                word: Word {
                    raw: Cow::Owned(c.word.raw.into_owned()),
                },
                arms: c
                    .arms
                    .into_iter()
                    .map(|arm| CaseArm {
                        patterns: arm
                            .patterns
                            .into_iter()
                            .map(|w| Word {
                                raw: Cow::Owned(w.raw.into_owned()),
                            })
                            .collect(),
                        body: program_static(arm.body),
                    })
                    .collect(),
            }),
            Command::Redirected(cmd, redirs) => Command::Redirected(
                Box::new(cmd.into_static()),
                redirs.into_iter().map(redir_static).collect(),
            ),
        }
    }
}

fn program_static(p: Program<'_>) -> Program<'static> {
    Program {
        items: p
            .items
            .into_iter()
            .map(|item| ListItem {
                and_or: AndOr {
                    first: pipeline_static(item.and_or.first),
                    rest: item
                        .and_or
                        .rest
                        .into_iter()
                        .map(|(op, pl)| (op, pipeline_static(pl)))
                        .collect(),
                },
                asynchronous: item.asynchronous,
            })
            .collect(),
    }
}

fn pipeline_static(p: Pipeline<'_>) -> Pipeline<'static> {
    Pipeline {
        negated: p.negated,
        timed: p.timed,
        commands: p.commands.into_iter().map(|c| c.into_static()).collect(),
    }
}

fn redir_static(r: Redirection<'_>) -> Redirection<'static> {
    Redirection {
        fd: r.fd,
        kind: r.kind,
        target: Word {
            raw: Cow::Owned(r.target.raw.into_owned()),
        },
        here_doc: r.here_doc.map(|hd| HereDoc {
            delimiter: Cow::Owned(hd.delimiter.into_owned()),
            body: Cow::Owned(hd.body.into_owned()),
            expand: hd.expand,
            strip_tabs: hd.strip_tabs,
        }),
    }
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
enum Token<'src> {
    Word(Cow<'src, str>),
    Newline,
    Semi,
    DSemi,
    Amp,
    Pipe,
    AndIf,
    OrIf,
    LParen,
    RParen,
    IoNumber(i32),
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

struct Tokenized<'src> {
    tokens: Vec<Token<'src>>,
    here_docs: VecDeque<HereDoc<'src>>,
}

pub fn parse(source: &str) -> Result<Program<'_>, ParseError> {
    parse_with_aliases(source, &HashMap::new())
}

pub fn parse_with_aliases<'src>(
    source: &'src str,
    aliases: &HashMap<String, String>,
) -> Result<Program<'src>, ParseError> {
    let tokenized = tokenize(source)?;
    Parser::new(tokenized.tokens, tokenized.here_docs, aliases.clone()).parse_program_until(
        false,
        &[],
        false,
    )
}

pub struct ParseSession<'src> {
    parser: Parser<'src>,
}

impl<'src> ParseSession<'src> {
    pub fn new(source: &'src str) -> Result<Self, ParseError> {
        let tokenized = tokenize(source)?;
        Ok(Self {
            parser: Parser::new(tokenized.tokens, tokenized.here_docs, HashMap::new()),
        })
    }

    pub fn next_item(
        &mut self,
        aliases: &HashMap<String, String>,
    ) -> Result<Option<ListItem<'src>>, ParseError> {
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

    pub fn current_line(&self) -> usize {
        self.parser.tokens[..self.parser.index]
            .iter()
            .filter(|t| matches!(t, Token::Newline))
            .count()
            + 1
    }
}

fn next_char(source: &str, index: usize) -> Option<char> {
    source[index..].chars().next()
}

fn peek_byte(source: &str, index: usize) -> Option<u8> {
    source.as_bytes().get(index).copied()
}

fn char_len_at(source: &str, index: usize) -> usize {
    next_char(source, index).map_or(0, |ch| ch.len_utf8())
}

fn skip_scan(source: &str, index: &mut usize) -> Result<(), ParseError> {
    let ch = next_char(source, *index).unwrap();
    match ch {
        '\'' => {
            *index += 1;
            while *index < source.len() {
                let c = next_char(source, *index).unwrap();
                *index += c.len_utf8();
                if c == '\'' {
                    return Ok(());
                }
            }
            Err(ParseError {
                message: "unterminated single quote".to_string(),
            })
        }
        '"' => {
            *index += 1;
            skip_dquote_body(source, index)
        }
        '\\' => {
            *index += 1;
            if *index < source.len() {
                *index += char_len_at(source, *index);
            }
            Ok(())
        }
        '$' if matches!(peek_byte(source, *index + 1), Some(b'(' | b'{')) => {
            skip_dollar_construct(source, index)
        }
        '$' if peek_byte(source, *index + 1) == Some(b'\'') => {
            skip_dollar_single_quote(source, index)
        }
        '`' => skip_backtick_body(source, index),
        _ => {
            *index += ch.len_utf8();
            Ok(())
        }
    }
}

fn skip_dollar_single_quote(source: &str, index: &mut usize) -> Result<(), ParseError> {
    *index += 2;
    while *index < source.len() {
        let ch = next_char(source, *index).unwrap();
        if ch == '\'' {
            *index += 1;
            return Ok(());
        }
        if ch == '\\' && *index + 1 < source.len() {
            *index += 1;
            *index += char_len_at(source, *index);
            continue;
        }
        *index += ch.len_utf8();
    }
    Err(ParseError {
        message: "unterminated dollar-single-quotes".to_string(),
    })
}

fn skip_dollar_construct(source: &str, index: &mut usize) -> Result<(), ParseError> {
    let next = source.as_bytes()[*index + 1];
    if next == b'(' {
        if source.as_bytes().get(*index + 2) == Some(&b'(') {
            *index += 3;
            let mut depth = 1usize;
            while *index < source.len() {
                let ch = next_char(source, *index).unwrap();
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    if depth == 1 && source.as_bytes().get(*index + 1) == Some(&b')') {
                        *index += 2;
                        return Ok(());
                    }
                    depth = depth.saturating_sub(1);
                }
                *index += ch.len_utf8();
            }
            return Err(ParseError {
                message: "unterminated arithmetic expansion".to_string(),
            });
        }
        *index += 2;
        return skip_paren_body(source, index);
    }
    *index += 2;
    skip_brace_body(source, index)
}

fn skip_paren_body(source: &str, index: &mut usize) -> Result<(), ParseError> {
    let mut depth = 1usize;
    while *index < source.len() {
        let ch = next_char(source, *index).unwrap();
        match ch {
            '(' => {
                depth += 1;
                *index += 1;
            }
            ')' => {
                depth -= 1;
                *index += 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            '\'' | '"' | '\\' | '$' | '`' => {
                skip_scan(source, index)?;
            }
            _ => {
                *index += ch.len_utf8();
            }
        }
    }
    Err(ParseError {
        message: "unterminated command substitution".to_string(),
    })
}

fn skip_brace_body(source: &str, index: &mut usize) -> Result<(), ParseError> {
    while *index < source.len() {
        let ch = next_char(source, *index).unwrap();
        match ch {
            '}' => {
                *index += 1;
                return Ok(());
            }
            '\'' | '"' | '\\' | '$' | '`' => {
                skip_scan(source, index)?;
            }
            _ => {
                *index += ch.len_utf8();
            }
        }
    }
    Err(ParseError {
        message: "unterminated parameter expansion".to_string(),
    })
}

fn skip_backtick_body(source: &str, index: &mut usize) -> Result<(), ParseError> {
    *index += 1;
    while *index < source.len() {
        let ch = next_char(source, *index).unwrap();
        if ch == '\\' && *index + 1 < source.len() {
            *index += 1;
            *index += char_len_at(source, *index);
            continue;
        }
        *index += ch.len_utf8();
        if ch == '`' {
            return Ok(());
        }
    }
    Err(ParseError {
        message: "unterminated backquote".to_string(),
    })
}

fn skip_dquote_body(source: &str, index: &mut usize) -> Result<(), ParseError> {
    while *index < source.len() {
        let ch = next_char(source, *index).unwrap();
        match ch {
            '\\' => {
                *index += 1;
                if *index < source.len() {
                    *index += char_len_at(source, *index);
                }
            }
            '"' => {
                *index += 1;
                return Ok(());
            }
            '$' if matches!(peek_byte(source, *index + 1), Some(b'(' | b'{')) => {
                skip_dollar_construct(source, index)?;
            }
            '`' => {
                *index += 1;
                while *index < source.len() {
                    let next = next_char(source, *index).unwrap();
                    if next == '\\' && *index + 1 < source.len() {
                        *index += 1;
                        *index += char_len_at(source, *index);
                        continue;
                    }
                    *index += next.len_utf8();
                    if next == '`' {
                        break;
                    }
                }
            }
            _ => {
                *index += ch.len_utf8();
            }
        }
    }
    Err(ParseError {
        message: "unterminated double quote".to_string(),
    })
}

/// Word accumulator that tracks either a zero-copy slice into the source
/// or falls back to an owned `String` when the content diverges (e.g.
/// backslash-newline continuation).
enum WordBuf<'src> {
    /// Verbatim span: word is `source[start..current_pos]`.
    Slice { start: usize, source: &'src str },
    /// Modified span: the owned buffer plus a trailing slice
    /// `source[tail..current_pos]` that hasn't been copied yet.
    Owned { buf: String, tail: usize, source: &'src str },
}

impl<'src> WordBuf<'src> {
    fn take(&mut self, end: usize) -> Option<Cow<'src, str>> {
        match self {
            WordBuf::Slice { start, source } => {
                if *start >= end {
                    return None;
                }
                let s = &source[*start..end];
                *start = end;
                Some(Cow::Borrowed(s))
            }
            WordBuf::Owned { buf, tail, source } => {
                buf.push_str(&source[*tail..end]);
                if buf.is_empty() {
                    return None;
                }
                let result = std::mem::take(buf);
                *tail = end;
                Some(Cow::Owned(result))
            }
        }
    }

    fn switch_to_owned(&mut self, end: usize) {
        if let WordBuf::Slice { start, source } = *self {
            let buf = source[start..end].to_string();
            *self = WordBuf::Owned { buf, tail: end, source };
        } else if let WordBuf::Owned { buf, tail, source } = self {
            buf.push_str(&source[*tail..end]);
            *tail = end;
        }
    }

    fn is_word_empty(&self, end: usize) -> bool {
        match self {
            WordBuf::Slice { start, source: _ } => *start >= end,
            WordBuf::Owned { buf, tail, .. } => {
                buf.is_empty() && *tail >= end
            }
        }
    }

    fn try_parse_io_number(&mut self, end: usize) -> Option<i32> {
        match self {
            WordBuf::Slice { start, source } => {
                let text = &source[*start..end];
                if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let fd = text.parse::<i32>().ok()?;
                *start = end;
                Some(fd)
            }
            WordBuf::Owned { buf, tail, source } => {
                let trailing = &source[*tail..end];
                let all_digits = buf.bytes().all(|b| b.is_ascii_digit())
                    && trailing.bytes().all(|b| b.is_ascii_digit());
                if !all_digits || (buf.is_empty() && trailing.is_empty()) {
                    return None;
                }
                let mut full = std::mem::take(buf);
                full.push_str(trailing);
                let fd = full.parse::<i32>().ok()?;
                *tail = end;
                Some(fd)
            }
        }
    }

    fn set_tail(&mut self, pos: usize) {
        if let WordBuf::Owned { tail, .. } = self {
            *tail = pos;
        }
    }

    fn reset_slice(&mut self, pos: usize) {
        match self {
            WordBuf::Slice { start, .. } => *start = pos,
            WordBuf::Owned { buf, tail, .. } => {
                buf.clear();
                *tail = pos;
            }
        }
    }
}

fn tokenize<'src>(source: &'src str) -> Result<Tokenized<'src>, ParseError> {
    let mut tokens: Vec<Token<'src>> = Vec::new();
    let mut here_docs: VecDeque<HereDoc<'src>> = VecDeque::new();
    let mut pending_here_docs: VecDeque<(String, bool, bool)> = VecDeque::new();
    let mut expect_here_doc_target = false;
    let mut index = 0usize;
    let mut word = WordBuf::Slice {
        start: 0,
        source,
    };

    while index < source.len() {
        let ch = next_char(source, index).unwrap();
        match ch {
            ' ' | '\t' | '\r' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                index += 1;
                word.reset_slice(index);
            }
            '\n' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token::Newline);
                index += 1;
                while let Some((delimiter, expand, strip_tabs)) = pending_here_docs.pop_front() {
                    let body = read_here_doc_body(source, &mut index, &delimiter, strip_tabs, expand)?;
                    here_docs.push_back(HereDoc {
                        delimiter: Cow::Owned(delimiter),
                        body: Cow::Owned(body),
                        expand,
                        strip_tabs,
                    });
                }
                word.reset_slice(index);
            }
            '#' if word.is_word_empty(index) => {
                while index < source.len() && peek_byte(source, index) != Some(b'\n') {
                    index += char_len_at(source, index);
                }
                word.reset_slice(index);
            }
            '\'' => {
                let start_of_quote = index;
                index += 1;
                loop {
                    if index >= source.len() {
                        return Err(ParseError {
                            message: "unterminated single quote".to_string(),
                        });
                    }
                    let c = next_char(source, index).unwrap();
                    index += c.len_utf8();
                    if c == '\'' {
                        break;
                    }
                }
                let _ = start_of_quote;
            }
            '"' => {
                index += 1;
                skip_dquote_body(source, &mut index)?;
            }
            '\\' => {
                if peek_byte(source, index + 1) == Some(b'\n') {
                    word.switch_to_owned(index);
                    index += 2;
                    word.set_tail(index);
                } else {
                    index += 1;
                    if index < source.len() {
                        index += char_len_at(source, index);
                    }
                }
            }
            ';' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if peek_byte(source, index + 1) == Some(b';') {
                    tokens.push(Token::DSemi);
                    index += 2;
                } else {
                    tokens.push(Token::Semi);
                    index += 1;
                }
                word.reset_slice(index);
            }
            '&' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if peek_byte(source, index + 1) == Some(b'&') {
                    tokens.push(Token::AndIf);
                    index += 2;
                } else {
                    tokens.push(Token::Amp);
                    index += 1;
                }
                word.reset_slice(index);
            }
            '|' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if peek_byte(source, index + 1) == Some(b'|') {
                    tokens.push(Token::OrIf);
                    index += 2;
                } else {
                    tokens.push(Token::Pipe);
                    index += 1;
                }
                word.reset_slice(index);
            }
            '(' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token::LParen);
                index += 1;
                word.reset_slice(index);
            }
            ')' => {
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                tokens.push(Token::RParen);
                index += 1;
                word.reset_slice(index);
            }
            '<' => {
                if let Some(fd) = word.try_parse_io_number(index) {
                    tokens.push(Token::IoNumber(fd));
                }
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if peek_byte(source, index + 1) == Some(b'<') {
                    if peek_byte(source, index + 2) == Some(b'-') {
                        tokens.push(Token::DLessDash);
                        expect_here_doc_target = true;
                        index += 3;
                    } else {
                        tokens.push(Token::DLess);
                        expect_here_doc_target = true;
                        index += 2;
                    }
                } else if peek_byte(source, index + 1) == Some(b'&') {
                    tokens.push(Token::LessAnd);
                    index += 2;
                } else if peek_byte(source, index + 1) == Some(b'>') {
                    tokens.push(Token::LessGreat);
                    index += 2;
                } else {
                    tokens.push(Token::Less);
                    index += 1;
                }
                word.reset_slice(index);
            }
            '>' => {
                if let Some(fd) = word.try_parse_io_number(index) {
                    tokens.push(Token::IoNumber(fd));
                }
                flush_word(
                    &mut word,
                    index,
                    &mut tokens,
                    &mut expect_here_doc_target,
                    &mut pending_here_docs,
                );
                if peek_byte(source, index + 1) == Some(b'>') {
                    tokens.push(Token::DGreat);
                    index += 2;
                } else if peek_byte(source, index + 1) == Some(b'&') {
                    tokens.push(Token::GreatAnd);
                    index += 2;
                } else if peek_byte(source, index + 1) == Some(b'|') {
                    tokens.push(Token::Clobber);
                    index += 2;
                } else {
                    tokens.push(Token::Greater);
                    index += 1;
                }
                word.reset_slice(index);
            }
            '$' if matches!(peek_byte(source, index + 1), Some(b'(' | b'{')) => {
                skip_dollar_construct(source, &mut index)?;
            }
            '$' if peek_byte(source, index + 1) == Some(b'\'') => {
                skip_dollar_single_quote(source, &mut index)?;
            }
            '`' => {
                let bt_start = index;
                skip_backtick_body(source, &mut index)?;
                let _ = bt_start;
            }
            _ => {
                index += ch.len_utf8();
            }
        }
    }

    flush_word(
        &mut word,
        index,
        &mut tokens,
        &mut expect_here_doc_target,
        &mut pending_here_docs,
    );
    if !pending_here_docs.is_empty() {
        return Err(ParseError {
            message: "unterminated here-document".to_string(),
        });
    }
    tokens.push(Token::Eof);
    Ok(Tokenized { tokens, here_docs })
}

fn flush_word<'src>(
    word: &mut WordBuf<'src>,
    end: usize,
    tokens: &mut Vec<Token<'src>>,
    expect_here_doc_target: &mut bool,
    pending_here_docs: &mut VecDeque<(String, bool, bool)>,
) {
    let Some(cow) = word.take(end) else {
        return;
    };
    if *expect_here_doc_target {
        let strip_tabs = matches!(
            tokens.last(),
            Some(Token::DLessDash)
        );
        let (delimiter, expand) = parse_here_doc_delimiter(&cow);
        pending_here_docs.push_back((delimiter, expand, strip_tabs));
        *expect_here_doc_target = false;
    }
    tokens.push(Token::Word(cow));
}

fn parse_here_doc_delimiter(raw: &str) -> (String, bool) {
    let mut delimiter = String::new();
    let mut index = 0usize;
    let mut expand = true;

    while index < raw.len() {
        let ch = next_char(raw, index).unwrap();
        match ch {
            '\'' => {
                expand = false;
                index += 1;
                while index < raw.len() {
                    let c = next_char(raw, index).unwrap();
                    if c == '\'' {
                        index += 1;
                        break;
                    }
                    delimiter.push(c);
                    index += c.len_utf8();
                }
            }
            '"' => {
                expand = false;
                index += 1;
                while index < raw.len() {
                    let c = next_char(raw, index).unwrap();
                    if c == '"' {
                        index += 1;
                        break;
                    }
                    delimiter.push(c);
                    index += c.len_utf8();
                }
            }
            '\\' => {
                expand = false;
                index += 1;
                if index < raw.len() {
                    let c = next_char(raw, index).unwrap();
                    delimiter.push(c);
                    index += c.len_utf8();
                }
            }
            _ => {
                delimiter.push(ch);
                index += ch.len_utf8();
            }
        }
    }

    (delimiter, expand)
}

fn read_here_doc_body(
    source: &str,
    index: &mut usize,
    delimiter: &str,
    strip_tabs: bool,
    expand: bool,
) -> Result<String, ParseError> {
    let mut body = String::new();

    loop {
        let mut line = String::new();
        let mut had_newline;
        loop {
            let line_start = *index;
            while *index < source.len() && peek_byte(source, *index) != Some(b'\n') {
                *index += char_len_at(source, *index);
            }
            let raw = &source[line_start..*index];
            had_newline = peek_byte(source, *index) == Some(b'\n');
            if had_newline {
                *index += 1;
            }

            if expand && had_newline {
                let trailing = raw.bytes().rev().take_while(|&b| b == b'\\').count();
                if trailing % 2 == 1 {
                    line.push_str(&raw[..raw.len() - 1]);
                    continue;
                }
            }
            line.push_str(raw);
            break;
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

struct Parser<'src> {
    tokens: Vec<Token<'src>>,
    here_docs: VecDeque<HereDoc<'src>>,
    aliases: HashMap<String, String>,
    alias_expand_next_word_at: Option<usize>,
    alias_expansions_remaining: usize,
    index: usize,
}

impl<'src> Parser<'src> {
    fn new(
        tokens: Vec<Token<'src>>,
        here_docs: VecDeque<HereDoc<'src>>,
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
    ) -> Result<Program<'src>, ParseError> {
        let mut items = Vec::new();
        self.skip_separators();

        loop {
            self.expand_alias_at_command_start()?;
            if self.is_eof()
                || (stop_on_closer && self.at_closer())
                || self.at_reserved_stop(stop_words)
                || (stop_on_dsemi && matches!(self.peek(), Token::DSemi))
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

    fn parse_and_or(&mut self) -> Result<AndOr<'src>, ParseError> {
        let first = self.parse_pipeline()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek() {
                Token::AndIf => LogicalOp::And,
                Token::OrIf => LogicalOp::Or,
                _ => break,
            };
            self.index += 1;
            self.skip_linebreaks();
            let rhs = self.parse_pipeline()?;
            rest.push((op, rhs));
        }
        Ok(AndOr { first, rest })
    }

    fn parse_pipeline(&mut self) -> Result<Pipeline<'src>, ParseError> {
        self.expand_alias_at_command_start()?;
        let timed = if self.consume_word("time") {
            self.expand_alias_at_command_start()?;
            if self.consume_word("-p") {
                self.expand_alias_at_command_start()?;
                TimedMode::Posix
            } else {
                TimedMode::Default
            }
        } else {
            TimedMode::Off
        };
        let negated = self.consume_bang();
        self.expand_alias_at_command_start()?;
        let mut commands = vec![self.parse_command()?];
        while matches!(self.peek(), Token::Pipe) {
            self.index += 1;
            self.skip_linebreaks();
            commands.push(self.parse_command()?);
        }
        Ok(Pipeline {
            negated,
            timed,
            commands,
        })
    }

    fn parse_command(&mut self) -> Result<Command<'src>, ParseError> {
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
        } else if self.peek_reserved_word("function") {
            self.parse_function_keyword()?
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
            match self.peek() {
                Token::LParen => {
                    self.index += 1;
                    let body = self.parse_program_until(true, &[], false)?;
                    self.expect(Token::RParen, "expected ')' to close subshell")?;
                    Command::Subshell(body)
                }
                Token::Word(text) if text == "{" => {
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

    fn parse_command_redirections(&mut self, command: Command<'src>) -> Result<Command<'src>, ParseError> {
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

    fn parse_if_command(&mut self) -> Result<Command<'src>, ParseError> {
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

    fn parse_loop_command(&mut self, kind: LoopKind) -> Result<Command<'src>, ParseError> {
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

    fn parse_for_command(&mut self) -> Result<Command<'src>, ParseError> {
        self.expect_reserved_word("for")?;
        let name = match self.peek().clone() {
            Token::Word(text) if is_name(&text) => {
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
                && !matches!(self.peek(), Token::Semi | Token::Newline)
            {
                match self.peek().clone() {
                    Token::Word(text) => {
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

    fn parse_case_command(&mut self) -> Result<Command<'src>, ParseError> {
        self.expect_reserved_word("case")?;
        let word = match self.peek().clone() {
            Token::Word(text) => {
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
            if matches!(self.peek(), Token::LParen) {
                self.index += 1;
            }

            let mut patterns = Vec::new();
            loop {
                match self.peek().clone() {
                    Token::Word(text) => {
                        self.index += 1;
                        patterns.push(Word { raw: text });
                    }
                    _ => {
                        return Err(ParseError {
                            message: "expected case pattern".to_string(),
                        });
                    }
                }

                if matches!(self.peek(), Token::Pipe) {
                    self.index += 1;
                    continue;
                }
                break;
            }

            self.expect(Token::RParen, "expected ')' after case pattern")?;
            self.skip_separators();
            let body = self.parse_program_until(false, &["esac"], true)?;
            arms.push(CaseArm { patterns, body });

            if matches!(self.peek(), Token::DSemi) {
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

    fn parse_function_keyword(&mut self) -> Result<Command<'src>, ParseError> {
        self.expect_reserved_word("function")?;
        let name = match self.peek().clone() {
            Token::Word(name) if is_name(&name) => {
                self.index += 1;
                name
            }
            _ => {
                return Err(ParseError {
                    message: "expected function name".to_string(),
                });
            }
        };
        if matches!(self.peek(), Token::LParen) {
            self.index += 1;
            self.expect(Token::RParen, "expected ')' after '('")?;
        }
        let body = self.parse_command()?;
        Ok(Command::FunctionDef(FunctionDef {
            name,
            body: Box::new(body),
        }))
    }

    fn parse_simple_command(&mut self) -> Result<SimpleCommand<'src>, ParseError> {
        let mut command = SimpleCommand::default();

        loop {
            self.expand_alias_after_blank_in_simple_command()?;
            if let Some(redirection) = self.try_parse_redirection()? {
                command.redirections.push(redirection);
                continue;
            }

            if command.words.is_empty() {
                if let Some(text) = self.peek_word_text() {
                    if let Some((name, value)) = split_assignment(&text) {
                        let name = Cow::Owned(name.to_string());
                        let value = Word { raw: Cow::Owned(value.to_string()) };
                        self.index += 1;
                        command.assignments.push(Assignment { name, value });
                        continue;
                    }
                }
                if !command.assignments.is_empty() || !command.redirections.is_empty() {
                    self.expand_alias_at_command_start()?;
                }
            }

            let word = match self.peek().clone() {
                Token::Word(text) => {
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

        if !command.words.is_empty() && matches!(self.peek(), Token::LParen) {
            return Err(ParseError {
                message: "syntax error near unexpected token `('".to_string(),
            });
        }

        Ok(command)
    }

    fn try_parse_redirection(&mut self) -> Result<Option<Redirection<'src>>, ParseError> {
        let (fd, kind) = match self.peek().clone() {
            Token::IoNumber(fd) => {
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

        let target = match self.peek().clone() {
            Token::Word(text) => {
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
        let kind = match self.tokens.get(index) {
            Some(Token::Less) => RedirectionKind::Read,
            Some(Token::Greater) => RedirectionKind::Write,
            Some(Token::Clobber) => RedirectionKind::ClobberWrite,
            Some(Token::DGreat) => RedirectionKind::Append,
            Some(Token::DLess | Token::DLessDash) => RedirectionKind::HereDoc,
            Some(Token::LessAnd) => RedirectionKind::DupInput,
            Some(Token::GreatAnd) => RedirectionKind::DupOutput,
            Some(Token::LessGreat) => RedirectionKind::ReadWrite,
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
        while matches!(self.peek(), Token::Newline | Token::Semi) {
            self.index += 1;
        }
    }

    fn skip_linebreaks(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.index += 1;
        }
    }

    fn consume_amp(&mut self) -> bool {
        if matches!(self.peek(), Token::Amp) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Token::Word(text) if text == word) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_bang(&mut self) -> bool {
        if matches!(self.peek(), Token::Word(text) if text == "!") {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token<'src>, message: &str) -> Result<(), ParseError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected) {
            self.index += 1;
            Ok(())
        } else {
            Err(ParseError {
                message: message.to_string(),
            })
        }
    }

    fn peek(&self) -> &Token<'src> {
        &self.tokens[self.index]
    }

    fn peek_word_text(&self) -> Option<Cow<'src, str>> {
        match &self.tokens[self.index] {
            Token::Word(text) => Some(text.clone()),
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
        let Some(Token::Word(text)) = self.tokens.get(self.index) else {
            return Ok(false);
        };
        if !is_alias_word(text) {
            return Ok(false);
        }
        let Some(replacement) = self.aliases.get(text.as_ref()).cloned() else {
            return Ok(false);
        };
        if self.alias_expansions_remaining == 0 {
            return Err(ParseError {
                message: "alias expansion too deep".to_string(),
            });
        };
        self.alias_expansions_remaining -= 1;
        let tokenized = tokenize(&replacement)?;
        let mut replacement_tokens: Vec<Token<'src>> = tokenized
            .tokens
            .into_iter()
            .map(own_token)
            .collect();
        if replacement_tokens
            .last()
            .is_some_and(|t| matches!(t, Token::Eof))
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

    fn try_peek_function_name(&self) -> Option<Cow<'src, str>> {
        let name = match self.tokens.get(self.index)?.clone() {
            Token::Word(name) => name,
            _ => return None,
        };
        if is_reserved_word(&name) {
            return None;
        }
        if !is_name(&name) {
            return None;
        }
        if !matches!(self.tokens.get(self.index + 1), Some(Token::LParen)) {
            return None;
        }
        if !matches!(self.tokens.get(self.index + 2), Some(Token::RParen)) {
            return None;
        }
        Some(name)
    }

    fn peek_reserved_word(&self, word: &str) -> bool {
        matches!(self.peek(), Token::Word(text) if text == word)
    }

    fn peek_bang_word(&self) -> bool {
        matches!(self.peek(), Token::Word(text) if text == "!")
    }

    fn at_reserved_stop(&self, stop_words: &[&str]) -> bool {
        match self.peek() {
            Token::Word(text) => stop_words.iter().any(|word| text == word),
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
        matches!(self.peek(), Token::Eof)
    }

    fn at_closer(&self) -> bool {
        matches!(self.peek(), Token::RParen) || self.peek_reserved_word("}")
    }
}

fn own_token<'a>(token: Token<'_>) -> Token<'a> {
    match token {
        Token::Word(cow) => Token::Word(Cow::Owned(cow.into_owned())),
        Token::IoNumber(fd) => Token::IoNumber(fd),
        Token::Newline => Token::Newline,
        Token::Semi => Token::Semi,
        Token::DSemi => Token::DSemi,
        Token::Amp => Token::Amp,
        Token::Pipe => Token::Pipe,
        Token::AndIf => Token::AndIf,
        Token::OrIf => Token::OrIf,
        Token::LParen => Token::LParen,
        Token::RParen => Token::RParen,
        Token::Less => Token::Less,
        Token::Greater => Token::Greater,
        Token::DGreat => Token::DGreat,
        Token::DLess => Token::DLess,
        Token::DLessDash => Token::DLessDash,
        Token::LessAnd => Token::LessAnd,
        Token::GreatAnd => Token::GreatAnd,
        Token::LessGreat => Token::LessGreat,
        Token::Clobber => Token::Clobber,
        Token::Eof => Token::Eof,
    }
}

fn split_assignment(input: &str) -> Option<(&str, &str)> {
    let (name, value) = input.split_once('=')?;
    if !is_name(name) {
        return None;
    }
    Some((name, value))
}

fn is_alias_word(word: &str) -> bool {
    !word.is_empty() && !word.chars().any(|ch| matches!(ch, '\'' | '"' | '\\'))
}

fn alias_has_trailing_blank(alias: &str) -> bool {
    matches!(alias.chars().last(), Some(' ' | '\t'))
}

pub fn is_name(name: &str) -> bool {
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
            | "function"
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
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body.as_ref()) == Some("hello $USER\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.delimiter.as_ref()) == Some("EOF")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body.as_ref()) == Some("one\n")
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["echo", "}"]
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
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>())
                        == Some(vec!["a", "b"])
        ));

        let reserved_words_as_items = parse("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>())
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
        assert!(parser.expect(Token::Semi, "semi").is_err());

        let mut parser = Parser::new(
            vec![
                Token::Newline,
                Token::Word("do".into()),
                Token::Eof,
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        parser.skip_linebreaks();
        assert!(parser.peek_reserved_word("do"));

        let func_tokens = tokenize("name( x").expect("tokenize");
        let parser = Parser::new(func_tokens.tokens, VecDeque::new(), HashMap::new());
        assert_eq!(parser.try_peek_function_name(), None);

        let closer_tokens = vec![Token::Word("}".into())];
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
                Token::Word("word".into()),
                Token::Eof,
            ],
            VecDeque::new(),
            HashMap::from([(String::from("word"), String::from("ok"))]),
        );
        parser.alias_expand_next_word_at = Some(0);
        parser
            .expand_alias_after_blank_in_simple_command()
            .expect("expand pending alias");
        assert!(matches!(parser.peek(), Token::Word(text) if text == "ok"));

        let mut parser = Parser::new(
            vec![
                Token::Word("loop".into()),
                Token::Eof,
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
                Token::Word("tail".into()),
                Token::Eof,
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["printf", "ok"]
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["printf", "hi"]
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["echo", "!"]
        ));

        let program = parse("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["!true"]
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["printf", "%s", "ok"]
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
                if simple.words.iter().map(|word| word.raw.as_ref()).collect::<Vec<_>>() == vec!["loop", "ok"]
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
                if simple.words.iter().map(|w| w.raw.as_ref()).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.assignments.len() == 1
        ));

        let program = parse_with_aliases("</dev/null foo", &aliases)
            .expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| w.raw.as_ref()).collect::<Vec<_>>() == vec!["echo", "aliased"]
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
                Token::Word("case".into()),
                Token::Semi,
                Token::Eof,
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
                Token::Word("case".into()),
                Token::Word("name".into()),
                Token::Newline,
                Token::Word("esac".into()),
                Token::Eof,
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
                Token::Word("case".into()),
                Token::Word("name".into()),
                Token::Word("in".into()),
                Token::RParen,
                Token::Word("esac".into()),
                Token::Eof,
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
                Token::Word("case".into()),
                Token::Word("name".into()),
                Token::Word("in".into()),
                Token::Word("foo".into()),
                Token::RParen,
                Token::Word("echo".into()),
                Token::Word("hi".into()),
                Token::Semi,
                Token::Eof,
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

        let source = "line";
        let mut index = 0usize;
        assert_eq!(
            read_here_doc_body(source, &mut index, "EOF", false, true)
                .expect_err("unterminated body")
                .message,
            "unterminated here-document"
        );

        let source = "EO\\\nF\n";
        let mut index = 0usize;
        let body = read_here_doc_body(source, &mut index, "EOF", false, true)
            .expect("continuation delimiter");
        assert_eq!(body, "");

        let source2 = "body\\\nEOF\nreal body\nEOF\n";
        let mut index2 = 0usize;
        let body2 = read_here_doc_body(source2, &mut index2, "EOF", false, true)
            .expect("continuation body");
        assert_eq!(body2, "bodyEOF\nreal body\n");

        let tokenized = tokenize("cat <<A <<'B'\nfirst\nA\nsecond\nB\n").expect("tokenize");
        assert_eq!(tokenized.here_docs.len(), 2);
        assert_eq!(tokenized.here_docs[0].body, "first\n");
        assert_eq!(tokenized.here_docs[1].body, "second\n");

        let mut parser = Parser::new(
            vec![
                Token::DLess,
                Token::Word("EOF".into()),
                Token::Eof,
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
                Token::IoNumber(2),
                Token::Greater,
                Token::Word("out".into()),
                Token::Eof,
            ],
            VecDeque::new(),
            HashMap::new(),
        );
        let redir = parser
            .try_parse_redirection()
            .expect("valid redirection")
            .expect("should be Some");
        assert_eq!(redir.fd, Some(2));
        assert_eq!(redir.kind, RedirectionKind::Write);

        let parser = Parser::new(
            vec![Token::Eof],
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
    fn tokenizer_emits_io_number_for_adjacent_digits() {
        let t = tokenize("2>err").expect("tokenize");
        assert_eq!(t.tokens[0], Token::IoNumber(2));
        assert_eq!(t.tokens[1], Token::Greater);
        assert_eq!(t.tokens[2], Token::Word("err".into()));

        let t = tokenize("0<in").expect("tokenize");
        assert_eq!(t.tokens[0], Token::IoNumber(0));
        assert_eq!(t.tokens[1], Token::Less);

        let t = tokenize("2 >err").expect("tokenize");
        assert_eq!(t.tokens[0], Token::Word("2".into()));
        assert_eq!(t.tokens[1], Token::Greater);

        let t = tokenize("abc>err").expect("tokenize");
        assert_eq!(t.tokens[0], Token::Word("abc".into()));
        assert_eq!(t.tokens[1], Token::Greater);

        let t = tokenize("999999999999999999999>out").expect("tokenize");
        assert_eq!(t.tokens[0], Token::Word("999999999999999999999".into()));
        assert_eq!(t.tokens[1], Token::Greater);
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

    #[test]
    fn time_default_pipeline() {
        let program = parse("time echo hello").expect("parse time default");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Default);
        assert!(!pipeline.negated);
        assert!(matches!(&pipeline.commands[0], Command::Simple(cmd) if cmd.words[0].raw == "echo"));
    }

    #[test]
    fn time_posix_pipeline() {
        let program = parse("time -p echo hello").expect("parse time -p");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Posix);
        assert!(matches!(&pipeline.commands[0], Command::Simple(cmd) if cmd.words[0].raw == "echo"));
    }

    #[test]
    fn function_keyword_basic() {
        let program = parse("function foo { echo hi; }").expect("parse function keyword");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_with_parens() {
        let program = parse("function foo() { echo hi; }").expect("parse function keyword parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_invalid_name() {
        let error = parse("function 123").expect_err("bad function name");
        assert_eq!(error.message, "expected function name");
    }

    #[test]
    fn into_static_covers_all_command_variants() {
        use std::borrow::Cow;

        let simple = Command::Simple(SimpleCommand {
            assignments: vec![Assignment {
                name: Cow::Borrowed("X"),
                value: Word {
                    raw: Cow::Borrowed("1"),
                },
            }],
            words: vec![Word {
                raw: Cow::Borrowed("echo"),
            }],
            redirections: vec![Redirection {
                fd: Some(2),
                kind: RedirectionKind::Write,
                target: Word {
                    raw: Cow::Borrowed("err"),
                },
                here_doc: None,
            }],
        });
        let s: Command<'static> = simple.into_static();
        assert!(matches!(s, Command::Simple(ref sc) if sc.words[0].raw == "echo"));

        let subshell = Command::Subshell(Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![s.clone()],
                    },
                    rest: vec![],
                },
                asynchronous: false,
            }],
        });
        assert!(matches!(subshell.clone().into_static(), Command::Subshell(_)));

        let group = Command::Group(Program { items: vec![] });
        assert!(matches!(group.into_static(), Command::Group(_)));

        let func = Command::FunctionDef(FunctionDef {
            name: Cow::Borrowed("f"),
            body: Box::new(s.clone()),
        });
        assert!(matches!(func.into_static(), Command::FunctionDef(fd) if fd.name == "f"));

        let if_cmd = Command::If(IfCommand {
            condition: Program { items: vec![] },
            then_branch: Program { items: vec![] },
            elif_branches: vec![ElifBranch {
                condition: Program { items: vec![] },
                body: Program { items: vec![] },
            }],
            else_branch: Some(Program { items: vec![] }),
        });
        assert!(matches!(if_cmd.into_static(), Command::If(_)));

        let loop_cmd = Command::Loop(LoopCommand {
            kind: LoopKind::While,
            condition: Program { items: vec![] },
            body: Program { items: vec![] },
        });
        assert!(matches!(loop_cmd.into_static(), Command::Loop(_)));

        let for_cmd = Command::For(ForCommand {
            name: Cow::Borrowed("i"),
            items: Some(vec![Word {
                raw: Cow::Borrowed("a"),
            }]),
            body: Program { items: vec![] },
        });
        let for_static = for_cmd.into_static();
        assert!(matches!(&for_static, Command::For(fc) if fc.name == "i"));

        let case_cmd = Command::Case(CaseCommand {
            word: Word {
                raw: Cow::Borrowed("x"),
            },
            arms: vec![CaseArm {
                patterns: vec![Word {
                    raw: Cow::Borrowed("*"),
                }],
                body: Program { items: vec![] },
            }],
        });
        assert!(matches!(case_cmd.into_static(), Command::Case(_)));

        let redir = Command::Redirected(
            Box::new(s.clone()),
            vec![Redirection {
                fd: None,
                kind: RedirectionKind::Write,
                target: Word {
                    raw: Cow::Borrowed("out"),
                },
                here_doc: Some(HereDoc {
                    delimiter: Cow::Borrowed("EOF"),
                    body: Cow::Borrowed("test\n"),
                    expand: true,
                    strip_tabs: false,
                }),
            }],
        );
        assert!(matches!(redir.into_static(), Command::Redirected(_, _)));
    }

    #[test]
    fn alias_expansion_produces_non_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert("both".to_string(), "echo a; echo b".to_string());
        let program =
            parse_with_aliases("both", &aliases).expect("parse alias with semicolon");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn backslash_newline_mid_word_produces_owned_cow() {
        let program = parse("ec\\\nho ok").expect("continuation in command");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "echo" && cmd.words[1].raw == "ok"
        ));

        let program = parse("echo a\\\nb\\\nc").expect("multiple continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "abc"
        ));

        let program = parse("2\\\n>err echo ok").expect("continuation in digit");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "echo"
        ));
    }

    #[test]
    fn own_token_all_variants() {
        use std::borrow::Cow;
        let cases: Vec<Token<'_>> = vec![
            Token::Word(Cow::Borrowed("hello")),
            Token::IoNumber(3),
            Token::Newline,
            Token::Semi,
            Token::DSemi,
            Token::Amp,
            Token::Pipe,
            Token::AndIf,
            Token::OrIf,
            Token::LParen,
            Token::RParen,
            Token::Less,
            Token::Greater,
            Token::DGreat,
            Token::DLess,
            Token::DLessDash,
            Token::LessAnd,
            Token::GreatAnd,
            Token::LessGreat,
            Token::Clobber,
            Token::Eof,
        ];
        for token in cases {
            let owned: Token<'static> = own_token(token);
            match owned {
                Token::Word(cow) => assert_eq!(&*cow, "hello"),
                _ => {}
            }
        }
    }

    #[test]
    fn skip_scan_covers_dollar_single_quote_and_default_in_subshell() {
        let program = parse("echo $(echo $'hi' done)").expect("dollar-sq in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program = parse("echo $(echo $VAR done)").expect("bare dollar in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let err = parse("echo $(echo 'unterminated)").expect_err("sq in paren");
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn backslash_newline_before_comment_does_not_start_comment() {
        let program = parse("a\\\n#b").expect("continuation before hash");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "a#b"
        ));
    }

    #[test]
    fn backslash_newline_before_operator_resets_owned_mode() {
        let program = parse("echo a\\\nb; echo c").expect("continuation before semi");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn backslash_newline_non_digit_before_redirect_is_not_io_number() {
        let program = parse("a\\\nb>out").expect("non-digit continuation before redir");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "ab"
                && cmd.redirections.len() == 1
                && cmd.redirections[0].fd.is_none()
        ));
    }
}
