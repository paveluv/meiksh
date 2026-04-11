use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};

use super::ParseError;

struct AliasLayer<'a> {
    text: Cow<'a, str>,
    pos: usize,
    trailing_blank: bool,
}

const BC_WORD_BREAK: u8 = 0x01;
const BC_DELIM: u8 = 0x02;
const BC_BLANK: u8 = 0x04;
const BC_QUOTE: u8 = 0x08;
pub(super) const BC_NAME_START: u8 = 0x10;
pub(super) const BC_NAME_CONT: u8 = 0x20;

pub(super) const BYTE_CLASS: [u8; 256] = {
    let mut t = [0u8; 256];

    t[b' ' as usize] = BC_WORD_BREAK | BC_DELIM | BC_BLANK;
    t[b'\t' as usize] = BC_WORD_BREAK | BC_DELIM | BC_BLANK;

    t[b'\n' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b';' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'&' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'|' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'(' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b')' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'<' as usize] = BC_WORD_BREAK | BC_DELIM;
    t[b'>' as usize] = BC_WORD_BREAK | BC_DELIM;

    t[b'#' as usize] = BC_DELIM;

    t[b'\'' as usize] |= BC_QUOTE;
    t[b'"' as usize] |= BC_QUOTE;
    t[b'\\' as usize] |= BC_QUOTE;
    t[b'$' as usize] |= BC_QUOTE;
    t[b'`' as usize] |= BC_QUOTE;

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

pub(super) fn alias_has_trailing_blank(s: &str) -> bool {
    s.as_bytes()
        .last()
        .map_or(false, |&b| BYTE_CLASS[b as usize] & BC_BLANK != 0)
}

fn is_alias_eligible(word: &str) -> bool {
    !word.is_empty() && !word.bytes().any(|b| is_quote(b))
}

struct HereDocInfo {
    delimiter: Box<str>,
    strip_tabs: bool,
    expand: bool,
}

enum HereDocLineItem {
    Token(Token),
    HereDocRef(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
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
    pub(super) fn keyword_name(&self) -> Option<&'static str> {
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

impl Token {
    pub(super) fn into_word(self) -> Option<Box<str>> {
        if let Token::Word(w) = self {
            Some(w)
        } else {
            None
        }
    }
}

impl Token {
    pub(super) fn display_name(&self) -> Box<str> {
        self.keyword_name()
            .unwrap_or(match self {
                Token::Bang => "!",
                Token::LBrace => "{",
                Token::RBrace => "}",
                _ => "word",
            })
            .into()
    }
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

pub struct Parser<'a> {
    pub(super) line: usize,
    cached_byte: Option<u8>,
    pub(super) aliases: &'a HashMap<Box<str>, Box<str>>,
    alias_stack: Vec<AliasLayer<'a>>,
    alias_depth: usize,
    active_alias_names: Vec<String>,
    popped_alias_names: Vec<String>,
    alias_trailing_blank_pending: bool,
    pushed_back_byte: Option<u8>,
    cached_token: Option<Token>,
    token_queue: VecDeque<Token>,
    pub(super) keyword_mode: bool,
    alias_mode: bool,
}

impl<'a> Parser<'a> {
    pub(super) fn new(source: &'a str, aliases: &'a HashMap<Box<str>, Box<str>>) -> Self {
        Self::new_at(source, 0, 1, aliases)
    }

    pub(super) fn new_at(
        source: &'a str,
        pos: usize,
        line: usize,
        aliases: &'a HashMap<Box<str>, Box<str>>,
    ) -> Self {
        let cached_byte = source.as_bytes().get(pos).copied();
        Self {
            line,
            cached_byte,
            aliases,
            alias_stack: vec![AliasLayer {
                text: Cow::Borrowed(source),
                pos,
                trailing_blank: false,
            }],
            alias_depth: 0,
            active_alias_names: Vec::new(),
            popped_alias_names: Vec::new(),
            alias_trailing_blank_pending: false,
            pushed_back_byte: None,
            cached_token: None,
            token_queue: VecDeque::new(),
            keyword_mode: true,
            alias_mode: true,
        }
    }

    pub fn current_line(&self) -> usize {
        self.line
    }

    pub(super) fn source_pos(&self) -> usize {
        self.alias_stack[0].pos
    }

    pub(super) fn restore_alias_state(&mut self, saved: SavedAliasState) {
        for layer in saved.layers {
            self.alias_stack.push(layer);
        }
        self.alias_depth = saved.depth;
        self.active_alias_names = saved.active_names;
        self.alias_trailing_blank_pending = saved.trailing_blank_pending;
        self.sync_cached_byte();
    }

    pub(super) fn save_alias_state(self) -> Option<SavedAliasState> {
        if self.alias_stack.len() <= 1 {
            return None;
        }
        let layers = self
            .alias_stack
            .into_iter()
            .skip(1)
            .map(|layer| AliasLayer {
                text: Cow::Owned(layer.text.into_owned()),
                pos: layer.pos,
                trailing_blank: layer.trailing_blank,
            })
            .collect();
        Some(SavedAliasState {
            layers,
            depth: self.alias_depth,
            active_names: self.active_alias_names,
            trailing_blank_pending: self.alias_trailing_blank_pending,
        })
    }

    fn sync_cached_byte(&mut self) {
        let layer = self.alias_stack.last().unwrap();
        self.cached_byte = layer.text.as_bytes().get(layer.pos).copied();
    }

    pub(super) fn error(&self, message: impl Into<Box<str>>) -> ParseError {
        ParseError {
            message: message.into(),
            line: Some(self.line),
        }
    }

    #[inline(always)]
    fn pop_exhausted_layers(&mut self) {
        if self.alias_stack.len() > 1 {
            self.pop_exhausted_layers_slow();
        }
    }

    #[cold]
    fn pop_exhausted_layers_slow(&mut self) {
        while self.alias_stack.len() > 1 {
            let layer = self.alias_stack.last().unwrap();
            if layer.pos < layer.text.len() {
                break;
            }
            if layer.trailing_blank {
                self.alias_trailing_blank_pending = true;
            }
            self.alias_stack.pop();
            self.alias_depth = self.alias_depth.saturating_sub(1);
            if let Some(name) = self.active_alias_names.pop() {
                self.popped_alias_names.push(name);
            }
        }
    }

    #[inline(always)]
    fn peek_byte(&self) -> Option<u8> {
        self.cached_byte
    }

    #[inline(always)]
    fn advance_byte(&mut self) {
        if let Some(b) = self.pushed_back_byte.take() {
            self.cached_byte = Some(b);
            return;
        }
        let at_source = self.alias_stack.len() == 1;
        let layer = self.alias_stack.last_mut().unwrap();
        let bytes = layer.text.as_bytes();
        if at_source {
            if bytes[layer.pos] == b'\n' {
                self.line += 1;
            }
            layer.pos += 1;
            self.cached_byte = bytes.get(layer.pos).copied();
        } else {
            layer.pos += 1;
            self.cached_byte = bytes.get(layer.pos).copied();
            if self.cached_byte.is_none() {
                self.pop_exhausted_layers();
                self.sync_cached_byte();
            }
        }
    }

    #[inline]
    fn skip_continuations(&mut self) {
        loop {
            if self.cached_byte != Some(b'\\') {
                return;
            }
            self.advance_byte();
            if self.cached_byte == Some(b'\n') {
                self.advance_byte();
            } else {
                self.pushed_back_byte = self.cached_byte;
                self.cached_byte = Some(b'\\');
                return;
            }
        }
    }

    #[inline]
    fn skip_blanks(&mut self) {
        loop {
            match self.peek_byte() {
                Some(b' ' | b'\t') => self.advance_byte(),
                _ => break,
            }
        }
    }

    #[inline]
    fn skip_blanks_and_comments(&mut self) {
        self.skip_blanks();
        if self.peek_byte() == Some(b'#') {
            while !matches!(self.peek_byte(), None | Some(b'\n')) {
                self.advance_byte();
            }
        }
    }

    fn consume_single_quote(&mut self, raw: &mut String) -> Result<(), ParseError> {
        if self.pushed_back_byte.is_none() && self.alias_stack.len() == 1 {
            let layer = &mut self.alias_stack[0];
            let bytes = layer.text.as_bytes();
            let start = layer.pos;
            let mut pos = start;
            while pos < bytes.len() {
                let c = bytes[pos];
                if c == b'\'' {
                    pos += 1;
                    raw.push_str(&layer.text[start..pos]);
                    layer.pos = pos;
                    self.cached_byte = bytes.get(pos).copied();
                    return Ok(());
                }
                if c == b'\n' {
                    self.line += 1;
                }
                pos += 1;
            }
            layer.pos = pos;
            self.cached_byte = None;
            return Err(self.error("unterminated single quote"));
        }
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
                    self.consume_arithmetic_body(raw)
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

    fn consume_arithmetic_body(&mut self, raw: &mut String) -> Result<(), ParseError> {
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
        let mut at_command_start = true;
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated command substitution")),
                Some(b'(') => {
                    depth += 1;
                    at_command_start = true;
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
                    at_command_start = true;
                }
                Some(b'#') if at_command_start => {
                    while let Some(b) = self.peek_byte() {
                        if b == b'\n' {
                            break;
                        }
                        raw.push(b as char);
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
                        at_command_start = false;
                        if let Some(b) = self.peek_byte() {
                            raw.push(b as char);
                            self.advance_byte();
                        }
                    }
                }
                Some(b) if is_quote(b) => {
                    at_command_start = false;
                    self.consume_quoted_element(raw)?;
                }
                Some(b) if is_word_break(b) => {
                    at_command_start = true;
                    raw.push(b as char);
                    self.advance_byte();
                }
                Some(b) => {
                    at_command_start = false;
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
        let b = self.peek_byte().unwrap();
        if b == b'\'' {
            raw.push('\'');
            self.advance_byte();
            self.consume_single_quote(raw)
        } else if b == b'"' {
            raw.push('"');
            self.advance_byte();
            self.consume_double_quote(raw)
        } else if b == b'\\' {
            raw.push('\\');
            self.advance_byte();
            if let Some(c) = self.peek_byte() {
                raw.push(c as char);
                self.advance_byte();
            }
            Ok(())
        } else if b == b'$' {
            raw.push('$');
            self.advance_byte();
            self.consume_dollar_construct(raw)
        } else {
            raw.push('`');
            self.advance_byte();
            self.consume_backtick_inner(raw)
        }
    }

    fn read_here_doc_body(
        &mut self,
        delimiter: &str,
        strip_tabs: bool,
        expand: bool,
    ) -> Result<String, ParseError> {
        let mut body = String::new();
        let mut continuation_buffer = String::new();
        let mut line = String::with_capacity(80);
        let mut body_len_before_continuation = 0;
        loop {
            line.clear();
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

            if expand && line.as_bytes().last() == Some(&b'\\') && has_newline {
                if continuation_buffer.is_empty() {
                    body_len_before_continuation = body.len();
                }
                continuation_buffer.push_str(&line[..line.len() - 1]);
                body.push_str(&line);
                body.push('\n');
                continue;
            }

            let compare = if !continuation_buffer.is_empty() {
                continuation_buffer.push_str(&line);
                &continuation_buffer
            } else {
                &line
            };
            let compare = if strip_tabs {
                compare.trim_start_matches('\t')
            } else {
                compare
            };
            if compare == delimiter {
                if !continuation_buffer.is_empty() {
                    body.truncate(body_len_before_continuation);
                }
                return Ok(body);
            }
            continuation_buffer.clear();

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

    pub(super) fn set_command_position(&mut self) {
        self.keyword_mode = true;
        self.alias_mode = true;
    }

    pub(super) fn set_keyword_position(&mut self) {
        self.keyword_mode = true;
        self.alias_mode = false;
    }

    pub(super) fn set_argument_position(&mut self) {
        self.keyword_mode = false;
        self.alias_mode = false;
    }

    pub(super) fn peek_token(&mut self) -> Result<&Token, ParseError> {
        if self.cached_token.is_none() {
            self.cached_token = Some(self.produce_token()?);
        }
        Ok(self.cached_token.as_ref().unwrap())
    }

    pub(super) fn advance_token(&mut self) {
        self.cached_token = None;
    }

    pub(super) fn next_token(&mut self) -> Token {
        self.cached_token
            .take()
            .expect("next_token without peek_token")
    }

    fn produce_token(&mut self) -> Result<Token, ParseError> {
        if let Some(tok) = self.token_queue.pop_front() {
            return self.reclassify_queued_token(tok);
        }
        self.produce_token_from_bytes()
    }

    fn reclassify_queued_token(&mut self, tok: Token) -> Result<Token, ParseError> {
        let check_keyword = self.keyword_mode;
        let check_alias = self.alias_mode || self.alias_trailing_blank_pending;
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        if let Token::Word(w) = tok {
            if check_keyword {
                if let Some(kw_tok) = word_to_keyword_token(&w) {
                    return Ok(kw_tok);
                }
            }
            if check_alias {
                if let Some(value) = self.aliases.get(&*w) {
                    if is_alias_eligible(&w)
                        && !self.active_alias_names.iter().any(|n| n == &*w)
                        && self.alias_depth < 1024
                    {
                        let value: &str = value;
                        let trailing_blank = alias_has_trailing_blank(value);
                        self.active_alias_names.push(String::from(w));
                        self.alias_stack.push(AliasLayer {
                            text: Cow::Borrowed(value),
                            pos: 0,
                            trailing_blank,
                        });
                        self.alias_depth += 1;
                        self.sync_cached_byte();
                        return self.produce_token_from_bytes();
                    }
                }
            }
            Ok(Token::Word(w))
        } else {
            Ok(tok)
        }
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
            Some(b'0'..=b'9') => self.produce_io_number_or_word(),
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
                self.produce_heredoc_token(strip_tabs)
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

    fn produce_heredoc_token(&mut self, first_strip_tabs: bool) -> Result<Token, ParseError> {
        self.skip_blanks();
        let mut first_raw = String::new();
        self.scan_raw_word(&mut first_raw)?;
        if first_raw.is_empty() {
            return Err(self.error("expected heredoc delimiter"));
        }
        let (first_delimiter, first_expand) = parse_here_doc_delimiter(&first_raw);

        let mut heredoc_entries: Vec<HereDocInfo> = vec![HereDocInfo {
            delimiter: first_delimiter,
            strip_tabs: first_strip_tabs,
            expand: first_expand,
        }];

        let mut queued_items: Vec<HereDocLineItem> = Vec::new();

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
                            let strip_tabs = if self.peek_byte() == Some(b'-') {
                                self.advance_byte();
                                true
                            } else {
                                false
                            };
                            self.skip_blanks();
                            let mut delimiter_raw = String::new();
                            self.scan_raw_word(&mut delimiter_raw)?;
                            if delimiter_raw.is_empty() {
                                return Err(self.error("expected heredoc delimiter"));
                            }
                            let (delimiter, expand) = parse_here_doc_delimiter(&delimiter_raw);
                            let idx = heredoc_entries.len();
                            heredoc_entries.push(HereDocInfo {
                                delimiter,
                                strip_tabs,
                                expand,
                            });
                            queued_items.push(HereDocLineItem::HereDocRef(idx));
                        }
                        Some(b'&') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::LessAnd));
                        }
                        Some(b'>') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::LessGreat));
                        }
                        _ => queued_items.push(HereDocLineItem::Token(Token::Less)),
                    }
                }
                Some(b'>') => {
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b'>') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::DGreat));
                        }
                        Some(b'&') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::GreatAnd));
                        }
                        Some(b'|') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::Clobber));
                        }
                        _ => queued_items.push(HereDocLineItem::Token(Token::Great)),
                    }
                }
                Some(b'|') => {
                    self.advance_byte();
                    self.skip_continuations();
                    if self.peek_byte() == Some(b'|') {
                        self.advance_byte();
                        queued_items.push(HereDocLineItem::Token(Token::OrIf));
                    } else {
                        queued_items.push(HereDocLineItem::Token(Token::Pipe));
                    }
                }
                Some(b'&') => {
                    self.advance_byte();
                    self.skip_continuations();
                    if self.peek_byte() == Some(b'&') {
                        self.advance_byte();
                        queued_items.push(HereDocLineItem::Token(Token::AndIf));
                    } else {
                        queued_items.push(HereDocLineItem::Token(Token::Amp));
                    }
                }
                Some(b';') => {
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b';') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::DSemi));
                        }
                        Some(b'&') => {
                            self.advance_byte();
                            queued_items.push(HereDocLineItem::Token(Token::SemiAmp));
                        }
                        _ => queued_items.push(HereDocLineItem::Token(Token::Semi)),
                    }
                }
                Some(b'(') => {
                    self.advance_byte();
                    queued_items.push(HereDocLineItem::Token(Token::LParen));
                }
                Some(b')') => {
                    self.advance_byte();
                    queued_items.push(HereDocLineItem::Token(Token::RParen));
                }
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
                            queued_items.push(HereDocLineItem::Token(Token::IoNumber(fd)));
                            continue;
                        }
                    }
                    let mut raw = digits;
                    self.scan_raw_word(&mut raw)?;
                    if !raw.is_empty() {
                        queued_items.push(HereDocLineItem::Token(Token::Word(raw.into())));
                    }
                }
                _ => {
                    let mut raw = String::new();
                    self.scan_raw_word(&mut raw)?;
                    if !raw.is_empty() {
                        queued_items.push(HereDocLineItem::Token(Token::Word(raw.into())));
                    }
                }
            }
        }

        if self.peek_byte() == Some(b'\n') {
            self.advance_byte();
        }

        let mut bodies: Vec<(Box<str>, usize)> = Vec::new();
        for entry in &heredoc_entries {
            let body_line = self.line;
            let body: Box<str> = self
                .read_here_doc_body(&entry.delimiter, entry.strip_tabs, entry.expand)?
                .into();
            bodies.push((body, body_line));
        }

        for item in queued_items {
            match item {
                HereDocLineItem::Token(tok) => self.token_queue.push_back(tok),
                HereDocLineItem::HereDocRef(idx) => {
                    let (ref body, body_line) = bodies[idx];
                    let entry = &heredoc_entries[idx];
                    self.token_queue.push_back(Token::HereDoc {
                        strip_tabs: entry.strip_tabs,
                        expand: entry.expand,
                        delimiter: entry.delimiter.clone(),
                        body: body.clone(),
                        body_line,
                    });
                }
            }
        }
        self.token_queue.push_back(Token::Newline);

        let (ref body, body_line) = bodies[0];
        let first = &heredoc_entries[0];
        Ok(Token::HereDoc {
            strip_tabs: first.strip_tabs,
            expand: first.expand,
            delimiter: first.delimiter.clone(),
            body: body.clone(),
            body_line,
        })
    }

    fn produce_io_number_or_word(&mut self) -> Result<Token, ParseError> {
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
        self.produce_word(digits)
    }

    #[inline(always)]
    fn produce_word_token(&mut self) -> Result<Token, ParseError> {
        self.produce_word(String::new())
    }

    fn produce_word(&mut self, prefix: String) -> Result<Token, ParseError> {
        let check_keyword = self.keyword_mode;
        let check_alias = self.alias_mode || self.alias_trailing_blank_pending;
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        let mut raw = prefix;
        loop {
            if raw.is_empty() {
                if self.cached_byte.is_none() || matches!(self.peek_byte(), Some(b) if is_delim(b))
                {
                    self.popped_alias_names.clear();
                    return Ok(Token::Eof);
                }
            }

            let had_quote = self.scan_raw_word(&mut raw)?;
            if raw.is_empty() {
                self.popped_alias_names.clear();
                return Ok(Token::Eof);
            }

            if !had_quote {
                if check_alias {
                    if let Some(value) = self.aliases.get(&*raw) {
                        if is_alias_eligible(&raw)
                            && !self.active_alias_names.iter().any(|n| n == &*raw)
                            && !self.popped_alias_names.iter().any(|n| n == &*raw)
                            && self.alias_depth < 1024
                        {
                            let value: &str = value;
                            let trailing_blank = alias_has_trailing_blank(value);
                            self.active_alias_names.push(std::mem::take(&mut raw));
                            self.alias_stack.push(AliasLayer {
                                text: Cow::Borrowed(value),
                                pos: 0,
                                trailing_blank,
                            });
                            self.alias_depth += 1;
                            self.sync_cached_byte();
                            self.skip_blanks_and_comments();
                            continue;
                        }
                    }
                }
                if check_keyword {
                    if let Some(kw_tok) = word_to_keyword_token(&raw) {
                        self.popped_alias_names.clear();
                        return Ok(kw_tok);
                    }
                }
            }

            self.popped_alias_names.clear();
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
                    if self.pushed_back_byte.is_none() && self.alias_stack.len() == 1 {
                        let layer = &mut self.alias_stack[0];
                        let bytes = layer.text.as_bytes();
                        let start = layer.pos;
                        let mut pos = start + 1;
                        while pos < bytes.len() {
                            if BYTE_CLASS[bytes[pos] as usize] & (BC_WORD_BREAK | BC_QUOTE) != 0 {
                                break;
                            }
                            pos += 1;
                        }
                        raw.push_str(&layer.text[start..pos]);
                        layer.pos = pos;
                        self.cached_byte = bytes.get(pos).copied();
                    } else {
                        raw.push(b as char);
                        self.advance_byte();
                    }
                }
            }
        }
        Ok(had_quote)
    }
}

pub(super) struct SavedAliasState {
    layers: Vec<AliasLayer<'static>>,
    depth: usize,
    active_names: Vec<String>,
    trailing_blank_pending: bool,
}

pub(super) fn parse_here_doc_delimiter(raw: &str) -> (Box<str>, bool) {
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
