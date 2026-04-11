use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub items: Box<[ListItem]>,
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub and_or: AndOr,
    pub asynchronous: bool,
    pub line: usize,
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.and_or == other.and_or && self.asynchronous == other.asynchronous
    }
}
impl Eq for ListItem {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndOr {
    pub first: Pipeline,
    pub rest: Box<[(LogicalOp, Pipeline)]>,
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
pub struct Pipeline {
    pub negated: bool,
    pub timed: TimedMode,
    pub commands: Box<[Command]>,
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
    Redirected(Box<Command>, Box<[Redirection]>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCommand {
    pub assignments: Box<[Assignment]>,
    pub words: Box<[Word]>,
    pub redirections: Box<[Redirection]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub name: Box<str>,
    pub value: Word,
}

#[derive(Clone, Debug)]
pub struct Word {
    pub raw: Box<str>,
    pub line: usize,
}

impl PartialEq for Word {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl Eq for Word {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirection {
    pub fd: Option<i32>,
    pub kind: RedirectionKind,
    pub target: Word,
    pub here_doc: Option<HereDoc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionDef {
    pub name: Box<str>,
    pub body: Box<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfCommand {
    pub condition: Program,
    pub then_branch: Program,
    pub elif_branches: Box<[ElifBranch]>,
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
    pub name: Box<str>,
    pub items: Option<Box<[Word]>>,
    pub body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCommand {
    pub word: Word,
    pub arms: Box<[CaseArm]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseArm {
    pub patterns: Box<[Word]>,
    pub body: Program,
    pub fallthrough: bool,
}

#[derive(Clone, Debug)]
pub struct HereDoc {
    pub delimiter: Box<str>,
    pub body: Box<str>,
    pub expand: bool,
    pub strip_tabs: bool,
    pub body_line: usize,
}

impl PartialEq for HereDoc {
    fn eq(&self, other: &Self) -> bool {
        self.delimiter == other.delimiter
            && self.body == other.body
            && self.expand == other.expand
            && self.strip_tabs == other.strip_tabs
    }
}
impl Eq for HereDoc {}

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
    pub message: Box<str>,
    pub line: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

#[allow(dead_code)]
fn line_at(source: &str, index: usize) -> usize {
    source[..index.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

struct AliasLayer<'a> {
    text: Cow<'a, str>,
    pos: usize,

    trailing_blank: bool,
}

const BC_WORD_BREAK: u8 = 0x01;
const BC_DELIM: u8      = 0x02;
const BC_BLANK: u8      = 0x04;
const BC_QUOTE: u8      = 0x08;
const BC_NAME_START: u8 = 0x10;
const BC_NAME_CONT: u8  = 0x20;

const BYTE_CLASS: [u8; 256] = {
    let mut t = [0u8; 256];

    t[b' '  as usize] = BC_WORD_BREAK | BC_DELIM | BC_BLANK;
    t[b'\t' as usize] = BC_WORD_BREAK | BC_DELIM | BC_BLANK;

    t[b'\n' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b';'  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'&'  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'|'  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'('  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b')'  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'<'  as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'>'  as usize] = BC_WORD_BREAK | BC_DELIM;

    t[b'#'  as usize] = BC_DELIM;

    t[b'\'' as usize] |= BC_QUOTE;
    t[b'"'  as usize] |= BC_QUOTE;
    t[b'\\' as usize] |= BC_QUOTE;
    t[b'$'  as usize] |= BC_QUOTE;
    t[b'`'  as usize] |= BC_QUOTE;

    t[b'_' as usize] |= BC_NAME_START | BC_NAME_CONT;
    let mut c: u8 = b'A';
    while c <= b'Z' {
        t[c as usize] |= BC_NAME_START | BC_NAME_CONT;
        c += 1;
    }
    c = b'a';
    while c <= b'z' {
        t[c as usize] |= BC_NAME_START | BC_NAME_CONT;
        c += 1;
    }
    c = b'0';
    while c <= b'9' {
        t[c as usize] |= BC_NAME_CONT;
        c += 1;
    }

    t
};

#[inline(always)]
fn is_delim(b: u8) -> bool {
    BYTE_CLASS[b as usize] & BC_DELIM != 0
}

#[inline(always)]
fn is_word_break(b: u8) -> bool {
    BYTE_CLASS[b as usize] & BC_WORD_BREAK != 0
}

#[inline(always)]
fn is_quote(b: u8) -> bool {
    BYTE_CLASS[b as usize] & BC_QUOTE != 0
}

fn alias_has_trailing_blank(s: &str) -> bool {
    s.as_bytes()
        .last()
        .map_or(false, |&b| BYTE_CLASS[b as usize] & BC_BLANK != 0)
}

fn is_alias_word(word: &str) -> bool {
    !word.is_empty() && !word.bytes().any(|b| is_quote(b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(Box<str>),
    IoNumber(i32),

    Pipe,
    OrIf,
    Amp,
    AndIf,
    Semi,
    DSemi,
    SemiAmp,
    Less,
    Great,
    DGreat,
    LessAnd,
    GreatAnd,
    LessGreat,
    Clobber,
    LParen,
    RParen,

    HereDoc {
        strip_tabs: bool,
        expand: bool,
        delimiter: Box<str>,
        body: Box<str>,
        body_line: usize,
    },

    If,
    Then,
    Else,
    Elif,
    Fi,
    Do,
    Done,
    Case,
    Esac,
    In,
    While,
    Until,
    For,
    Bang,
    LBrace,
    RBrace,
    Function,

    Newline,
    Eof,
}

impl Token {
    fn keyword_name(&self) -> Option<&'static str> {
        match self {
            Token::If => Some("if"),
            Token::Then => Some("then"),
            Token::Else => Some("else"),
            Token::Elif => Some("elif"),
            Token::Fi => Some("fi"),
            Token::Do => Some("do"),
            Token::Done => Some("done"),
            Token::Case => Some("case"),
            Token::Esac => Some("esac"),
            Token::In => Some("in"),
            Token::While => Some("while"),
            Token::Until => Some("until"),
            Token::For => Some("for"),
            Token::Function => Some("function"),
            _ => None,
        }
    }
}

fn token_to_keyword_name(tok: &Token) -> Box<str> {
    tok.keyword_name()
        .unwrap_or(match tok {
            Token::Bang => "!",
            Token::LBrace => "{",
            Token::RBrace => "}",
            _ => "word",
        })
        .into()
}

fn word_to_keyword_token(w: &str) -> Option<Token> {
    match w {
        "if" => Some(Token::If),
        "then" => Some(Token::Then),
        "else" => Some(Token::Else),
        "elif" => Some(Token::Elif),
        "fi" => Some(Token::Fi),
        "do" => Some(Token::Do),
        "done" => Some(Token::Done),
        "case" => Some(Token::Case),
        "esac" => Some(Token::Esac),
        "in" => Some(Token::In),
        "while" => Some(Token::While),
        "until" => Some(Token::Until),
        "for" => Some(Token::For),
        "function" => Some(Token::Function),
        "!" => Some(Token::Bang),
        "{" => Some(Token::LBrace),
        "}" => Some(Token::RBrace),
        _ => None,
    }
}

pub struct Parser<'src, 'a> {
    source: &'src str,
    pos: usize,
    line: usize,
    cached_byte: Option<u8>,
    aliases: &'a HashMap<Box<str>, Box<str>>,
    alias_stack: Vec<AliasLayer<'a>>,
    alias_depth: usize,
    expanding_aliases: Vec<String>,
    alias_trailing_blank_pending: bool,
    next_cached_byte: Option<u8>,
    cached_token: Option<Token>,
    token_queue: VecDeque<Token>,
    keyword_mode: bool,
}

impl<'src, 'a> Parser<'src, 'a> {
    fn new(source: &'src str, aliases: &'a HashMap<Box<str>, Box<str>>) -> Self {
        Self::new_at(source, 0, 1, aliases)
    }

    fn new_at(
        source: &'src str,
        pos: usize,
        line: usize,
        aliases: &'a HashMap<Box<str>, Box<str>>,
    ) -> Self {
        let cached_byte = source.as_bytes().get(pos).copied();
        Self {
            source,
            pos,
            line,
            cached_byte,
            aliases,
            alias_stack: Vec::new(),
            alias_depth: 0,
            expanding_aliases: Vec::new(),
            alias_trailing_blank_pending: false,
            next_cached_byte: None,
            cached_token: None,
            token_queue: VecDeque::new(),
            keyword_mode: true,
        }
    }

    pub fn current_line(&self) -> usize {
        self.line
    }

    fn sync_cache(&mut self) {
        if let Some(layer) = self.alias_stack.last() {
            self.cached_byte = layer.text.as_bytes().get(layer.pos).copied();
        } else {
            self.cached_byte = self.source.as_bytes().get(self.pos).copied();
        }
    }

    fn error(&self, message: impl Into<Box<str>>) -> ParseError {
        ParseError {
            message: message.into(),
            line: Some(self.line),
        }
    }

    #[inline(always)]
    fn pop_exhausted_layers(&mut self) {
        if let Some(layer) = self.alias_stack.last() {
            if layer.pos < layer.text.len() {
                return;
            }
            self.pop_exhausted_layers_slow();
        }
    }

    #[cold]
    fn pop_exhausted_layers_slow(&mut self) {
        while let Some(layer) = self.alias_stack.last() {
            if layer.pos < layer.text.len() {
                break;
            }
            if layer.trailing_blank {
                self.alias_trailing_blank_pending = true;
            }
            self.alias_stack.pop();
            self.alias_depth = self.alias_depth.saturating_sub(1);
            self.expanding_aliases.pop();
        }
    }

    #[inline(always)]
    fn peek_byte(&self) -> Option<u8> {
        self.cached_byte
    }

    #[inline(always)]
    fn advance_byte(&mut self) {
        if let Some(b) = self.next_cached_byte.take() {
            self.cached_byte = Some(b);
            return;
        }
        if let Some(layer) = self.alias_stack.last_mut() {
            layer.pos += 1;
            self.cached_byte = layer.text.as_bytes().get(layer.pos).copied();
            if self.cached_byte.is_none() {
                self.pop_exhausted_layers();
                self.sync_cache();
            }
            return;
        }
        let bytes = self.source.as_bytes();
        if self.pos < bytes.len() {
            if bytes[self.pos] == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        self.cached_byte = bytes.get(self.pos).copied();
    }

    fn skip_continuations(&mut self) {
        loop {
            if self.cached_byte != Some(b'\\') {
                return;
            }
            self.advance_byte();
            if self.cached_byte == Some(b'\n') {
                self.advance_byte();
            } else {
                self.next_cached_byte = self.cached_byte;
                self.cached_byte = Some(b'\\');
                return;
            }
        }
    }

    fn skip_blanks(&mut self) {
        loop {
            match self.peek_byte() {
                Some(b' ' | b'\t') => self.advance_byte(),
                _ => break,
            }
        }
    }

    fn skip_blanks_and_comments(&mut self) {
        self.skip_blanks();
        if self.peek_byte() == Some(b'#') {
            while !matches!(self.peek_byte(), None | Some(b'\n')) {
                self.advance_byte();
            }
            self.skip_blanks();
        }
    }

    fn consume_single_quote(&mut self, raw: &mut String) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated single quote")),
                Some(b'\'') => {
                    raw.push('\'');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_double_quote(&mut self, raw: &mut String) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated double quote")),
                Some(b'"') => {
                    raw.push('"');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push('\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b as char);
                        self.advance_byte();
                    }
                }
                Some(b'$') => {
                    raw.push('$');
                    self.advance_byte();
                    self.consume_dollar_construct(raw)?;
                }
                Some(b'`') => {
                    raw.push('`');
                    self.advance_byte();
                    self.consume_backtick_inner(raw)?;
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_dollar_construct(&mut self, raw: &mut String) -> Result<(), ParseError> {
        match self.peek_byte() {
            Some(b'(') => {
                raw.push('(');
                self.advance_byte();
                self.skip_continuations();
                if self.peek_byte() == Some(b'(') {
                    raw.push('(');
                    self.advance_byte();
                    self.consume_arith_body(raw)
                } else {
                    self.consume_paren_body(raw)
                }
            }
            Some(b'{') => {
                raw.push('{');
                self.advance_byte();
                self.consume_brace_body(raw)
            }
            _ => Ok(()),
        }
    }

    fn consume_arith_body(&mut self, raw: &mut String) -> Result<(), ParseError> {
        let mut depth = 1usize;
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated arithmetic expansion")),
                Some(b'(') => {
                    depth += 1;
                    raw.push('(');
                    self.advance_byte();
                }
                Some(b')') => {
                    if depth == 1 {
                        raw.push(')');
                        self.advance_byte();
                        self.skip_continuations();
                        if self.peek_byte() == Some(b')') {
                            raw.push(')');
                            self.advance_byte();
                            return Ok(());
                        }
                    } else {
                        depth -= 1;
                        raw.push(')');
                        self.advance_byte();
                    }
                }
                Some(b) if is_quote(b) => {
                    self.consume_quoted_element(raw)?;
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_dollar_single_quote(&mut self, raw: &mut String) -> Result<(), ParseError> {
        raw.push('\'');
        self.advance_byte();
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated dollar-single-quotes")),
                Some(b'\'') => {
                    raw.push('\'');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push('\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b as char);
                        self.advance_byte();
                    }
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_paren_body(&mut self, raw: &mut String) -> Result<(), ParseError> {
        let mut depth = 1usize;
        let mut at_boundary = true;
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated command substitution")),
                Some(b'(') => {
                    depth += 1;
                    at_boundary = true;
                    raw.push('(');
                    self.advance_byte();
                }
                Some(b')') => {
                    depth -= 1;
                    raw.push(')');
                    self.advance_byte();
                    if depth == 0 {
                        return Ok(());
                    }
                    at_boundary = true;
                }
                Some(b'#') if at_boundary => {
                    while !matches!(self.peek_byte(), None | Some(b'\n')) {
                        if let Some(b) = self.peek_byte() {
                            raw.push(b as char);
                        }
                        self.advance_byte();
                    }
                }
                Some(b'\\') => {
                    raw.push('\\');
                    self.advance_byte();
                    if self.peek_byte() == Some(b'\n') {
                        raw.push('\n');
                        self.advance_byte();
                    } else {
                        at_boundary = false;
                        if let Some(b) = self.peek_byte() {
                            raw.push(b as char);
                            self.advance_byte();
                        }
                    }
                }
                Some(b) if is_quote(b) => {
                    at_boundary = false;
                    self.consume_quoted_element(raw)?;
                }
                Some(b) if is_word_break(b) => {
                    at_boundary = true;
                    raw.push(b as char);
                    self.advance_byte();
                }
                Some(b) => {
                    at_boundary = false;
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_brace_body(&mut self, raw: &mut String) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated parameter expansion")),
                Some(b'}') => {
                    raw.push('}');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b) if is_quote(b) => {
                    self.consume_quoted_element(raw)?;
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_backtick_inner(&mut self, raw: &mut String) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated backquote")),
                Some(b'`') => {
                    raw.push('`');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push('\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b as char);
                        self.advance_byte();
                    }
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_quoted_element(&mut self, raw: &mut String) -> Result<(), ParseError> {
        match self.peek_byte() {
            Some(b'\'') => {
                raw.push('\'');
                self.advance_byte();
                self.consume_single_quote(raw)
            }
            Some(b'"') => {
                raw.push('"');
                self.advance_byte();
                self.consume_double_quote(raw)
            }
            Some(b'\\') => {
                raw.push('\\');
                self.advance_byte();
                if let Some(b) = self.peek_byte() {
                    raw.push(b as char);
                    self.advance_byte();
                }
                Ok(())
            }
            Some(b'$') => {
                raw.push('$');
                self.advance_byte();
                self.consume_dollar_construct(raw)
            }
            Some(b'`') => {
                raw.push('`');
                self.advance_byte();
                self.consume_backtick_inner(raw)
            }
            _ => {
                if let Some(b) = self.peek_byte() {
                    raw.push(b as char);
                }
                self.advance_byte();
                Ok(())
            }
        }
    }

    fn read_here_doc_body(
        &mut self,
        delimiter: &str,
        strip_tabs: bool,
        expand: bool,
    ) -> Result<String, ParseError> {
        let mut body = String::new();
        let mut delim_compare = String::new();
        let mut body_before_continuation = 0;
        loop {
            let mut line = String::new();
            let has_newline = loop {
                match self.peek_byte() {
                    Some(b'\n') => {
                        self.advance_byte();
                        break true;
                    }
                    Some(b) => {
                        line.push(b as char);
                        self.advance_byte();
                    }
                    None => break false,
                }
            };

            if expand && line.ends_with('\\') && has_newline {
                if delim_compare.is_empty() {
                    body_before_continuation = body.len();
                }
                delim_compare.push_str(&line[..line.len() - 1]);
                body.push_str(&line);
                body.push('\n');
                continue;
            }

            let compare = if !delim_compare.is_empty() {
                delim_compare.push_str(&line);
                &delim_compare
            } else {
                &line
            };
            let compare = if strip_tabs {
                compare.trim_start_matches('\t')
            } else {
                compare
            };
            if compare == delimiter {
                if !delim_compare.is_empty() {
                    body.truncate(body_before_continuation);
                }
                return Ok(body);
            }
            delim_compare.clear();

            if !has_newline {
                return Err(ParseError {
                    message: "unterminated here-document".into(),
                    line: Some(self.line),
                });
            }

            body.push_str(&line);
            body.push('\n');
        }
    }

    fn set_keyword_mode(&mut self, enabled: bool) {
        self.keyword_mode = enabled;
    }

    fn peek_token(&mut self) -> Result<&Token, ParseError> {
        if self.cached_token.is_none() {
            let tok = self.produce_next_token()?;
            self.cached_token = Some(tok);
        }
        Ok(self.cached_token.as_ref().unwrap())
    }

    fn advance_token(&mut self) -> Token {
        self.cached_token.take().expect("advance_token without peek_token")
    }

    fn produce_next_token(&mut self) -> Result<Token, ParseError> {
        if let Some(tok) = self.token_queue.pop_front() {
            return self.reclassify_queued_token(tok);
        }
        self.produce_token_from_bytes()
    }

    fn reclassify_queued_token(&mut self, tok: Token) -> Result<Token, ParseError> {
        let check_keyword = self.keyword_mode;
        let check_alias = self.keyword_mode || self.alias_trailing_blank_pending;
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        if let Token::Word(ref w) = tok {
            if check_keyword {
                if let Some(kw_tok) = word_to_keyword_token(w) {
                    return Ok(kw_tok);
                }
            }
            if check_alias {
                if let Some(value) = self.aliases.get(&**w) {
                    if is_alias_word(w)
                        && !self.expanding_aliases.iter().any(|n| n == &**w)
                        && self.alias_depth < 1024
                    {
                        let value: &str = value;
                        let trailing_blank = alias_has_trailing_blank(value);
                        let name: String = (**w).into();
                        self.expanding_aliases.push(name);
                        self.alias_stack.push(AliasLayer {
                            text: Cow::Borrowed(value),
                            pos: 0,
                            trailing_blank,
                        });
                        self.alias_depth += 1;
                        self.sync_cache();
                        return self.produce_token_from_bytes();
                    }
                }
            }
        }
        Ok(tok)
    }

    fn produce_token_from_bytes(&mut self) -> Result<Token, ParseError> {
        self.skip_blanks_and_comments();
        match self.peek_byte() {
            None => Ok(Token::Eof),
            Some(b'\n') => {
                self.advance_byte();
                Ok(Token::Newline)
            }
            Some(b'|') => {
                self.advance_byte();
                self.skip_continuations();
                if self.peek_byte() == Some(b'|') {
                    self.advance_byte();
                    Ok(Token::OrIf)
                } else {
                    Ok(Token::Pipe)
                }
            }
            Some(b'&') => {
                self.advance_byte();
                self.skip_continuations();
                if self.peek_byte() == Some(b'&') {
                    self.advance_byte();
                    Ok(Token::AndIf)
                } else {
                    Ok(Token::Amp)
                }
            }
            Some(b';') => {
                self.advance_byte();
                self.skip_continuations();
                match self.peek_byte() {
                    Some(b';') => {
                        self.advance_byte();
                        Ok(Token::DSemi)
                    }
                    Some(b'&') => {
                        self.advance_byte();
                        Ok(Token::SemiAmp)
                    }
                    _ => Ok(Token::Semi),
                }
            }
            Some(b'(') => {
                self.advance_byte();
                Ok(Token::LParen)
            }
            Some(b')') => {
                self.advance_byte();
                Ok(Token::RParen)
            }
            Some(b'<') => self.produce_less_token(),
            Some(b'>') => self.produce_great_token(),
            Some(b) if b.is_ascii_digit() => self.produce_digit_or_word(),
            _ => self.produce_word_token(),
        }
    }

    fn produce_less_token(&mut self) -> Result<Token, ParseError> {
        self.advance_byte();
        self.skip_continuations();
        match self.peek_byte() {
            Some(b'<') => {
                self.advance_byte();
                self.skip_continuations();
                let strip_tabs = if self.peek_byte() == Some(b'-') {
                    self.advance_byte();
                    true
                } else {
                    false
                };
                self.produce_heredoc_line(strip_tabs)
            }
            Some(b'&') => {
                self.advance_byte();
                Ok(Token::LessAnd)
            }
            Some(b'>') => {
                self.advance_byte();
                Ok(Token::LessGreat)
            }
            _ => Ok(Token::Less),
        }
    }

    fn produce_great_token(&mut self) -> Result<Token, ParseError> {
        self.advance_byte();
        self.skip_continuations();
        match self.peek_byte() {
            Some(b'>') => {
                self.advance_byte();
                Ok(Token::DGreat)
            }
            Some(b'&') => {
                self.advance_byte();
                Ok(Token::GreatAnd)
            }
            Some(b'|') => {
                self.advance_byte();
                Ok(Token::Clobber)
            }
            _ => Ok(Token::Great),
        }
    }

    fn produce_heredoc_line(&mut self, first_strip_tabs: bool) -> Result<Token, ParseError> {
        self.skip_blanks();
        let mut first_raw = String::new();
        self.scan_raw_word(&mut first_raw)?;
        if first_raw.is_empty() {
            return Err(self.error("expected heredoc delimiter"));
        }
        let (first_delim, first_expand) = parse_here_doc_delimiter(&first_raw);

        let mut hd_infos: Vec<(Box<str>, bool, bool)> = vec![
            (first_delim, first_strip_tabs, first_expand),
        ];

        let mut line_items: Vec<Result<Token, usize>> = Vec::new();

        loop {
            self.skip_blanks_and_comments();
            match self.peek_byte() {
                None | Some(b'\n') => break,
                Some(b'<') => {
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b'<') => {
                            self.advance_byte();
                            self.skip_continuations();
                            let st = if self.peek_byte() == Some(b'-') {
                                self.advance_byte();
                                true
                            } else {
                                false
                            };
                            self.skip_blanks();
                            let mut dw = String::new();
                            self.scan_raw_word(&mut dw)?;
                            if dw.is_empty() {
                                return Err(self.error("expected heredoc delimiter"));
                            }
                            let (d, e) = parse_here_doc_delimiter(&dw);
                            let idx = hd_infos.len();
                            hd_infos.push((d, st, e));
                            line_items.push(Err(idx));
                        }
                        Some(b'&') => {
                            self.advance_byte();
                            line_items.push(Ok(Token::LessAnd));
                        }
                        Some(b'>') => {
                            self.advance_byte();
                            line_items.push(Ok(Token::LessGreat));
                        }
                        _ => line_items.push(Ok(Token::Less)),
                    }
                }
                Some(b'>') => {
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b'>') => { self.advance_byte(); line_items.push(Ok(Token::DGreat)); }
                        Some(b'&') => { self.advance_byte(); line_items.push(Ok(Token::GreatAnd)); }
                        Some(b'|') => { self.advance_byte(); line_items.push(Ok(Token::Clobber)); }
                        _ => line_items.push(Ok(Token::Great)),
                    }
                }
                Some(b'|') => {
                    self.advance_byte();
                    self.skip_continuations();
                    if self.peek_byte() == Some(b'|') {
                        self.advance_byte();
                        line_items.push(Ok(Token::OrIf));
                    } else {
                        line_items.push(Ok(Token::Pipe));
                    }
                }
                Some(b'&') => {
                    self.advance_byte();
                    self.skip_continuations();
                    if self.peek_byte() == Some(b'&') {
                        self.advance_byte();
                        line_items.push(Ok(Token::AndIf));
                    } else {
                        line_items.push(Ok(Token::Amp));
                    }
                }
                Some(b';') => {
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b';') => { self.advance_byte(); line_items.push(Ok(Token::DSemi)); }
                        Some(b'&') => { self.advance_byte(); line_items.push(Ok(Token::SemiAmp)); }
                        _ => line_items.push(Ok(Token::Semi)),
                    }
                }
                Some(b'(') => { self.advance_byte(); line_items.push(Ok(Token::LParen)); }
                Some(b')') => { self.advance_byte(); line_items.push(Ok(Token::RParen)); }
                Some(b) if b.is_ascii_digit() => {
                    let mut digits = String::new();
                    while let Some(b) = self.peek_byte() {
                        if b.is_ascii_digit() {
                            digits.push(b as char);
                            self.advance_byte();
                        } else {
                            break;
                        }
                    }
                    self.skip_continuations();
                    if matches!(self.peek_byte(), Some(b'<' | b'>')) {
                        if let Ok(fd) = digits.parse::<i32>() {
                            line_items.push(Ok(Token::IoNumber(fd)));
                            continue;
                        }
                    }
                    let mut raw = digits;
                    self.scan_raw_word(&mut raw)?;
                    if !raw.is_empty() {
                        line_items.push(Ok(Token::Word(raw.into())));
                    }
                }
                _ => {
                    let mut raw = String::new();
                    self.scan_raw_word(&mut raw)?;
                    if !raw.is_empty() {
                        line_items.push(Ok(Token::Word(raw.into())));
                    }
                }
            }
        }

        if self.peek_byte() == Some(b'\n') {
            self.advance_byte();
        }

        let mut bodies: Vec<(Box<str>, usize)> = Vec::new();
        for (delim, strip_tabs, expand) in &hd_infos {
            let body_line = self.line;
            let body: Box<str> = self.read_here_doc_body(delim, *strip_tabs, *expand)?.into();
            bodies.push((body, body_line));
        }

        for item in line_items {
            match item {
                Ok(tok) => self.token_queue.push_back(tok),
                Err(idx) => {
                    let (ref body, body_line) = bodies[idx];
                    self.token_queue.push_back(Token::HereDoc {
                        strip_tabs: hd_infos[idx].1,
                        expand: hd_infos[idx].2,
                        delimiter: hd_infos[idx].0.clone(),
                        body: body.clone(),
                        body_line,
                    });
                }
            }
        }
        self.token_queue.push_back(Token::Newline);

        let (ref body, body_line) = bodies[0];
        Ok(Token::HereDoc {
            strip_tabs: hd_infos[0].1,
            expand: hd_infos[0].2,
            delimiter: hd_infos[0].0.clone(),
            body: body.clone(),
            body_line,
        })
    }

    fn produce_digit_or_word(&mut self) -> Result<Token, ParseError> {
        let mut digits = String::new();
        while let Some(b) = self.peek_byte() {
            if b.is_ascii_digit() {
                digits.push(b as char);
                self.advance_byte();
            } else {
                break;
            }
        }
        self.skip_continuations();
        if matches!(self.peek_byte(), Some(b'<' | b'>')) {
            if let Ok(fd) = digits.parse::<i32>() {
                return Ok(Token::IoNumber(fd));
            }
        }
        self.produce_word_with_prefix(digits)
    }

    fn produce_word_token(&mut self) -> Result<Token, ParseError> {
        self.produce_word_with_prefix(String::new())
    }

    fn produce_word_with_prefix(
        &mut self,
        prefix: String,
    ) -> Result<Token, ParseError> {
        let check_keyword = self.keyword_mode;
        let check_alias = self.keyword_mode || self.alias_trailing_blank_pending;
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        let mut raw = prefix;
        loop {
            if raw.is_empty() {
                if self.cached_byte.is_none()
                    || matches!(self.peek_byte(), Some(b) if is_delim(b))
                {
                    return Ok(Token::Eof);
                }
            }

            let had_quote = self.scan_raw_word(&mut raw)?;
            if raw.is_empty() {
                return Ok(Token::Eof);
            }

            if !had_quote {
                if check_alias {
                    if let Some(value) = self.aliases.get(&*raw) {
                        if is_alias_word(&raw)
                            && !self.expanding_aliases.iter().any(|n| n == &*raw)
                            && self.alias_depth < 1024
                        {
                            let value: &str = value;
                            let trailing_blank = alias_has_trailing_blank(value);
                            let name: String = raw.clone();
                            self.expanding_aliases.push(name);
                            self.alias_stack.push(AliasLayer {
                                text: Cow::Borrowed(value),
                                pos: 0,
                                trailing_blank,
                            });
                            self.alias_depth += 1;
                            self.sync_cache();
                            raw.clear();
                            self.skip_blanks_and_comments();
                            continue;
                        }
                    }
                }
                if check_keyword {
                    if let Some(kw_tok) = word_to_keyword_token(&raw) {
                        return Ok(kw_tok);
                    }
                }
            }

            return Ok(Token::Word(raw.into()));
        }
    }

    fn scan_raw_word(&mut self, raw: &mut String) -> Result<bool, ParseError> {
        let mut had_quote = false;
        loop {
            match self.peek_byte() {
                None => break,
                Some(b) if is_word_break(b) => break,
                Some(b'#') if raw.is_empty() => break,
                Some(b'\\') => {
                    self.advance_byte();
                    match self.peek_byte() {
                        Some(b'\n') => {
                            self.advance_byte();
                            if raw.is_empty() {
                                self.skip_blanks_and_comments();
                                if self.cached_byte.is_none()
                                    || matches!(self.peek_byte(), Some(b) if is_delim(b))
                                {
                                    break;
                                }
                            }
                        }
                        Some(b) => {
                            raw.push('\\');
                            raw.push(b as char);
                            self.advance_byte();
                            had_quote = true;
                        }
                        None => {
                            raw.push('\\');
                            had_quote = true;
                        }
                    }
                }
                Some(b'\'') => {
                    had_quote = true;
                    raw.push('\'');
                    self.advance_byte();
                    self.consume_single_quote(raw)?;
                }
                Some(b'"') => {
                    had_quote = true;
                    raw.push('"');
                    self.advance_byte();
                    self.consume_double_quote(raw)?;
                }
                Some(b'$') => {
                    raw.push('$');
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b'\'') => {
                            had_quote = true;
                            self.consume_dollar_single_quote(raw)?;
                        }
                        _ => {
                            self.consume_dollar_construct(raw)?;
                        }
                    }
                }
                Some(b'`') => {
                    raw.push('`');
                    self.advance_byte();
                    had_quote = true;
                    self.consume_backtick_inner(raw)?;
                }
                Some(b) => {
                    raw.push(b as char);
                    self.advance_byte();
                }
            }
        }
        Ok(had_quote)
    }

    fn eat_keyword(&mut self, expected: Token, name: &str) -> Result<(), ParseError> {
        self.set_keyword_mode(true);
        if *self.peek_token()? == expected {
            self.advance_token();
            self.skip_linebreaks_t()?;
            Ok(())
        } else {
            Err(self.error(format!("expected '{name}'")))
        }
    }

    fn skip_separators_t(&mut self) -> Result<(), ParseError> {
        loop {
            self.set_keyword_mode(true);
            match self.peek_token()? {
                Token::Newline | Token::Semi => {
                    self.advance_token();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn skip_linebreaks_t(&mut self) -> Result<(), ParseError> {
        loop {
            self.set_keyword_mode(true);
            match self.peek_token()? {
                Token::Newline => {
                    self.advance_token();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn take_word(&mut self) -> Box<str> {
        match self.advance_token() {
            Token::Word(w) => w,
            _ => unreachable!("expected Word token"),
        }
    }

    fn parse_program_until(
        &mut self,
        stop: fn(&Token) -> bool,
        stop_on_closer: bool,
        stop_on_dsemi: bool,
    ) -> Result<Program, ParseError> {
        let mut items = Vec::new();
        self.skip_separators_t()?;

        loop {
            self.set_keyword_mode(true);

            if stop_on_dsemi
                && matches!(self.peek_token()?, Token::DSemi | Token::SemiAmp)
            {
                break;
            }

            {
                let tok = self.peek_token()?;
                if stop(tok) {
                    break;
                }
                if stop_on_closer && matches!(tok, Token::RBrace | Token::RParen) {
                    break;
                }
                match tok {
                    Token::Eof => break,
                    _ => {}
                }
            }

            let line = self.current_line();
            let and_or = self.parse_and_or()?;
            let asynchronous = matches!(self.peek_token()?, Token::Amp);
            if asynchronous {
                self.advance_token();
            }
            self.skip_separators_t()?;

            items.push(ListItem {
                and_or,
                asynchronous,
                line,
            });
        }

        Ok(Program {
            items: items.into_boxed_slice(),
        })
    }

    fn parse_and_or(&mut self) -> Result<AndOr, ParseError> {
        let first = self.parse_pipeline()?;
        let mut rest = Vec::new();
        loop {
            let op = match self.peek_token()? {
                Token::OrIf => {
                    self.advance_token();
                    LogicalOp::Or
                }
                Token::AndIf => {
                    self.advance_token();
                    LogicalOp::And
                }
                _ => break,
            };
            self.skip_linebreaks_t()?;
            let rhs = self.parse_pipeline()?;
            rest.push((op, rhs));
        }
        Ok(AndOr {
            first,
            rest: rest.into_boxed_slice(),
        })
    }

    fn parse_pipeline(&mut self) -> Result<Pipeline, ParseError> {
        self.set_keyword_mode(true);

        let timed = if matches!(self.peek_token()?, Token::Word(w) if &**w == "time") {
            self.advance_token();
            self.set_keyword_mode(true);
            if matches!(self.peek_token()?, Token::Word(w) if &**w == "-p") {
                self.advance_token();
                self.set_keyword_mode(true);
                TimedMode::Posix
            } else {
                TimedMode::Default
            }
        } else {
            TimedMode::Off
        };

        let negated = if matches!(self.peek_token()?, Token::Bang) {
            self.advance_token();
            self.set_keyword_mode(true);
            true
        } else {
            false
        };

        let mut commands = vec![self.parse_command()?];
        loop {
            match self.peek_token()? {
                Token::Pipe => {
                    self.advance_token();
                    self.skip_linebreaks_t()?;
                    commands.push(self.parse_command()?);
                }
                _ => break,
            }
        }

        Ok(Pipeline {
            negated,
            timed,
            commands: commands.into_boxed_slice(),
        })
    }

    fn parse_command(&mut self) -> Result<Command, ParseError> {
        self.set_keyword_mode(true);
        let command = self.parse_command_inner()?;
        self.parse_command_redirections(command)
    }

    fn parse_command_inner(&mut self) -> Result<Command, ParseError> {
        let line = self.current_line();
        match self.peek_token()? {
            Token::If => {
                self.advance_token();
                self.parse_if_command()
            }
            Token::While => {
                self.advance_token();
                self.parse_loop_command(LoopKind::While)
            }
            Token::Until => {
                self.advance_token();
                self.parse_loop_command(LoopKind::Until)
            }
            Token::For => {
                self.advance_token();
                self.parse_for_command()
            }
            Token::Case => {
                self.advance_token();
                self.parse_case_command()
            }
            Token::Function => {
                self.advance_token();
                self.parse_function_keyword()
            }
            Token::LBrace => {
                self.advance_token();
                self.skip_separators_t()?;
                let body = self.parse_program_until(|_| false, true, false)?;
                if !matches!(self.peek_token()?, Token::RBrace) {
                    return Err(self.error("expected '}'"));
                }
                self.advance_token();
                self.skip_separators_t()?;
                Ok(Command::Group(body))
            }
            Token::LParen => {
                self.advance_token();
                let body = self.parse_program_until(|_| false, true, false)?;
                if !matches!(self.peek_token()?, Token::RParen) {
                    return Err(self.error("expected ')' to close subshell"));
                }
                self.advance_token();
                Ok(Command::Subshell(body))
            }
            Token::Bang => Err(self.error("expected command")),
            Token::Word(_) => {
                let raw = self.take_word();
                self.set_keyword_mode(false);
                if is_name(&raw) && matches!(self.peek_token()?, Token::LParen) {
                    self.advance_token();
                    if matches!(self.peek_token()?, Token::RParen) {
                        self.advance_token();
                        self.skip_linebreaks_t().ok();
                        let body = self.parse_command()?;
                        return Ok(Command::FunctionDef(FunctionDef {
                            name: raw,
                            body: Box::new(body),
                        }));
                    }
                    return Err(
                        self.error("syntax error near unexpected token `('"),
                    );
                }
                self.parse_simple_command_with_first_word(raw, line)
                    .map(Command::Simple)
            }
            Token::IoNumber(_) | Token::Less | Token::Great | Token::DGreat
            | Token::LessAnd | Token::GreatAnd | Token::LessGreat
            | Token::Clobber | Token::HereDoc { .. } => self
                .parse_simple_command_with_first_redir()
                .map(Command::Simple),
            Token::Eof => Err(self.error("expected command")),
            Token::Newline | Token::Semi | Token::DSemi | Token::SemiAmp
            | Token::Amp | Token::Pipe | Token::OrIf | Token::AndIf
            | Token::RParen => Err(self.error("expected command")),
            _ => {
                let name = token_to_keyword_name(self.peek_token()?);
                self.advance_token();
                self.parse_simple_command_with_first_word(name, line)
                    .map(Command::Simple)
            }
        }
    }

    fn parse_simple_command_with_first_word(
        &mut self,
        first_raw: Box<str>,
        first_line: usize,
    ) -> Result<SimpleCommand, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirections = Vec::new();

        if let Some((name, value_raw)) = split_assignment(&first_raw) {
            assignments.push(Assignment {
                name: name.into(),
                value: Word {
                    raw: value_raw.into(),
                    line: first_line,
                },
            });
        } else {
            words.push(Word {
                raw: first_raw,
                line: first_line,
            });
        }

        self.simple_command_scan_loop(
            &mut assignments,
            &mut words,
            &mut redirections,
        )?;

        if words.is_empty() && assignments.is_empty() && redirections.is_empty()
        {
            return Err(self.error("expected command"));
        }

        if !words.is_empty() && matches!(self.peek_token()?, Token::LParen) {
            return Err(
                self.error("syntax error near unexpected token `('"),
            );
        }

        Ok(SimpleCommand {
            assignments: assignments.into_boxed_slice(),
            words: words.into_boxed_slice(),
            redirections: redirections.into_boxed_slice(),
        })
    }

    fn parse_simple_command_with_first_redir(
        &mut self,
    ) -> Result<SimpleCommand, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirections = Vec::new();

        if let Some(redir) = self.try_parse_redirection()? {
            redirections.push(redir);
        }

        self.simple_command_scan_loop(
            &mut assignments,
            &mut words,
            &mut redirections,
        )?;

        if words.is_empty() && assignments.is_empty() && redirections.is_empty()
        {
            return Err(self.error("expected command"));
        }

        Ok(SimpleCommand {
            assignments: assignments.into_boxed_slice(),
            words: words.into_boxed_slice(),
            redirections: redirections.into_boxed_slice(),
        })
    }

    fn simple_command_scan_loop(
        &mut self,
        assignments: &mut Vec<Assignment>,
        words: &mut Vec<Word>,
        redirections: &mut Vec<Redirection>,
    ) -> Result<(), ParseError> {
        loop {
            let at_command_pos = words.is_empty()
                && (!assignments.is_empty() || !redirections.is_empty());
            self.set_keyword_mode(at_command_pos);
            let line = self.current_line();

            if let Some(redir) = self.try_parse_redirection()? {
                redirections.push(redir);
                continue;
            }

            match self.peek_token()? {
                Token::Word(_) => {}
                _ => break,
            }

            let raw = self.take_word();
            if words.is_empty() {
                if let Some((name, value_raw)) = split_assignment(&raw) {
                    assignments.push(Assignment {
                        name: name.into(),
                        value: Word {
                            raw: value_raw.into(),
                            line,
                        },
                    });
                    continue;
                }
            }
            words.push(Word { raw, line });
        }
        Ok(())
    }

    fn try_parse_redirection(
        &mut self,
    ) -> Result<Option<Redirection>, ParseError> {
        match self.peek_token()? {
            Token::IoNumber(_)
            | Token::Less
            | Token::Great
            | Token::DGreat
            | Token::LessAnd
            | Token::GreatAnd
            | Token::LessGreat
            | Token::Clobber
            | Token::HereDoc { .. } => {}
            _ => return Ok(None),
        }

        let mut fd: Option<i32> = None;
        if matches!(self.peek_token()?, Token::IoNumber(_)) {
            if let Token::IoNumber(n) = self.advance_token() {
                fd = Some(n);
            }
        }

        let line = self.current_line();
        let _ = self.peek_token()?;
        let tok = self.advance_token();
        match tok {
            Token::HereDoc {
                delimiter,
                body,
                strip_tabs,
                expand,
                body_line,
            } => Ok(Some(Redirection {
                fd,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: delimiter.clone(),
                    line,
                },
                here_doc: Some(HereDoc {
                    delimiter,
                    body,
                    expand,
                    strip_tabs,
                    body_line,
                }),
            })),
            Token::Less | Token::Great | Token::DGreat | Token::LessAnd
            | Token::GreatAnd | Token::LessGreat | Token::Clobber => {
                let kind = match tok {
                    Token::Less => RedirectionKind::Read,
                    Token::Great => RedirectionKind::Write,
                    Token::DGreat => RedirectionKind::Append,
                    Token::LessAnd => RedirectionKind::DupInput,
                    Token::GreatAnd => RedirectionKind::DupOutput,
                    Token::LessGreat => RedirectionKind::ReadWrite,
                    Token::Clobber => RedirectionKind::ClobberWrite,
                    _ => unreachable!(),
                };
                self.set_keyword_mode(false);
                let target_line = self.current_line();
                match self.peek_token()? {
                    Token::Word(_) => {
                        let w = self.take_word();
                        Ok(Some(Redirection {
                            fd,
                            kind,
                            target: Word {
                                raw: w,
                                line: target_line,
                            },
                            here_doc: None,
                        }))
                    }
                    _ => Err(self.error("expected redirection target")),
                }
            }
            _ => unreachable!("peek guaranteed a redirect token"),
        }
    }

    fn parse_command_redirections(
        &mut self,
        command: Command,
    ) -> Result<Command, ParseError> {
        if matches!(command, Command::Simple(_)) {
            return Ok(command);
        }
        let mut redirections = Vec::new();
        while let Some(redir) = self.try_parse_redirection()? {
            redirections.push(redir);
        }
        if redirections.is_empty() {
            Ok(command)
        } else {
            Ok(Command::Redirected(
                Box::new(command),
                redirections.into_boxed_slice(),
            ))
        }
    }

    fn parse_if_command(&mut self) -> Result<Command, ParseError> {
        let condition = self.parse_program_until(
            |tok| matches!(tok, Token::Then),
            false,
            false,
        )?;
        if condition.items.is_empty() {
            return Err(self.error("expected command list after 'if'"));
        }
        self.eat_keyword(Token::Then, "then")?;

        fn at_elif_else_fi(tok: &Token) -> bool {
            matches!(tok, Token::Elif | Token::Else | Token::Fi)
        }
        let then_branch =
            self.parse_program_until(at_elif_else_fi, false, false)?;
        let mut elif_branches = Vec::new();

        self.set_keyword_mode(true);
        while matches!(self.peek_token()?, Token::Elif) {
            self.advance_token();
            self.skip_separators_t()?;
            let cond = self.parse_program_until(
                |tok| matches!(tok, Token::Then),
                false,
                false,
            )?;
            if cond.items.is_empty() {
                return Err(self.error("expected command list after 'elif'"));
            }
            self.eat_keyword(Token::Then, "then")?;
            let body =
                self.parse_program_until(at_elif_else_fi, false, false)?;
            elif_branches.push(ElifBranch {
                condition: cond,
                body,
            });
            self.set_keyword_mode(true);
        }

        let else_branch =
            if matches!(self.peek_token()?, Token::Else) {
                self.advance_token();
                self.skip_separators_t()?;
                Some(self.parse_program_until(
                    |tok| matches!(tok, Token::Fi),
                    false,
                    false,
                )?)
            } else {
                None
            };

        self.eat_keyword(Token::Fi, "fi")?;
        Ok(Command::If(IfCommand {
            condition,
            then_branch,
            elif_branches: elif_branches.into_boxed_slice(),
            else_branch,
        }))
    }

    fn parse_loop_command(
        &mut self,
        kind: LoopKind,
    ) -> Result<Command, ParseError> {
        let keyword = match kind {
            LoopKind::While => "while",
            LoopKind::Until => "until",
        };
        let condition = self.parse_program_until(
            |tok| matches!(tok, Token::Do),
            false,
            false,
        )?;
        if condition.items.is_empty() {
            return Err(self.error(format!(
                "expected command list after '{keyword}'"
            )));
        }
        self.eat_keyword(Token::Do, "do")?;
        let body = self.parse_program_until(
            |tok| matches!(tok, Token::Done),
            false,
            false,
        )?;
        self.eat_keyword(Token::Done, "done")?;
        Ok(Command::Loop(LoopCommand {
            kind,
            condition,
            body,
        }))
    }

    fn parse_for_command(&mut self) -> Result<Command, ParseError> {
        self.set_keyword_mode(false);
        let name = match self.peek_token()? {
            Token::Word(_) => self.take_word(),
            _ => return Err(self.error("expected for loop variable name")),
        };
        if !is_name(&name) {
            return Err(self.error("expected for loop variable name"));
        }

        self.skip_linebreaks_t()?;
        self.set_keyword_mode(true);
        let items = if matches!(self.peek_token()?, Token::In) {
            self.advance_token();
            let mut items = Vec::new();
            self.set_keyword_mode(false);
            loop {
                match self.peek_token()? {
                    Token::Newline | Token::Semi | Token::Eof => break,
                    Token::Word(_) => {
                        let word_line = self.current_line();
                        let w = self.take_word();
                        items.push(Word {
                            raw: w,
                            line: word_line,
                        });
                    }
                    _ => break,
                }
            }
            Some(items.into_boxed_slice())
        } else {
            None
        };

        self.skip_separators_t()?;
        self.eat_keyword(Token::Do, "do")?;
        let body = self.parse_program_until(
            |tok| matches!(tok, Token::Done),
            false,
            false,
        )?;
        self.eat_keyword(Token::Done, "done")?;
        Ok(Command::For(ForCommand { name, items, body }))
    }

    fn parse_case_command(&mut self) -> Result<Command, ParseError> {
        self.set_keyword_mode(false);
        let line = self.current_line();
        let word_raw = match self.peek_token()? {
            Token::Word(_) => self.take_word(),
            _ => return Err(self.error("expected case word")),
        };
        let word = Word {
            raw: word_raw,
            line,
        };

        self.skip_linebreaks_t()?;
        self.eat_keyword(Token::In, "in")?;
        self.skip_linebreaks_t()?;

        let mut arms = Vec::new();
        loop {
            self.set_keyword_mode(true);
            if matches!(self.peek_token()?, Token::Esac | Token::Eof) {
                break;
            }

            if matches!(self.peek_token()?, Token::LParen) {
                self.advance_token();
            }

            let mut patterns = Vec::new();
            loop {
                self.set_keyword_mode(false);
                let pat_line = self.current_line();
                match self.peek_token()? {
                    Token::Word(_) => {
                        let w = self.take_word();
                        patterns.push(Word {
                            raw: w,
                            line: pat_line,
                        });
                    }
                    _ => {
                        return Err(self.error("expected case pattern"))
                    }
                }

                if matches!(self.peek_token()?, Token::Pipe) {
                    self.advance_token();
                    continue;
                }
                break;
            }

            if !matches!(self.peek_token()?, Token::RParen) {
                return Err(
                    self.error("expected ')' after case pattern"),
                );
            }
            self.advance_token();
            self.skip_separators_t()?;

            let body = self.parse_program_until(
                |tok| matches!(tok, Token::Esac),
                false,
                true,
            )?;

            self.set_keyword_mode(true);
            let (fallthrough, sep_kind) = match self.peek_token()? {
                Token::DSemi => {
                    self.advance_token();
                    (false, 0u8)
                }
                Token::SemiAmp => {
                    self.advance_token();
                    (true, 1)
                }
                Token::Semi => {
                    self.advance_token();
                    (false, 2)
                }
                _ => (false, 3),
            };

            arms.push(CaseArm {
                patterns: patterns.into_boxed_slice(),
                body,
                fallthrough,
            });

            match sep_kind {
                0 | 1 => {
                    self.skip_separators_t()?;
                }
                2 => {
                    self.set_keyword_mode(true);
                    if !matches!(self.peek_token()?, Token::Esac) {
                        return Err(self.error(
                            "expected ';;', ';&', or 'esac'",
                        ));
                    }
                }
                _ => {
                    self.set_keyword_mode(true);
                    if !matches!(self.peek_token()?, Token::Esac) {
                        break;
                    }
                }
            }
        }

        self.eat_keyword(Token::Esac, "esac")?;
        Ok(Command::Case(CaseCommand {
            word,
            arms: arms.into_boxed_slice(),
        }))
    }

    fn parse_function_keyword(&mut self) -> Result<Command, ParseError> {
        self.set_keyword_mode(false);
        let name = match self.peek_token()? {
            Token::Word(_) => self.take_word(),
            _ => return Err(self.error("expected function name")),
        };
        if !is_name(&name) {
            return Err(self.error("expected function name"));
        }
        if matches!(self.peek_token()?, Token::LParen) {
            self.advance_token();
            if matches!(self.peek_token()?, Token::RParen) {
                self.advance_token();
            }
        }
        self.skip_linebreaks_t().ok();
        let body = self.parse_command()?;
        Ok(Command::FunctionDef(FunctionDef {
            name,
            body: Box::new(body),
        }))
    }

    fn next_complete_command(
        &mut self,
    ) -> Result<Option<Program>, ParseError> {
        self.skip_separators_t()?;
        if matches!(self.peek_token()?, Token::Eof) {
            return Ok(None);
        }
        let mut items = Vec::new();
        loop {
            self.set_keyword_mode(true);
            if matches!(self.peek_token()?, Token::Eof) {
                break;
            }
            let line = self.current_line();
            let and_or = self.parse_and_or()?;
            let asynchronous = matches!(self.peek_token()?, Token::Amp);
            if asynchronous {
                self.advance_token();
            }

            let at_newline = matches!(self.peek_token()?, Token::Newline);
            if at_newline {
                self.advance_token();
            } else if matches!(self.peek_token()?, Token::Semi) {
                self.advance_token();
            }

            items.push(ListItem {
                and_or,
                asynchronous,
                line,
            });

            self.set_keyword_mode(true);
            if at_newline || matches!(self.peek_token()?, Token::Eof) {
                break;
            }
        }
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(Program {
            items: items.into_boxed_slice(),
        }))
    }
}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    parse_with_aliases(source, &HashMap::new())
}

pub fn parse_with_aliases(
    source: &str,
    aliases: &HashMap<Box<str>, Box<str>>,
) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source, aliases);
    parser.parse_program_until(|_| false, false, false)
}

struct SavedAliasState {
    layers: Vec<AliasLayer<'static>>,
    depth: usize,
    expanding: Vec<String>,
    trailing_blank_pending: bool,
}

pub struct ParseSession<'src> {
    source: &'src str,
    pos: usize,
    line: usize,
    saved_alias: Option<SavedAliasState>,
}

impl<'src> ParseSession<'src> {
    pub fn new(source: &'src str) -> Result<Self, ParseError> {
        Ok(Self {
            source,
            pos: 0,
            line: 1,
            saved_alias: None,
        })
    }

    pub fn next_command(
        &mut self,
        aliases: &HashMap<Box<str>, Box<str>>,
    ) -> Result<Option<Program>, ParseError> {
        let mut parser = Parser::new_at(self.source, self.pos, self.line, aliases);

        if let Some(saved) = self.saved_alias.take() {
            for layer in saved.layers {
                parser.alias_stack.push(layer);
            }
            parser.alias_depth = saved.depth;
            parser.expanding_aliases = saved.expanding;
            parser.alias_trailing_blank_pending = saved.trailing_blank_pending;
            parser.sync_cache();
        }

        let result = parser.next_complete_command();

        self.pos = parser.pos;
        self.line = parser.line;

        if parser.alias_stack.is_empty() {
            self.saved_alias = None;
        } else {
            let layers = parser
                .alias_stack
                .into_iter()
                .map(|layer| AliasLayer {
                    text: Cow::Owned(layer.text.into_owned()),
                    pos: layer.pos,
                    trailing_blank: layer.trailing_blank,
                })
                .collect();
            self.saved_alias = Some(SavedAliasState {
                layers,
                depth: parser.alias_depth,
                expanding: parser.expanding_aliases,
                trailing_blank_pending: parser.alias_trailing_blank_pending,
            });
        }

        result
    }

    pub fn current_line(&self) -> usize {
        self.line
    }
}

fn split_assignment(input: &str) -> Option<(&str, &str)> {
    let (name, value) = input.split_once('=')?;
    if !is_name(name) {
        return None;
    }
    Some((name, value))
}

fn parse_here_doc_delimiter(raw: &str) -> (Box<str>, bool) {
    let mut delimiter = String::new();
    let mut index = 0usize;
    let mut expand = true;
    let bytes = raw.as_bytes();

    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                expand = false;
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\'' {
                        index += 1;
                        break;
                    }
                    delimiter.push(bytes[index] as char);
                    index += 1;
                }
            }
            b'"' => {
                expand = false;
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'"' => {
                            index += 1;
                            break;
                        }
                        b'\\' if index + 1 < bytes.len() => {
                            let next = bytes[index + 1];
                            if matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n') {
                                index += 1;
                                delimiter.push(bytes[index] as char);
                                index += 1;
                            } else {
                                delimiter.push(b'\\' as char);
                                index += 1;
                            }
                        }
                        ch => {
                            delimiter.push(ch as char);
                            index += 1;
                        }
                    }
                }
            }
            b'$' if index + 1 < bytes.len() && bytes[index + 1] == b'\'' => {
                expand = false;
                index += 2;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\'' => {
                            index += 1;
                            break;
                        }
                        b'\\' if index + 1 < bytes.len() => {
                            index += 1;
                            delimiter.push(bytes[index] as char);
                            index += 1;
                        }
                        ch => {
                            delimiter.push(ch as char);
                            index += 1;
                        }
                    }
                }
            }
            b'\\' => {
                expand = false;
                index += 1;
                if index < bytes.len() {
                    delimiter.push(bytes[index] as char);
                    index += 1;
                }
            }
            ch => {
                delimiter.push(ch as char);
                index += 1;
            }
        }
    }

    (delimiter.into_boxed_str(), expand)
}

pub fn is_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && BYTE_CLASS[bytes[0] as usize] & BC_NAME_START != 0
        && bytes[1..].iter().fold(0xFFu8, |acc, &b| acc & BYTE_CLASS[b as usize])
            & BC_NAME_CONT
            != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test(source: &str) -> Result<Program, ParseError> {
        parse(source)
    }

    fn parse_with_aliases_test(
        source: &str,
        aliases: &HashMap<Box<str>, Box<str>>,
    ) -> Result<Program, ParseError> {
        parse_with_aliases(source, aliases)
    }

    #[test]
    fn parses_simple_pipeline() {
        let program = parse_test("echo hi | wc -c").expect("parse");
        assert_eq!(program.items.len(), 1);
        assert_eq!(program.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn parses_assignments_and_groups() {
        let program = parse_test("FOO=bar echo \"$FOO\"").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.assignments.len() == 1 && &*cmd.words[0].raw == "echo"
        ));
    }

    #[test]
    fn parses_logical_and_subshell_forms() {
        let program = parse_test("(echo ok) && echo done || echo fail").expect("parse");
        let and_or = &program.items[0].and_or;
        assert_eq!(and_or.rest.len(), 2);
        assert!(matches!(
            and_or.first.commands.first(),
            Some(Command::Subshell(_))
        ));

        let linebreak_and_or =
            parse_test("true &&\n echo done ||\n echo fail").expect("parse linebreak and-or");
        assert_eq!(linebreak_and_or.items[0].and_or.rest.len(), 2);
    }

    #[test]
    fn tokenizes_terminated_single_quotes() {
        let program = parse_test("echo 'ok'").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == "'ok'"
        ));
    }

    #[test]
    fn parses_case_arm_without_trailing_dsemi_before_esac() {
        let program = parse_test("case x in x) esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_cmd) if case_cmd.arms.len() == 1
        ));
    }

    #[test]
    fn parses_heredoc_operator_shape() {
        let program = parse_test("cat <<EOF\nhello $USER\nEOF\n").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 1
                    && cmd.redirections[0].kind == RedirectionKind::HereDoc
                    && &*cmd.redirections[0].target.raw == "EOF"
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some("hello $USER\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse_test("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.delimiter) == Some("EOF")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse_test("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some("\tone\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.strip_tabs) == Some(true)
        ));
    }

    #[test]
    fn parses_extended_redirection_forms() {
        let program =
            parse_test("cat 3<in 2>out 4>>log 5<>rw 0<&3 1>&2 2>|force").expect("parse");
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
        let program = parse_test("{ echo hi; } >out").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Redirected(inner, redirections)
                if matches!(inner.as_ref(), Command::Group(_))
                    && redirections.len() == 1
                    && redirections[0].kind == RedirectionKind::Write
                    && &*redirections[0].target.raw == "out"
        ));

        let not_a_group = parse_test("{echo hi; }").expect("parse brace word");
        assert!(matches!(
            &not_a_group.items[0].and_or.first.commands[0],
            Command::Simple(simple) if &*simple.words[0].raw == "{echo"
        ));

        let closer_literal = parse_test("echo }").expect("parse literal closer");
        assert!(matches!(
            &closer_literal.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["echo", "}"]
        ));
    }

    #[test]
    fn parses_function_definition() {
        let program = parse_test("greet() { echo hi; }").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(function) if &*function.name == "greet"
        ));
        assert!(parse_test("if() { echo hi; }").is_err());
    }

    #[test]
    fn parses_negated_async_pipeline() {
        let program = parse_test("! echo ok | wc -c &").expect("parse");
        let item = &program.items[0];
        assert!(item.asynchronous);
        assert!(item.and_or.first.negated);
        assert_eq!(item.and_or.first.commands.len(), 2);

        let linebreak_pipeline =
            parse_test("printf ok |\n wc -c").expect("parse linebreak pipeline");
        assert_eq!(linebreak_pipeline.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn rejects_invalid_empty_command() {
        let error = parse_test("| wc").expect_err("parse should fail");
        assert_eq!(&*error.message, "expected command");

        let error = parse_test("echo hi | ! cat").expect_err("bang after pipe should fail");
        assert_eq!(&*error.message, "expected command");
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = parse_test("echo 'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated single quote");
    }

    #[test]
    fn rejects_unterminated_dollar_single_quote() {
        let error = parse_test("echo $'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated dollar-single-quotes");
        let error = parse_test(r"echo $'backslash at end\").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated dollar-single-quotes");
    }

    #[test]
    fn parses_if_with_elif_and_else() {
        let program =
            parse_test("if true; then echo yes; elif false; then echo no; else echo maybe; fi")
                .expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(if_command)
                if if_command.elif_branches.len() == 1 && if_command.else_branch.is_some()
        ));

        let simple_if = parse_test("if true; then echo yes; fi").expect("parse");
        assert!(matches!(
            &simple_if.items[0].and_or.first.commands[0],
            Command::If(if_command) if if_command.else_branch.is_none()
        ));
    }

    #[test]
    fn parses_while_and_until_loops() {
        let while_program = parse_test("while true; do echo yes; done").expect("parse");
        assert!(matches!(
            while_program.items[0].and_or.first.commands[0],
            Command::Loop(LoopCommand {
                kind: LoopKind::While,
                ..
            })
        ));

        let until_program = parse_test("until false; do echo yes; done").expect("parse");
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
        let explicit = parse_test("for item in a b c; do echo $item; done").expect("parse");
        assert!(matches!(
            &explicit.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if &*for_command.name == "item" && for_command.items.as_ref().map(|s| s.len()) == Some(3)
        ));

        let positional = parse_test("for item; do echo $item; done").expect("parse");
        assert!(matches!(
            &positional.items[0].and_or.first.commands[0],
            Command::For(for_command) if &*for_command.name == "item" && for_command.items.is_none()
        ));

        let linebreak_before_in =
            parse_test("for item\nin a b; do echo $item; done").expect("parse linebreak before in");
        assert!(matches!(
            &linebreak_before_in.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if &*for_command.name == "item"
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                        == Some(vec!["a", "b"])
        ));

        let reserved_words_as_items = parse_test("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                    == Some(vec!["do", "done"])
        ));
    }

    #[test]
    fn parses_case_commands() {
        let program =
            parse_test("case $name in foo|bar) echo hit ;; baz) echo miss ;; esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_command)
                if &*case_command.word.raw == "$name"
                    && case_command.arms.len() == 2
                    && case_command.arms[0].patterns.len() == 2
        ));

        let with_optional_paren = parse_test("case x in (x) echo ok ;; esac").expect("parse");
        assert!(matches!(
            &with_optional_paren.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let with_linebreak_before_in =
            parse_test("case x\nin\nx) echo ok ;;\nesac").expect("parse case linebreak");
        assert!(matches!(
            &with_linebreak_before_in.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let empty_after_linebreak =
            parse_test("case x\nin\nesac").expect("parse empty case after linebreak");
        assert!(matches!(
            &empty_after_linebreak.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.is_empty()
        ));

        let fallthrough =
            parse_test("case a in a) echo one ;& b) echo two ;; esac").expect("parse fallthrough");
        if let Command::Case(c) = &fallthrough.items[0].and_or.first.commands[0] {
            assert_eq!(c.arms.len(), 2);
            assert!(c.arms[0].fallthrough);
            assert!(!c.arms[1].fallthrough);
        } else {
            panic!("expected Case");
        }

        let ft_clone = fallthrough.items[0].and_or.first.commands[0].clone();
        assert!(matches!(ft_clone, Command::Case(ref cc) if cc.arms[0].fallthrough));
    }

    #[test]
    fn parser_covers_misc_error_and_token_paths() {
        assert_eq!(
            format!(
                "{}",
                ParseError {
                    message: "x".into(),
                    line: None,
                }
            ),
            "x"
        );
        assert!(parse_test("echo \"unterminated").is_err());
        assert!(parse_test("cat <").is_err());
        assert!(parse_test("for 1 in a; do echo hi; done").is_err());
        assert!(parse_test("for item in ; do echo hi; done").is_ok());
        assert!(parse_test("case x in ; esac").is_err());
        assert!(parse_test("cat <<EOF").is_err());
        assert!(parse_test("echo 2>&").is_err());
        assert!(parse_test("if true; echo hi; fi").is_err());
        assert!(parse_test("while true; echo hi; done").is_err());
        assert!(parse_test("# comment only\n").is_ok());
        assert!(parse_test("echo foo\\ bar").is_ok());
        assert!(parse_test("echo \"a\\\"b\"").is_ok());
        assert!(parse_test("printf ok |\n wc -c").is_ok());
        assert!(parse_test("true &&\n echo ok").is_ok());
        assert!(parse_test("false ||\n echo ok").is_ok());
    }

    #[test]
    fn parse_session_uses_updated_aliases_between_commands() {
        let mut session =
            ParseSession::new("alias setok='printf ok'\nsetok\n").expect("session");
        let first = session
            .next_command(&HashMap::new())
            .expect("first cmd")
            .expect("some cmd");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(first.items[0].and_or.first.commands[0], Command::Simple(_)));

        let second = session
            .next_command(&HashMap::from([(
                Box::from("setok"),
                Box::from("printf ok"),
            )]))
            .expect("second cmd")
            .expect("some cmd");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "ok"]
        ));

        assert!(session.next_command(&HashMap::new()).expect("eof").is_none());
    }

    #[test]
    fn alias_expansion_in_simple_commands() {
        let mut aliases = HashMap::new();
        aliases.insert("say".into(), "printf hi".into());
        let program = parse_with_aliases_test("say", &aliases).expect("parse alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "hi"]
        ));

        let mut aliases = HashMap::new();
        aliases.insert("cond".into(), "if".into());
        let program = parse_with_aliases_test("cond true; then echo ok; fi", &aliases)
            .expect("parse reserved alias");
        assert!(matches!(
            program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn standalone_bang_is_context_sensitive() {
        let program = parse_test("echo !").expect("parse echo bang");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["echo", "!"]
        ));

        let program = parse_test("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["!true"]
        ));

        let program = parse_test("! true").expect("parse negation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn trailing_blank_aliases_expand_next_simple_command_word() {
        let mut aliases = HashMap::new();
        aliases.insert("say".into(), "printf %s ".into());
        aliases.insert("word".into(), "ok".into());
        let program = parse_with_aliases_test("say word", &aliases).expect("parse chained alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "%s", "ok"]
        ));
    }

    #[test]
    fn self_referential_aliases_do_not_loop_indefinitely() {
        let mut aliases = HashMap::new();
        aliases.insert("loop".into(), "loop ".into());
        let program = parse_with_aliases_test("loop ok", &aliases).expect("self alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["loop", "ok"]
        ));
        assert!(alias_has_trailing_blank("value "));
        assert!(!alias_has_trailing_blank("value"));
    }

    #[test]
    fn alias_expansion_after_assignment_and_redirection() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".into(), "echo aliased".into());
        let program =
            parse_with_aliases_test("VAR=value foo", &aliases).expect("alias after assignment");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.assignments.len() == 1
        ));

        let program =
            parse_with_aliases_test("</dev/null foo", &aliases).expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.redirections.len() == 1
        ));
    }

    #[test]
    fn lparen_after_simple_command_is_syntax_error() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".into(), "echo aliased".into());
        let err = parse_with_aliases_test("foo () { true; }", &aliases).unwrap_err();
        assert!(
            err.message.contains("("),
            "error should mention '(': {}",
            err.message
        );
    }

    #[test]
    fn tokenizer_keeps_dollar_paren_as_single_word() {
        let program = parse_test("echo $(cmd arg)").expect("parse dollar paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "$(cmd arg)"
        ));
    }

    #[test]
    fn tokenizer_keeps_dollar_double_paren_as_single_word() {
        let program = parse_test("echo $((1 + 2))").expect("parse dollar arith");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "$((1 + 2))"
        ));

        let nested = parse_test("echo $((1 + (2 * 3)))").expect("parse nested arith");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if &*cmd.words[1].raw == "$((1 + (2 * 3)))"
        ));

        let error = parse_test("echo $((1 + 2").expect_err("unterminated arith");
        assert_eq!(&*error.message, "unterminated arithmetic expansion");
    }

    #[test]
    fn tokenizer_keeps_dollar_brace_as_single_word() {
        let program = parse_test("echo ${VAR:-default}").expect("parse dollar brace");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "${VAR:-default}"
        ));

        let nested = parse_test("echo ${VAR:-${INNER}}").expect("parse nested brace");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-${INNER}}"
        ));
    }

    #[test]
    fn tokenizer_keeps_backtick_as_single_word() {
        let program = parse_test("echo `cmd arg`").expect("parse backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "`cmd arg`"
        ));

        let error = parse_test("echo `unterminated").expect_err("unterminated backtick");
        assert_eq!(&*error.message, "unterminated backquote");
    }

    #[test]
    fn tokenizer_nested_constructs_in_brace_body() {
        let program = parse_test("echo ${VAR:-'a}b'}").expect("parse brace sq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-'a}b'}"
        ));

        let program = parse_test("echo ${VAR:-\"a}b\"}").expect("parse brace dq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-\"a}b\"}"
        ));

        let program = parse_test("echo ${VAR:-\\}}").expect("parse brace escaped");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-\\}}"
        ));

        let error = parse_test("echo ${VAR:-unclosed").expect_err("unterminated brace body");
        assert_eq!(&*error.message, "unterminated parameter expansion");

        let error = parse_test("echo $(unclosed").expect_err("unterminated paren body");
        assert_eq!(&*error.message, "unterminated command substitution");
    }

    #[test]
    fn tokenizer_emits_io_number_for_adjacent_digits() {
        let p = parse_test("2>err echo ok").expect("parse 2>err");
        assert!(matches!(
            &p.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 1
                    && cmd.redirections[0].fd == Some(2)
                    && cmd.redirections[0].kind == RedirectionKind::Write
        ));

        let p = parse_test("0<in echo ok").expect("parse 0<in");
        assert!(matches!(
            &p.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].fd == Some(0)
                    && cmd.redirections[0].kind == RedirectionKind::Read
        ));
    }

    #[test]
    fn backslash_newline_continuation() {
        let program = parse_test("echo hel\\\nlo").expect("parse continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == "hello"
        ));
    }

    #[test]
    fn if_empty_condition_is_parse_error() {
        let error = parse_test("if then fi").expect_err("empty if condition");
        assert!(error.message.contains("expected command list after 'if'"));
    }

    #[test]
    fn elif_empty_condition_is_parse_error() {
        let error =
            parse_test("if true; then true; elif then true; fi").expect_err("empty elif condition");
        assert!(error.message.contains("expected command list after 'elif'"));
    }

    #[test]
    fn while_empty_condition_is_parse_error() {
        let error = parse_test("while do true; done").expect_err("empty while condition");
        assert!(error
            .message
            .contains("expected command list after 'while'"));
    }

    #[test]
    fn until_empty_condition_is_parse_error() {
        let error = parse_test("until do true; done").expect_err("empty until condition");
        assert!(error
            .message
            .contains("expected command list after 'until'"));
    }

    #[test]
    fn time_default_pipeline() {
        let program = parse_test("time echo hello").expect("parse time default");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Default);
        assert!(!pipeline.negated);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn time_posix_pipeline() {
        let program = parse_test("time -p echo hello").expect("parse time -p");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Posix);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn function_keyword_basic() {
        let program = parse_test("function foo { echo hi; }").expect("parse function keyword");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_with_parens() {
        let program =
            parse_test("function foo() { echo hi; }").expect("parse function keyword parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_invalid_name() {
        let error = parse_test("function 123").expect_err("bad function name");
        assert_eq!(&*error.message, "expected function name");
    }

    #[test]
    fn clone_covers_all_command_variants() {
        let simple = Command::Simple(SimpleCommand {
            assignments: vec![Assignment {
                name: "X".into(),
                value: Word { raw: "1".into(), line: 0 },
            }]
            .into_boxed_slice(),
            words: vec![Word { raw: "echo".into(), line: 0 }].into_boxed_slice(),
            redirections: vec![Redirection {
                fd: Some(2),
                kind: RedirectionKind::Write,
                target: Word { raw: "err".into(), line: 0 },
                here_doc: None,
            }]
            .into_boxed_slice(),
        });
        let s = simple.clone();
        assert!(matches!(&s, Command::Simple(sc) if &*sc.words[0].raw == "echo"));

        let subshell = Command::Subshell(Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![s.clone()].into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                },
                asynchronous: false,
                line: 0,
            }]
            .into_boxed_slice(),
        });
        assert!(matches!(subshell.clone(), Command::Subshell(_)));

        let group = Command::Group(Program { items: vec![].into_boxed_slice() });
        assert!(matches!(group.clone(), Command::Group(_)));

        let func = Command::FunctionDef(FunctionDef {
            name: "f".into(),
            body: Box::new(s.clone()),
        });
        assert!(matches!(&func, Command::FunctionDef(fd) if &*fd.name == "f"));

        let if_cmd = Command::If(IfCommand {
            condition: Program { items: vec![].into_boxed_slice() },
            then_branch: Program { items: vec![].into_boxed_slice() },
            elif_branches: vec![ElifBranch {
                condition: Program { items: vec![].into_boxed_slice() },
                body: Program { items: vec![].into_boxed_slice() },
            }]
            .into_boxed_slice(),
            else_branch: Some(Program { items: vec![].into_boxed_slice() }),
        });
        assert!(matches!(if_cmd, Command::If(_)));

        let loop_cmd = Command::Loop(LoopCommand {
            kind: LoopKind::While,
            condition: Program { items: vec![].into_boxed_slice() },
            body: Program { items: vec![].into_boxed_slice() },
        });
        assert!(matches!(loop_cmd, Command::Loop(_)));

        let for_cmd = Command::For(ForCommand {
            name: "i".into(),
            items: Some(vec![Word { raw: "a".into(), line: 0 }].into_boxed_slice()),
            body: Program { items: vec![].into_boxed_slice() },
        });
        assert!(matches!(&for_cmd, Command::For(fc) if &*fc.name == "i"));

        let case_cmd = Command::Case(CaseCommand {
            word: Word { raw: "x".into(), line: 0 },
            arms: vec![CaseArm {
                patterns: vec![Word { raw: "*".into(), line: 0 }].into_boxed_slice(),
                body: Program { items: vec![].into_boxed_slice() },
                fallthrough: false,
            }]
            .into_boxed_slice(),
        });
        assert!(matches!(case_cmd, Command::Case(_)));

        let redir = Command::Redirected(
            Box::new(s.clone()),
            vec![Redirection {
                fd: None,
                kind: RedirectionKind::Write,
                target: Word { raw: "out".into(), line: 0 },
                here_doc: Some(HereDoc {
                    delimiter: "EOF".into(),
                    body: "test\n".into(),
                    expand: true,
                    strip_tabs: false,
                    body_line: 0,
                }),
            }]
            .into_boxed_slice(),
        );
        assert!(matches!(redir, Command::Redirected(_, _)));
    }

    #[test]
    fn alias_expansion_produces_non_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert("both".into(), "echo a; echo b".into());
        let program =
            parse_with_aliases_test("both", &aliases).expect("parse alias with semicolon");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn alias_expansion_interns_reserved_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "myif".into(),
            "if true; then echo ok; elif false; then echo no; else echo fb; fi".into(),
        );
        let program =
            parse_with_aliases_test("myif", &aliases).expect("alias if/then/elif/else/fi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "mywhile".into(),
            "while false; do echo loop; done".into(),
        );
        let program = parse_with_aliases_test("mywhile", &aliases).expect("alias while/do/done");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myuntil".into(),
            "until true; do echo u; done".into(),
        );
        let program = parse_with_aliases_test("myuntil", &aliases).expect("alias until");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myfor".into(),
            "for x in a b; do echo $x; done".into(),
        );
        let program = parse_with_aliases_test("myfor", &aliases).expect("alias for/in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "mycase".into(),
            "case x in a) echo a;; esac".into(),
        );
        let program = parse_with_aliases_test("mycase", &aliases).expect("alias case/esac");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myfn".into(),
            "function myfunc { echo hi; }".into(),
        );
        let program = parse_with_aliases_test("myfn", &aliases).expect("alias function/{/}");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myneg".into(), "! true".into());
        let program = parse_with_aliases_test("myneg", &aliases).expect("alias bang");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn backslash_newline_mid_word_produces_stripped_form() {
        let program = parse_test("ec\\\nho ok").expect("continuation in command");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == "echo" && &*cmd.words[1].raw == "ok"
        ));

        let program = parse_test("echo a\\\nb\\\nc").expect("multiple continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "abc"
        ));
    }

    #[test]
    fn backslash_newline_before_comment_does_not_start_comment() {
        let program = parse_test("a\\\n#b").expect("continuation before hash");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == "a#b"
        ));
    }

    #[test]
    fn backslash_newline_before_operator_resets() {
        let program = parse_test("echo a\\\nb; echo c").expect("continuation before semi");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn backslash_newline_inside_double_quotes_preserved() {
        let program = parse_test("echo \"ab\\\ncd\"").expect("dquote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "\"ab\\\ncd\""
        ));
    }

    #[test]
    fn backslash_newline_inside_single_quotes_preserved() {
        let program = parse_test("echo 'ab\\\ncd'").expect("squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_backticks_preserved() {
        let program = parse_test("echo `ab\\\ncd`").expect("backtick continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "`ab\\\ncd`"
        ));
    }

    #[test]
    fn backslash_newline_inside_dollar_single_quote_preserved() {
        let program = parse_test("echo $'ab\\\ncd'").expect("dollar-squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "$'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_command_substitution_preserved() {
        let program = parse_test("echo $(ab\\\ncd)").expect("cmdsub continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "$(ab\\\ncd)"
        ));
    }

    #[test]
    fn backslash_newline_mixed_unquoted_and_dquoted() {
        let program =
            parse_test("echo hel\\\nlo\"wor\\\nld\"").expect("mixed continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "hello\"wor\\\nld\""
        ));
    }

    #[test]
    fn heredoc_delimiter_helpers() {
        assert_eq!(parse_here_doc_delimiter("\"EOF\""), ("EOF".into(), false));
        assert_eq!(parse_here_doc_delimiter("\\EOF"), ("EOF".into(), false));
    }

    #[test]
    fn heredoc_delimiter_backslash_inside_double_quotes() {
        assert_eq!(
            parse_here_doc_delimiter("\"ab\\\"cd\""),
            ("ab\"cd".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\\b\""),
            ("a\\b".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\$b\""),
            ("a$b".into(), false)
        );
    }

    #[test]
    fn heredoc_delimiter_dollar_single_quote() {
        assert_eq!(
            parse_here_doc_delimiter("$'EOF'"),
            ("EOF".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("$'ab\\'cd'"),
            ("ab'cd".into(), false)
        );
    }

    #[test]
    fn arithmetic_expansion_with_quoting() {
        let program =
            parse_test("echo $(( 1 + 2 ))").expect("basic arithmetic");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program =
            parse_test("echo $(( \")\" ))").expect("arithmetic with quoted paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn consume_scan_covers_dollar_single_quote_and_default_in_subshell() {
        let program = parse_test("echo $(echo $'hi' done)").expect("dollar-sq in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program = parse_test("echo $(echo $VAR done)").expect("bare dollar in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let err = parse_test("echo $(echo 'unterminated)").expect_err("sq in paren");
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn is_name_basic() {
        assert!(is_name("FOO"));
        assert!(is_name("_bar"));
        assert!(is_name("a1"));
        assert!(!is_name(""));
        assert!(!is_name("1abc"));
    }

    #[test]
    fn aliases_basic() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("ls".into(), "ls --color".into());
        aliases.insert("ll".into(), "ls -la".into());

        assert_eq!(aliases.get("ls").map(|s| &**s), Some("ls --color"));
        assert_eq!(aliases.get("ll").map(|s| &**s), Some("ls -la"));
        assert_eq!(aliases.get("xyz"), None);
    }

    #[test]
    fn multi_line_alias_produces_separate_commands() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("both".into(), "echo a\necho b".into());
        let mut session = ParseSession::new("both\necho c").unwrap();

        let first = session.next_command(&aliases).expect("first").expect("some");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(
            &first.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "a"]
        ));

        let second = session.next_command(&aliases).expect("second").expect("some");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "b"]
        ));

        let third = session.next_command(&aliases).expect("third").expect("some");
        assert_eq!(third.items.len(), 1);
        assert!(matches!(
            &third.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "c"]
        ));

        assert!(session.next_command(&aliases).expect("eof").is_none());
    }

    #[test]
    fn heredoc_delimiter_backslash_preserves_non_special_in_dquotes() {

        assert_eq!(
            parse_here_doc_delimiter("\"E\\OF\""),
            ("E\\OF".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\nb\""),
            ("a\\nb".into(), false)
        );

        assert_eq!(
            parse_here_doc_delimiter("\"a\\$b\""),
            ("a$b".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\\b\""),
            ("a\\b".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\"b\""),
            ("a\"b".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\`b\""),
            ("a`b".into(), false)
        );
    }

    #[test]
    fn io_number_recognised_inside_alias() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("redir".into(), "echo hello 2>/dev/null".into());
        let program =
            parse_with_aliases_test("redir", &aliases).expect("alias with IO number");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if {
                let has_echo = cmd.words.iter().any(|w| &*w.raw == "echo");
                let has_redir_fd2 = cmd.redirections.iter().any(|r|
                    r.fd == Some(2) && r.kind == RedirectionKind::Write
                );

                let no_word_2 = !cmd.words.iter().any(|w| &*w.raw == "2");
                has_echo && has_redir_fd2 && no_word_2
            }
        ));
    }

    #[test]
    fn comment_with_close_paren_inside_command_substitution() {

        let program = parse_test("echo $(echo hello # )\necho world\n)")
            .expect("comment with ) in $(...)");

        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn backslash_newline_continuation_in_alias() {

        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("foo".into(), "echo hell\\\no".into());
        let program =
            parse_with_aliases_test("foo", &aliases).expect("alias with continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "hello"]
        ));
    }

    #[test]
    fn heredoc_quoted_delimiter_no_continuation() {

        let program = parse_test("cat <<'EOF'\nEO\\\nF\nreal body\nEOF\n")
            .expect("quoted heredoc with backslash line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            _ => panic!("expected simple command"),
        };
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.body, "EO\\\nF\nreal body\n");
        assert!(!doc.expand);
    }

    #[test]
    fn backslash_newline_before_comment_in_command_substitution() {

        let program =
            parse_test("echo $(echo foo \\\n# comment with )\necho bar)\n")
                .expect("continuation before comment in $(...)");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn heredoc_body_inside_alias_expansion() {

        let aliases: HashMap<Box<str>, Box<str>> =
            [("x".into(), "cat <<EOF\nhello\nEOF".into())]
                .into_iter()
                .collect();
        let program =
            parse_with_aliases_test("x\n", &aliases).expect("heredoc inside alias");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 1, "word count");
        assert_eq!(&*cmd.words[0].raw, "cat");
        assert_eq!(cmd.redirections.len(), 1, "redirection count");
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0]
            .here_doc
            .as_ref()
            .expect("heredoc body should be present");
        assert_eq!(&*doc.body, "hello\n");
        assert_eq!(&*doc.delimiter, "EOF");
        assert!(doc.expand);
    }

    #[test]
    fn continuation_splits_keyword_if() {

        let program = parse_test("i\\\nf true; then echo ha; fi\n")
            .expect("if split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_then() {
        let program = parse_test("if true; th\\\nen echo ha; fi\n")
            .expect("then split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.then_branch.items.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_while() {
        let program = parse_test("wh\\\nile false; do :; done\n")
            .expect("while split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_do() {
        let program = parse_test("while false; d\\\no :; done\n")
            .expect("do split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_done() {
        let program = parse_test("while false; do :; do\\\nne\n")
            .expect("done split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_for() {
        let program = parse_test("fo\\\nr i in a; do echo $i; done\n")
            .expect("for split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_case() {
        let program = parse_test("cas\\\ne x in x) echo y;; esac\n")
            .expect("case split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_alias_name() {

        let aliases: HashMap<Box<str>, Box<str>> =
            [("foo".into(), "echo aliased".into())]
                .into_iter()
                .collect();
        let program = parse_with_aliases_test("fo\\\no\n", &aliases)
            .expect("alias split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[0].raw, "echo");
        assert_eq!(&*cmd.words[1].raw, "aliased");
    }

    #[test]
    fn continuation_in_word() {

        let program = parse_test("echo he\\\nllo\n").expect("word continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[1].raw, "hello");
    }

    #[test]
    fn continuation_splits_double_semicolon() {

        let program = parse_test("case x in x) echo y;\\\n;esac\n")
            .expect(";; split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(cmd) => cmd,
            other => panic!("expected Case, got {other:?}"),
        };
        assert_eq!(cmd.arms.len(), 1);
        assert!(!cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_and_if() {

        let program = parse_test("true &\\\n& echo ok\n")
            .expect("&& split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::And);
    }

    #[test]
    fn continuation_splits_or_if() {

        let program = parse_test("false |\\\n| echo ok\n")
            .expect("|| split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::Or);
    }

    #[test]
    fn continuation_splits_heredoc_operator() {

        let program = parse_test("cat <\\\n<EOF\nhello\nEOF\n")
            .expect("<< split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections.len(), 1);
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.body, "hello\n");
    }

    #[test]
    fn continuation_splits_append_operator() {

        let program = parse_test("echo hi >\\\n> /dev/null\n")
            .expect(">> split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections.len(), 1);
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::Append);
    }

    #[test]
    fn multiple_continuations_in_keyword() {

        let program = parse_test("w\\\nh\\\ni\\\nl\\\ne false; do :; done\n")
            .expect("while with many continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_at_start_of_input() {

        let program = parse_test("\\\nif true; then echo ha; fi\n")
            .expect("continuation at start");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_esac() {
        let program = parse_test("case x in x) echo y;; es\\\nac\n")
            .expect("esac split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_elif() {
        let program =
            parse_test("if false; then :; el\\\nif true; then echo ok; fi\n")
                .expect("elif split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.elif_branches.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_else() {
        let program =
            parse_test("if false; then :; el\\\nse echo ok; fi\n")
                .expect("else split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert!(cmd.else_branch.is_some());
    }

    #[test]
    fn continuation_splits_keyword_fi() {
        let program = parse_test("if true; then echo ok; f\\\ni\n")
            .expect("fi split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_in() {
        let program = parse_test("for i i\\\nn a b; do echo $i; done\n")
            .expect("in split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_until() {
        let program = parse_test("un\\\ntil false; do echo ok; break; done\n")
            .expect("until split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_dup_input() {

        let program = parse_test("cat <\\\n&0 < /dev/null\n")
            .expect("<& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.kind == RedirectionKind::DupInput));
    }

    #[test]
    fn continuation_splits_dup_output() {

        let program = parse_test("echo hi >\\\n&2\n")
            .expect(">& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.kind == RedirectionKind::DupOutput));
    }

    #[test]
    fn continuation_splits_read_write() {

        let program = parse_test("echo ok <\\\n> /dev/null\n")
            .expect("<> split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.kind == RedirectionKind::ReadWrite));
    }

    #[test]
    fn continuation_splits_clobber_write() {

        let program = parse_test("echo ok >\\\n| /dev/null\n")
            .expect(">| split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.kind == RedirectionKind::ClobberWrite));
    }

    #[test]
    fn continuation_splits_heredoc_strip_tabs() {

        let program = parse_test("cat <\\\n<-EOF\n\thello\n\tEOF\n")
            .expect("<<- split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert!(doc.strip_tabs);
    }

    #[test]
    fn continuation_splits_semi_amp() {

        let program =
            parse_test("case x in x) echo first;\\\n& y) echo second;; esac\n")
                .expect(";& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected Case, got {other:?}"),
        };
        assert!(cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_bang_negation() {

        let program = parse_test("!\\\n true\n")
            .expect("! with continuation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn continuation_splits_heredoc_delimiter_word() {

        let program = parse_test("cat <<EO\\\nF\nhello\nEOF\n")
            .expect("heredoc delimiter split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.delimiter, "EOF");
        assert_eq!(&*doc.body, "hello\n");
    }

    #[test]
    fn continuation_splits_assignment() {

        let program = parse_test("x\\\n=hello echo $x\n")
            .expect("assignment split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.assignments.len(), 1);
        assert_eq!(&*cmd.assignments[0].name, "x");
    }

    #[test]
    fn continuation_splits_io_number() {

        let program = parse_test("echo ok 2\\\n>/dev/null\n")
            .expect("IO number split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.fd == Some(2)));
    }

    #[test]
    fn continuation_inside_double_quotes() {

        let program = parse_test("echo \"he\\\nllo\"\n")
            .expect("continuation inside double quotes");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_inside_backticks() {

        let program = parse_test("echo `echo he\\\nllo`\n")
            .expect("continuation inside backticks");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_arith_close() {

        let program = parse_test("echo $((1+2)\\\n)\n")
            .expect("arith close split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with("$(("));
    }

    #[test]
    fn continuation_splits_dollar_paren() {

        let program = parse_test("echo $\\\n(echo inner)\n")
            .expect("$( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_dollar_brace() {

        let program = parse_test("x=hello; echo $\\\n{x}\n")
            .expect("${ split by continuation");
        let cmd = match &program.items[1].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert_eq!(&*cmd.words[1].raw, "${x}");
    }

    #[test]
    fn continuation_splits_dollar_double_paren() {

        let program = parse_test("echo $(\\\n(1+2))\n")
            .expect("$(( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with("$(("));
    }

    #[test]
    fn continuation_splits_dollar_single_quote() {

        let program = parse_test("echo $\\\n'hello'\n")
            .expect("$' split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn arithmetic_unmatched_close_paren() {

        let program = parse_test("echo $(( 1 ) + 2 ))")
            .expect("arithmetic with unmatched )");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }
}
