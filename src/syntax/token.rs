use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

use super::ParseError;
use super::byte_class::{
    alias_has_trailing_blank, is_delim, is_digit, is_glob_char, is_name_cont, is_name_start,
    is_quote, is_special_param, is_tilde_user_break, is_word_break,
};
use super::word_parts::{BracedName, BracedOp, ExpansionKind, WordPart};
use crate::hash::ShellMap;

struct AliasLayer<'a> {
    text: Cow<'a, [u8]>,
    pos: usize,
    trailing_blank: bool,
}

fn is_alias_eligible(word: &[u8]) -> bool {
    !word.is_empty() && !word.iter().any(|&b| is_quote(b))
}

struct HereDocInfo {
    delimiter: Box<[u8]>,
    strip_tabs: bool,
    expand: bool,
}

enum HereDocLineItem {
    Token(Token),
    HereDocRef(usize),
}

fn parse_i32_bytes(b: &[u8]) -> Option<i32> {
    if b.is_empty() {
        return None;
    }
    let mut result: i32 = 0;
    for &d in b {
        if !is_digit(d) {
            return None;
        }
        result = result.checked_mul(10)?.checked_add((d - b'0') as i32)?;
    }
    Some(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
    Word(Box<[u8]>, Box<[WordPart]>),
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
        delimiter: Box<[u8]>,
        body: Box<[u8]>,
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
    pub(super) fn keyword_name(&self) -> Option<&'static [u8]> {
        match self {
            Token::If => Some(b"if"),
            Token::Then => Some(b"then"),
            Token::Else => Some(b"else"),
            Token::Elif => Some(b"elif"),
            Token::Fi => Some(b"fi"),
            Token::Do => Some(b"do"),
            Token::Done => Some(b"done"),
            Token::Case => Some(b"case"),
            Token::Esac => Some(b"esac"),
            Token::In => Some(b"in"),
            Token::While => Some(b"while"),
            Token::Until => Some(b"until"),
            Token::For => Some(b"for"),
            Token::Function => Some(b"function"),
            _ => None,
        }
    }
}

impl Token {
    pub(super) fn into_word(self) -> Option<(Box<[u8]>, Box<[WordPart]>)> {
        if let Token::Word(w, p) = self {
            Some((w, p))
        } else {
            None
        }
    }
}

impl Token {
    pub(super) fn display_name(&self) -> &'static [u8] {
        self.keyword_name().unwrap_or(match self {
            Token::Bang => b"!",
            Token::LBrace => b"{",
            Token::RBrace => b"}",
            _ => b"word",
        })
    }
}

fn word_to_keyword_token(w: &[u8]) -> Option<Token> {
    match w {
        b"if" => Some(Token::If),
        b"then" => Some(Token::Then),
        b"else" => Some(Token::Else),
        b"elif" => Some(Token::Elif),
        b"fi" => Some(Token::Fi),
        b"do" => Some(Token::Do),
        b"done" => Some(Token::Done),
        b"case" => Some(Token::Case),
        b"esac" => Some(Token::Esac),
        b"in" => Some(Token::In),
        b"while" => Some(Token::While),
        b"until" => Some(Token::Until),
        b"for" => Some(Token::For),
        b"function" => Some(Token::Function),
        b"!" => Some(Token::Bang),
        b"{" => Some(Token::LBrace),
        b"}" => Some(Token::RBrace),
        _ => None,
    }
}

pub(super) struct Parser<'a> {
    pub(super) line: usize,
    cached_byte: Option<u8>,
    pub(super) aliases: &'a ShellMap<Box<[u8]>, Box<[u8]>>,
    alias_stack: Vec<AliasLayer<'a>>,
    alias_depth: usize,
    expanding_aliases: HashSet<Cow<'a, [u8]>>,
    alias_trailing_blank_pending: bool,
    pushed_back_byte: Option<u8>,
    cached_token: Option<Token>,
    token_queue: VecDeque<Token>,
    pub(super) keyword_mode: bool,
    alias_mode: bool,
    word_raw: Vec<u8>,
    word_parts: Vec<WordPart>,
    word_qbuf: Vec<u8>,
}

impl<'a> Parser<'a> {
    pub(super) fn new(source: &'a [u8], aliases: &'a ShellMap<Box<[u8]>, Box<[u8]>>) -> Self {
        Self::new_at(source, 0, 1, aliases)
    }

    pub(super) fn new_at(
        source: &'a [u8],
        pos: usize,
        line: usize,
        aliases: &'a ShellMap<Box<[u8]>, Box<[u8]>>,
    ) -> Self {
        let cached_byte = source.get(pos).copied();
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
            expanding_aliases: HashSet::new(),
            alias_trailing_blank_pending: false,
            pushed_back_byte: None,
            cached_token: None,
            token_queue: VecDeque::new(),
            keyword_mode: true,
            alias_mode: true,
            word_raw: Vec::new(),
            word_parts: Vec::new(),
            word_qbuf: Vec::new(),
        }
    }

    pub(super) fn current_line(&self) -> usize {
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
        self.expanding_aliases = saved
            .expanding_aliases
            .into_iter()
            .map(Cow::Owned)
            .collect();
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
            expanding_aliases: self
                .expanding_aliases
                .into_iter()
                .map(Cow::into_owned)
                .collect(),
            trailing_blank_pending: self.alias_trailing_blank_pending,
        })
    }

    fn sync_cached_byte(&mut self) {
        let layer = self.alias_stack.last().unwrap();
        self.cached_byte = layer.text.get(layer.pos).copied();
    }

    pub(super) fn error(&self, message: &[u8]) -> ParseError {
        ParseError {
            message: message.to_vec().into_boxed_slice(),
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
        let bytes = &*layer.text;
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

    fn consume_single_quote(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        if self.pushed_back_byte.is_none() && self.alias_stack.len() == 1 {
            let layer = &mut self.alias_stack[0];
            let bytes = &*layer.text;
            let start = layer.pos;
            let mut pos = start;
            while pos < bytes.len() {
                let c = bytes[pos];
                if c == b'\'' {
                    pos += 1;
                    raw.extend_from_slice(&bytes[start..pos]);
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
            return Err(self.error(b"unterminated single quote"));
        }
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated single quote")),
                Some(b'\'') => {
                    raw.push(b'\'');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_double_quote(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated double quote")),
                Some(b'"') => {
                    raw.push(b'"');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push(b'\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b);
                        self.advance_byte();
                    }
                }
                Some(b'$') => {
                    raw.push(b'$');
                    self.advance_byte();
                    self.consume_dollar_construct(raw)?;
                }
                Some(b'`') => {
                    raw.push(b'`');
                    self.advance_byte();
                    self.consume_backtick_inner(raw)?;
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_dollar_construct(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        match self.peek_byte() {
            Some(b'(') => {
                raw.push(b'(');
                self.advance_byte();
                self.skip_continuations();
                if self.peek_byte() == Some(b'(') {
                    raw.push(b'(');
                    self.advance_byte();
                    self.consume_arithmetic_body(raw)
                } else {
                    self.consume_paren_body(raw)
                }
            }
            Some(b'{') => {
                raw.push(b'{');
                self.advance_byte();
                self.consume_brace_body(raw)
            }
            _ => Ok(()),
        }
    }

    fn consume_arithmetic_body(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        let mut depth = 1usize;
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated arithmetic expansion")),
                Some(b'(') => {
                    depth += 1;
                    raw.push(b'(');
                    self.advance_byte();
                }
                Some(b')') => {
                    if depth == 1 {
                        raw.push(b')');
                        self.advance_byte();
                        self.skip_continuations();
                        if self.peek_byte() == Some(b')') {
                            raw.push(b')');
                            self.advance_byte();
                            return Ok(());
                        }
                    } else {
                        depth -= 1;
                        raw.push(b')');
                        self.advance_byte();
                    }
                }
                Some(b) if is_quote(b) => {
                    self.consume_quoted_element(raw)?;
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_dollar_single_quote(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        raw.push(b'\'');
        self.advance_byte();
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated dollar-single-quotes")),
                Some(b'\'') => {
                    raw.push(b'\'');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push(b'\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b);
                        self.advance_byte();
                    }
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_paren_body(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        let mut depth = 1usize;
        let mut at_command_start = true;
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated command substitution")),
                Some(b'(') => {
                    depth += 1;
                    at_command_start = true;
                    raw.push(b'(');
                    self.advance_byte();
                }
                Some(b')') => {
                    depth -= 1;
                    raw.push(b')');
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
                        raw.push(b);
                        self.advance_byte();
                    }
                }
                Some(b'\\') => {
                    raw.push(b'\\');
                    self.advance_byte();
                    if self.peek_byte() == Some(b'\n') {
                        raw.push(b'\n');
                        self.advance_byte();
                    } else {
                        at_command_start = false;
                        if let Some(b) = self.peek_byte() {
                            raw.push(b);
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
                    raw.push(b);
                    self.advance_byte();
                }
                Some(b) => {
                    at_command_start = false;
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_brace_body(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated parameter expansion")),
                Some(b'}') => {
                    raw.push(b'}');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b) if is_quote(b) => {
                    self.consume_quoted_element(raw)?;
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_backtick_inner(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error(b"unterminated backquote")),
                Some(b'`') => {
                    raw.push(b'`');
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    raw.push(b'\\');
                    self.advance_byte();
                    if let Some(b) = self.peek_byte() {
                        raw.push(b);
                        self.advance_byte();
                    }
                }
                Some(b) => {
                    raw.push(b);
                    self.advance_byte();
                }
            }
        }
    }

    fn consume_quoted_element(&mut self, raw: &mut Vec<u8>) -> Result<(), ParseError> {
        let b = self.peek_byte().unwrap();
        if b == b'\'' {
            raw.push(b'\'');
            self.advance_byte();
            self.consume_single_quote(raw)
        } else if b == b'"' {
            raw.push(b'"');
            self.advance_byte();
            self.consume_double_quote(raw)
        } else if b == b'\\' {
            raw.push(b'\\');
            self.advance_byte();
            if let Some(c) = self.peek_byte() {
                raw.push(c);
                self.advance_byte();
            }
            Ok(())
        } else if b == b'$' {
            raw.push(b'$');
            self.advance_byte();
            self.consume_dollar_construct(raw)
        } else {
            raw.push(b'`');
            self.advance_byte();
            self.consume_backtick_inner(raw)
        }
    }

    fn read_here_doc_body(
        &mut self,
        delimiter: &[u8],
        strip_tabs: bool,
        expand: bool,
    ) -> Result<Vec<u8>, ParseError> {
        let mut body = Vec::new();
        let mut continuation_buffer = Vec::new();
        let mut line = Vec::with_capacity(80);
        loop {
            line.clear();
            let has_newline = loop {
                match self.peek_byte() {
                    Some(b'\n') => {
                        self.advance_byte();
                        break true;
                    }
                    Some(b) => {
                        line.push(b);
                        self.advance_byte();
                    }
                    None => break false,
                }
            };

            let trailing_backslashes = line.iter().rev().take_while(|&&b| b == b'\\').count();
            if expand && trailing_backslashes % 2 == 1 && has_newline {
                continuation_buffer.extend_from_slice(&line[..line.len() - 1]);
                continue;
            }

            let logical_line = if !continuation_buffer.is_empty() {
                continuation_buffer.extend_from_slice(&line);
                &continuation_buffer
            } else {
                &line
            };
            let stripped = if strip_tabs {
                let skip = logical_line.iter().take_while(|&&b| b == b'\t').count();
                &logical_line[skip..]
            } else {
                logical_line.as_slice()
            };
            if stripped == delimiter {
                return Ok(body);
            }

            if !has_newline {
                body.extend_from_slice(stripped);
                return Err(ParseError {
                    message: b"unterminated here-document".to_vec().into_boxed_slice(),
                    line: Some(self.line),
                });
            }

            body.extend_from_slice(stripped);
            body.push(b'\n');
            continuation_buffer.clear();
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
        if !self.alias_trailing_blank_pending && self.alias_stack.len() == 1 {
            self.expanding_aliases.clear();
        }
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        if let Token::Word(w, p) = tok {
            if check_keyword {
                if let Some(kw_tok) = word_to_keyword_token(&w) {
                    return Ok(kw_tok);
                }
            }
            if check_alias {
                if let Some((key, value)) = self.aliases.get_key_value(&*w) {
                    if is_alias_eligible(&w)
                        && !self.expanding_aliases.contains(&*w)
                        && self.alias_depth < 1024
                    {
                        let value: &[u8] = value;
                        let trailing_blank = alias_has_trailing_blank(value);
                        self.expanding_aliases.insert(Cow::Borrowed(&**key));
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
            Ok(Token::Word(w, p))
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
        let mut first_raw = Vec::new();
        self.scan_raw_word(&mut first_raw)?;
        if first_raw.is_empty() {
            return Err(self.error(b"expected heredoc delimiter"));
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
                            let mut delimiter_raw = Vec::new();
                            self.scan_raw_word(&mut delimiter_raw)?;
                            if delimiter_raw.is_empty() {
                                return Err(self.error(b"expected heredoc delimiter"));
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
                Some(b) if is_digit(b) => {
                    let mut digits = Vec::new();
                    while let Some(b) = self.peek_byte() {
                        if is_digit(b) {
                            digits.push(b);
                            self.advance_byte();
                        } else {
                            break;
                        }
                    }
                    self.skip_continuations();
                    if matches!(self.peek_byte(), Some(b'<' | b'>')) {
                        if let Some(fd) = parse_i32_bytes(&digits) {
                            queued_items.push(HereDocLineItem::Token(Token::IoNumber(fd)));
                            continue;
                        }
                    }
                    self.word_raw.clear();
                    self.word_raw.extend_from_slice(&digits);
                    self.word_parts.clear();
                    self.word_qbuf.clear();
                    let (_had_quote, lit_start) = self.scan_raw_word_parts_scratch(0)?;
                    flush_literal(
                        &self.word_raw,
                        lit_start,
                        self.word_raw.len(),
                        &mut self.word_parts,
                    );
                    flush_quoted_buf(&mut self.word_qbuf, &mut self.word_parts);
                    if !self.word_raw.is_empty() {
                        let raw = std::mem::take(&mut self.word_raw);
                        let parts = std::mem::take(&mut self.word_parts);
                        queued_items.push(HereDocLineItem::Token(Token::Word(
                            raw.into_boxed_slice(),
                            parts.into_boxed_slice(),
                        )));
                    }
                }
                _ => {
                    self.word_raw.clear();
                    self.word_parts.clear();
                    self.word_qbuf.clear();
                    let (_had_quote, lit_start) = self.scan_raw_word_parts_scratch(0)?;
                    flush_literal(
                        &self.word_raw,
                        lit_start,
                        self.word_raw.len(),
                        &mut self.word_parts,
                    );
                    flush_quoted_buf(&mut self.word_qbuf, &mut self.word_parts);
                    if !self.word_raw.is_empty() {
                        let raw = std::mem::take(&mut self.word_raw);
                        let parts = std::mem::take(&mut self.word_parts);
                        queued_items.push(HereDocLineItem::Token(Token::Word(
                            raw.into_boxed_slice(),
                            parts.into_boxed_slice(),
                        )));
                    }
                }
            }
        }

        if self.peek_byte() == Some(b'\n') {
            self.advance_byte();
        }

        let mut bodies: Vec<(Box<[u8]>, usize)> = Vec::new();
        for entry in &heredoc_entries {
            let body_line = self.line;
            let body: Box<[u8]> = self
                .read_here_doc_body(&entry.delimiter, entry.strip_tabs, entry.expand)?
                .into_boxed_slice();
            bodies.push((body, body_line));
        }

        for item in queued_items {
            match item {
                HereDocLineItem::Token(tok) => self.token_queue.push_back(tok),
                HereDocLineItem::HereDocRef(idx) => {
                    let (body, body_line) = std::mem::take(&mut bodies[idx]);
                    let entry = &mut heredoc_entries[idx];
                    self.token_queue.push_back(Token::HereDoc {
                        strip_tabs: entry.strip_tabs,
                        expand: entry.expand,
                        delimiter: std::mem::take(&mut entry.delimiter),
                        body,
                        body_line,
                    });
                }
            }
        }
        self.token_queue.push_back(Token::Newline);

        let (body, body_line) = std::mem::take(&mut bodies[0]);
        let first = &mut heredoc_entries[0];
        Ok(Token::HereDoc {
            strip_tabs: first.strip_tabs,
            expand: first.expand,
            delimiter: std::mem::take(&mut first.delimiter),
            body,
            body_line,
        })
    }

    fn produce_io_number_or_word(&mut self) -> Result<Token, ParseError> {
        let mut digits = Vec::new();
        while let Some(b) = self.peek_byte() {
            if is_digit(b) {
                digits.push(b);
                self.advance_byte();
            } else {
                break;
            }
        }
        self.skip_continuations();
        if matches!(self.peek_byte(), Some(b'<' | b'>')) {
            if let Some(fd) = parse_i32_bytes(&digits) {
                return Ok(Token::IoNumber(fd));
            }
        }
        self.produce_word_with_prefix(digits)
    }

    #[inline(always)]
    fn produce_word_token(&mut self) -> Result<Token, ParseError> {
        self.word_raw.clear();
        self.produce_word()
    }

    fn produce_word_with_prefix(&mut self, prefix: Vec<u8>) -> Result<Token, ParseError> {
        self.word_raw.clear();
        self.word_raw.extend_from_slice(&prefix);
        self.produce_word()
    }

    fn produce_word(&mut self) -> Result<Token, ParseError> {
        let check_keyword = self.keyword_mode;
        let check_alias = self.alias_mode || self.alias_trailing_blank_pending;
        if !self.alias_trailing_blank_pending && self.alias_stack.len() == 1 {
            self.expanding_aliases.clear();
        }
        if self.alias_trailing_blank_pending {
            self.alias_trailing_blank_pending = false;
        }

        self.word_parts.clear();
        self.word_qbuf.clear();
        loop {
            if self.word_raw.is_empty() {
                if self.cached_byte.is_none() || matches!(self.peek_byte(), Some(b) if is_delim(b))
                {
                    return Ok(Token::Eof);
                }
            }

            #[rustfmt::skip]
            let initial_lit = if self.word_parts.is_empty() { 0 } else { self.word_raw.len() };
            let (had_quote, lit_start) = self.scan_raw_word_parts_scratch(initial_lit)?;
            flush_literal(
                &self.word_raw,
                lit_start,
                self.word_raw.len(),
                &mut self.word_parts,
            );
            if self.word_raw.is_empty() {
                return Ok(Token::Eof);
            }

            if !had_quote {
                if check_alias {
                    if let Some((key, value)) = self.aliases.get_key_value(self.word_raw.as_slice())
                    {
                        if is_alias_eligible(&self.word_raw)
                            && !self.expanding_aliases.contains(self.word_raw.as_slice())
                            && self.alias_depth < 1024
                        {
                            let value: &[u8] = value;
                            let trailing_blank = alias_has_trailing_blank(value);
                            self.expanding_aliases.insert(Cow::Borrowed(&**key));
                            self.word_raw.clear();
                            self.word_parts.clear();
                            self.word_qbuf.clear();
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
                    if let Some(kw_tok) = word_to_keyword_token(&self.word_raw) {
                        return Ok(kw_tok);
                    }
                }
            }

            flush_quoted_buf(&mut self.word_qbuf, &mut self.word_parts);
            let raw = std::mem::take(&mut self.word_raw);
            let parts = std::mem::take(&mut self.word_parts);
            return Ok(Token::Word(
                raw.into_boxed_slice(),
                parts.into_boxed_slice(),
            ));
        }
    }

    fn scan_raw_word(&mut self, raw: &mut Vec<u8>) -> Result<bool, ParseError> {
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
                            #[cfg(not(coverage))]
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
                            raw.push(b'\\');
                            raw.push(b);
                            self.advance_byte();
                            had_quote = true;
                        }
                        None => {
                            raw.push(b'\\');
                            had_quote = true;
                        }
                    }
                }
                Some(b'\'') => {
                    had_quote = true;
                    raw.push(b'\'');
                    self.advance_byte();
                    self.consume_single_quote(raw)?;
                }
                Some(b'"') => {
                    had_quote = true;
                    raw.push(b'"');
                    self.advance_byte();
                    self.consume_double_quote(raw)?;
                }
                Some(b'$') => {
                    raw.push(b'$');
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
                    raw.push(b'`');
                    self.advance_byte();
                    had_quote = true;
                    self.consume_backtick_inner(raw)?;
                }
                Some(b) => {
                    if self.pushed_back_byte.is_none() && self.alias_stack.len() == 1 {
                        let layer = &mut self.alias_stack[0];
                        let bytes = &*layer.text;
                        let start = layer.pos;
                        let mut pos = start + 1;
                        while pos < bytes.len() {
                            if is_word_break(bytes[pos]) || is_quote(bytes[pos]) {
                                break;
                            }
                            pos += 1;
                        }
                        raw.extend_from_slice(&bytes[start..pos]);
                        layer.pos = pos;
                        self.cached_byte = bytes.get(pos).copied();
                    } else {
                        raw.push(b);
                        self.advance_byte();
                    }
                }
            }
        }
        Ok(had_quote)
    }

    /// Scans a word from the input, accumulating raw bytes and building WordParts.
    /// `initial_lit_start` is the position in `raw` where the current literal
    fn scan_raw_word_parts_scratch(
        &mut self,
        initial_lit_start: usize,
    ) -> Result<(bool, usize), ParseError> {
        let raw = &mut self.word_raw as *mut Vec<u8>;
        let parts = &mut self.word_parts as *mut Vec<WordPart>;
        let qbuf = &mut self.word_qbuf as *mut Vec<u8>;
        // SAFETY: scratch buffers are disjoint from all other Parser fields
        // accessed by scan_raw_word_parts.
        unsafe { self.scan_raw_word_parts(&mut *raw, &mut *parts, &mut *qbuf, initial_lit_start) }
    }

    /// region begins (typically 0 for a fresh word, or raw.len() if a prefix was
    /// already flushed).
    /// Returns `(had_quote, literal_start)` where `literal_start` is the position
    /// in `raw` from which any trailing literal bytes should be flushed by the caller.
    fn scan_raw_word_parts(
        &mut self,
        raw: &mut Vec<u8>,
        parts: &mut Vec<WordPart>,
        qbuf: &mut Vec<u8>,
        initial_lit_start: usize,
    ) -> Result<(bool, usize), ParseError> {
        let mut had_quote = false;
        let mut lit_start = initial_lit_start;

        if raw.is_empty() && parts.is_empty() && self.peek_byte() == Some(b'~') {
            raw.push(b'~');
            self.advance_byte();
            let user_start = raw.len();
            let broke_on_quote = loop {
                match self.peek_byte() {
                    Some(b'/') => break false,
                    Some(b) if is_word_break(b) => break false,
                    None => break false,
                    Some(b) if is_quote(b) => break true,
                    Some(b) => {
                        raw.push(b);
                        self.advance_byte();
                    }
                }
            };
            let user_end = raw.len();
            if !broke_on_quote {
                if self.peek_byte() == Some(b'/') {
                    raw.push(b'/');
                    self.advance_byte();
                    while let Some(b) = self.peek_byte() {
                        if is_word_break(b) || is_quote(b) {
                            break;
                        }
                        raw.push(b);
                        self.advance_byte();
                    }
                }
                let end = raw.len();
                let (tilde_user_end, tilde_end) = if user_end > user_start || end > 1 {
                    (user_end, end)
                } else {
                    (1, 1)
                };
                parts.push(WordPart::TildeLiteral {
                    tilde_pos: 0,
                    user_end: tilde_user_end,
                    end: tilde_end,
                });
                lit_start = raw.len();
                if self.peek_byte().is_none()
                    || matches!(self.peek_byte(), Some(b) if is_word_break(b))
                {
                    return Ok((had_quote, lit_start));
                }
            } else {
                lit_start = 0;
            }
        }

        loop {
            match self.peek_byte() {
                None => break,
                Some(b) if is_word_break(b) => break,
                Some(b'#') if raw.is_empty() => break,
                Some(b'\\') => {
                    flush_literal(raw, lit_start, raw.len(), parts);
                    self.advance_byte();
                    match self.peek_byte() {
                        Some(b'\n') => {
                            self.advance_byte();
                            if raw.is_empty() {
                                self.skip_blanks_and_comments();
                                if self.cached_byte.is_none()
                                    || matches!(self.peek_byte(), Some(b) if is_delim(b))
                                {
                                    lit_start = raw.len();
                                    break;
                                }
                            }
                        }
                        Some(b) => {
                            raw.push(b'\\');
                            raw.push(b);
                            self.advance_byte();
                            had_quote = true;
                            qbuf.push(b);
                        }
                        None => {
                            raw.push(b'\\');
                            had_quote = true;
                        }
                    }
                    lit_start = raw.len();
                }
                Some(b'\'') => {
                    flush_literal(raw, lit_start, raw.len(), parts);
                    had_quote = true;
                    raw.push(b'\'');
                    self.advance_byte();
                    let content_start = raw.len();
                    self.consume_single_quote(raw)?;
                    let content_end = raw.len() - 1;
                    if content_start == content_end {
                        flush_quoted_buf(qbuf, parts);
                        parts.push(WordPart::QuotedLiteral {
                            bytes: Box::new([]),
                            newlines: 0,
                        });
                    } else {
                        qbuf.extend_from_slice(&raw[content_start..content_end]);
                    }
                    lit_start = raw.len();
                }
                Some(b'"') => {
                    flush_literal(raw, lit_start, raw.len(), parts);
                    had_quote = true;
                    raw.push(b'"');
                    self.advance_byte();
                    let raw_before = raw.len();
                    self.consume_double_quote(raw)?;
                    let dq_raw = &raw[raw_before..raw.len() - 1];
                    if dq_raw.is_empty() {
                        flush_quoted_buf(qbuf, parts);
                        parts.push(WordPart::QuotedLiteral {
                            bytes: Box::new([]),
                            newlines: 0,
                        });
                    } else {
                        self.build_double_quote_parts(dq_raw, raw_before, parts, qbuf);
                    }
                    lit_start = raw.len();
                }
                Some(b'$') => {
                    flush_literal(raw, lit_start, raw.len(), parts);
                    flush_quoted_buf(qbuf, parts);
                    let raw_start = raw.len();
                    raw.push(b'$');
                    self.advance_byte();
                    self.skip_continuations();
                    match self.peek_byte() {
                        Some(b'\'') => {
                            had_quote = true;
                            let sq_start = raw.len();
                            self.consume_dollar_single_quote(raw)?;
                            let body = &raw[sq_start + 1..raw.len() - 1];
                            let decoded =
                                crate::expand::parameter::parse_dollar_single_quoted_body(body);
                            qbuf.extend_from_slice(&decoded);
                        }
                        Some(b'(' | b'{') => {
                            self.consume_dollar_construct(raw)?;
                            let raw_end = raw.len();
                            let dollar_raw = &raw[raw_start..raw_end];
                            self.build_dollar_parts(
                                dollar_raw, raw_start, raw_end, parts, qbuf, false,
                            );
                        }
                        Some(c) if is_special_param(c) => {
                            let ch = self.peek_byte().unwrap();
                            raw.push(ch);
                            self.advance_byte();
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::SpecialVar { ch },
                                quoted: false,
                            });
                        }
                        Some(b'0') => {
                            raw.push(b'0');
                            self.advance_byte();
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::ShellName,
                                quoted: false,
                            });
                        }
                        Some(b'1'..=b'9') => {
                            let ch = self.peek_byte().unwrap();
                            raw.push(ch);
                            self.advance_byte();
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::Positional { index: ch - b'0' },
                                quoted: false,
                            });
                        }
                        Some(c) if is_name_start(c) => {
                            let name_start = raw.len();
                            raw.push(c);
                            self.advance_byte();
                            while let Some(c2) = self.peek_byte() {
                                if is_name_cont(c2) {
                                    raw.push(c2);
                                    self.advance_byte();
                                } else {
                                    break;
                                }
                            }
                            let name_end = raw.len();
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::SimpleVar {
                                    start: name_start,
                                    end: name_end,
                                },
                                quoted: false,
                            });
                        }
                        _ => {
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::LiteralDollar,
                                quoted: false,
                            });
                        }
                    }
                    lit_start = raw.len();
                }
                Some(b'`') => {
                    flush_literal(raw, lit_start, raw.len(), parts);
                    flush_quoted_buf(qbuf, parts);
                    let bt_start = raw.len();
                    raw.push(b'`');
                    self.advance_byte();
                    had_quote = true;
                    self.consume_backtick_inner(raw)?;
                    let bt_end = raw.len();
                    let body_raw = &raw[bt_start + 1..bt_end - 1];
                    let body = unescape_backtick(body_raw, false);
                    let program = crate::syntax::parse(&body).unwrap_or_default();
                    parts.push(WordPart::Expansion {
                        kind: ExpansionKind::Command {
                            program: Rc::new(program),
                        },
                        quoted: false,
                    });
                    lit_start = raw.len();
                }
                Some(_b) => {
                    flush_quoted_buf(qbuf, parts);
                    if self.pushed_back_byte.is_none() && self.alias_stack.len() == 1 {
                        let layer = &mut self.alias_stack[0];
                        let bytes = &*layer.text;
                        let start = layer.pos;
                        let mut pos = start + 1;
                        while pos < bytes.len() {
                            if is_word_break(bytes[pos]) || is_quote(bytes[pos]) {
                                break;
                            }
                            pos += 1;
                        }
                        raw.extend_from_slice(&bytes[start..pos]);
                        layer.pos = pos;
                        self.cached_byte = bytes.get(pos).copied();
                    } else {
                        raw.push(_b);
                        self.advance_byte();
                    }
                }
            }
        }
        Ok((had_quote, lit_start))
    }

    fn build_double_quote_parts(
        &self,
        dq_raw: &[u8],
        abs_offset: usize,
        parts: &mut Vec<WordPart>,
        qbuf: &mut Vec<u8>,
    ) {
        let mut i = 0;
        while i < dq_raw.len() {
            match dq_raw[i] {
                b'\\' if i + 1 < dq_raw.len() => {
                    let next = dq_raw[i + 1];
                    if matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n' | b'}') {
                        if next != b'\n' {
                            qbuf.push(next);
                        }
                        i += 2;
                    } else {
                        qbuf.push(b'\\');
                        i += 1;
                    }
                }
                b'$' => {
                    flush_quoted_buf(qbuf, parts);
                    let remaining = &dq_raw[i..];
                    let (kind, consumed) =
                        classify_dollar_from_slice(remaining, true, abs_offset + i);
                    parts.push(WordPart::Expansion { kind, quoted: true });
                    i += consumed;
                }
                b'`' => {
                    flush_quoted_buf(qbuf, parts);
                    let bt_end = find_backtick_end_in_slice(dq_raw, i + 1);
                    let body_raw = &dq_raw[i + 1..bt_end];
                    let body = unescape_backtick(body_raw, true);
                    let program = crate::syntax::parse(&body).unwrap_or_default();
                    parts.push(WordPart::Expansion {
                        kind: ExpansionKind::Command {
                            program: Rc::new(program),
                        },
                        quoted: true,
                    });
                    i = (bt_end + 1).min(dq_raw.len());
                }
                _ => {
                    qbuf.push(dq_raw[i]);
                    i += 1;
                }
            }
        }
    }

    fn build_dollar_parts(
        &self,
        dollar_raw: &[u8],
        raw_start: usize,
        _raw_end: usize,
        parts: &mut Vec<WordPart>,
        qbuf: &mut Vec<u8>,
        quoted: bool,
    ) {
        debug_assert!(dollar_raw.len() >= 2);
        flush_quoted_buf(qbuf, parts);
        let (kind, _consumed) = classify_dollar_from_slice(dollar_raw, quoted, raw_start);
        parts.push(WordPart::Expansion { kind, quoted });
    }
}

/// Classifies a `$...` expansion from a byte slice.
/// `base` is added to all position offsets in the returned `ExpansionKind`
/// so they reference positions in the full `Word.raw` buffer.
fn classify_dollar_from_slice(slice: &[u8], _quoted: bool, base: usize) -> (ExpansionKind, usize) {
    if slice.len() < 2 {
        return (ExpansionKind::LiteralDollar, 1);
    }
    let c1 = slice[1];
    match c1 {
        b'{' => {
            let brace_end = find_closing_brace(slice, 2, slice.len());
            let consumed = if brace_end < slice.len() {
                brace_end + 1
            } else {
                slice.len()
            };
            let expr = &slice[2..brace_end];
            if expr.is_empty() {
                return (
                    ExpansionKind::Braced {
                        name: BracedName::Var {
                            start: base + 2,
                            end: base + 2,
                        },
                        op: BracedOp::None,
                        parts: Box::new([]),
                    },
                    consumed,
                );
            }
            let is_length = expr[0] == b'#'
                && expr.len() > 1
                && !matches!(expr[1], b'}' | b'-' | b'=' | b'?' | b'+' | b'%' | b'#');
            let name_offset = if is_length { 1 } else { 0 };
            let name_rel_end = parse_braced_name_end(&expr[name_offset..]);
            let rel_name_start = 2 + name_offset;
            let rel_name_end = rel_name_start + name_rel_end;
            let braced_name = classify_braced_name(
                &slice[rel_name_start..rel_name_end],
                base + rel_name_start,
                base + rel_name_end,
            );
            if is_length {
                return (
                    ExpansionKind::Braced {
                        name: braced_name,
                        op: BracedOp::Length,
                        parts: Box::new([]),
                    },
                    consumed,
                );
            }
            let (op, word_rel_start) = classify_braced_op(slice, rel_name_end);
            if op == BracedOp::None
                && word_rel_start >= brace_end
                && matches!(braced_name, BracedName::Var { start, end } if start != end)
            {
                let (start, end) = braced_name.name_range();
                return (ExpansionKind::SimpleVar { start, end }, consumed);
            }
            let word_parts = if word_rel_start < brace_end {
                build_word_parts_for_slice(slice, word_rel_start, brace_end, base)
            } else {
                Box::new([])
            };
            (
                ExpansionKind::Braced {
                    name: braced_name,
                    op,
                    parts: word_parts,
                },
                consumed,
            )
        }
        b'(' => {
            if slice.get(2) == Some(&b'(') {
                let arith_end = find_arith_end(slice, 3, slice.len());
                let consumed = if arith_end + 2 <= slice.len() {
                    arith_end + 2
                } else {
                    slice.len()
                };
                let arith_parts = build_word_parts_impl(slice, 3, arith_end, base, false);
                if let [WordPart::Literal { start, end, .. }] = &*arith_parts {
                    return (
                        ExpansionKind::ArithmeticLiteral {
                            start: *start,
                            end: *end,
                        },
                        consumed,
                    );
                }
                (ExpansionKind::Arithmetic { parts: arith_parts }, consumed)
            } else {
                let paren_end = find_closing_paren(slice, 2, slice.len());
                let body = &slice[2..paren_end];
                let program = crate::syntax::parse(body).unwrap_or_default();
                let consumed = if paren_end < slice.len() {
                    paren_end + 1
                } else {
                    slice.len()
                };
                (
                    ExpansionKind::Command {
                        program: Rc::new(program),
                    },
                    consumed,
                )
            }
        }
        _ if is_special_param(c1) => (ExpansionKind::SpecialVar { ch: c1 }, 2),
        b'0' => (ExpansionKind::ShellName, 2),
        b'1'..=b'9' => (ExpansionKind::Positional { index: c1 - b'0' }, 2),
        _ if is_name_start(c1) => {
            let mut i = 2;
            while i < slice.len() && is_name_cont(slice[i]) {
                i += 1;
            }
            (
                ExpansionKind::SimpleVar {
                    start: base + 1,
                    end: base + i,
                },
                i,
            )
        }
        _ => (ExpansionKind::LiteralDollar, 1),
    }
}

fn classify_braced_name(name_bytes: &[u8], start: usize, end: usize) -> BracedName {
    if name_bytes.is_empty() {
        return BracedName::Var { start, end };
    }
    let b0 = name_bytes[0];
    if is_digit(b0) {
        if let Some(index) = parse_u32_digits(name_bytes) {
            return BracedName::Positional { start, end, index };
        }
        return BracedName::Var { start, end };
    }
    if is_special_param(b0) && name_bytes.len() == 1 {
        return BracedName::Special { start, end, ch: b0 };
    }
    BracedName::Var { start, end }
}

fn parse_u32_digits(b: &[u8]) -> Option<u32> {
    if b.is_empty() {
        return None;
    }
    let mut result: u32 = 0;
    for &d in b {
        if !is_digit(d) {
            return None;
        }
        result = result.checked_mul(10)?.checked_add((d - b'0') as u32)?;
    }
    Some(result)
}

fn find_backtick_end_in_slice(raw: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < raw.len() && raw[i] != b'`' {
        i += if raw[i] == b'\\' { 2 } else { 1 };
    }
    i.min(raw.len())
}

fn flush_quoted_buf(qbuf: &mut Vec<u8>, parts: &mut Vec<WordPart>) {
    if !qbuf.is_empty() {
        parts.push(WordPart::QuotedLiteral {
            newlines: qbuf.iter().filter(|&&b| b == b'\n').count() as u16,
            bytes: Box::from(qbuf.as_slice()),
        });
        qbuf.clear();
    }
}

fn flush_literal(raw: &[u8], start: usize, end: usize, parts: &mut Vec<WordPart>) {
    if start >= end {
        return;
    }
    let span = &raw[start..end];
    let has_glob = span.iter().any(|&b| is_glob_char(b));
    let newlines = span.iter().filter(|&&b| b == b'\n').count() as u16;
    if let Some(WordPart::Literal {
        end: prev_end,
        has_glob: prev_glob,
        newlines: prev_nl,
        ..
    }) = parts
        .last_mut()
        .filter(|p| matches!(p, WordPart::Literal { end, .. } if *end == start))
    {
        *prev_end = end;
        *prev_glob |= has_glob;
        *prev_nl += newlines;
    } else {
        parts.push(WordPart::Literal {
            start,
            end,
            has_glob,
            newlines,
        });
    }
}

fn classify_braced_op(expr: &[u8], name_end: usize) -> (BracedOp, usize) {
    let rest = &expr[name_end..];
    if rest.is_empty() {
        return (BracedOp::None, name_end);
    }
    match rest[0] {
        b':' if rest.len() > 1 => match rest[1] {
            b'-' => (BracedOp::DefaultColon, name_end + 2),
            b'=' => (BracedOp::AssignColon, name_end + 2),
            b'?' => (BracedOp::ErrorColon, name_end + 2),
            b'+' => (BracedOp::AltColon, name_end + 2),
            _ => (BracedOp::None, name_end),
        },
        b'-' => (BracedOp::Default, name_end + 1),
        b'=' => (BracedOp::Assign, name_end + 1),
        b'?' => (BracedOp::Error, name_end + 1),
        b'+' => (BracedOp::Alt, name_end + 1),
        b'%' if rest.len() > 1 && rest[1] == b'%' => (BracedOp::TrimSuffixLong, name_end + 2),
        b'%' => (BracedOp::TrimSuffix, name_end + 1),
        b'#' if rest.len() > 1 && rest[1] == b'#' => (BracedOp::TrimPrefixLong, name_end + 2),
        b'#' => (BracedOp::TrimPrefix, name_end + 1),
        _ => (BracedOp::None, name_end),
    }
}

fn parse_braced_name_end(expr: &[u8]) -> usize {
    if expr.is_empty() {
        return 0;
    }
    let b0 = expr[0];
    if is_digit(b0) {
        let mut i = 0;
        while i < expr.len() && is_digit(expr[i]) {
            i += 1;
        }
        return i;
    }
    if is_special_param(b0) {
        return 1;
    }
    if is_name_start(b0) {
        let mut i = 0;
        while i < expr.len() && is_name_cont(expr[i]) {
            i += 1;
        }
        return i;
    }
    0
}

/// Builds `WordPart` entries for a sub-range of a raw byte buffer.
/// `base` is added to all positional offsets so they reference positions
/// in the full `Word.raw` buffer (use 0 when `raw` is already the full buffer).
fn build_word_parts_for_slice(
    raw: &[u8],
    start: usize,
    end: usize,
    base: usize,
) -> Box<[WordPart]> {
    build_word_parts_impl(raw, start, end, base, true)
}

fn build_word_parts_impl(
    raw: &[u8],
    start: usize,
    end: usize,
    base: usize,
    allow_tilde: bool,
) -> Box<[WordPart]> {
    let mut parts = Vec::new();
    let mut qbuf = Vec::new();
    let mut i = start;
    if allow_tilde && i < end && raw[i] == b'~' {
        let tilde_pos = base + i;
        i += 1;
        while i < end && !is_tilde_user_break(raw[i]) {
            i += 1;
        }
        let user_end = base + i;
        if i < end && raw[i] == b'/' {
            i += 1;
            while i < end && !is_quote(raw[i]) {
                i += 1;
            }
        }
        let tilde_end = base + i;
        if user_end > tilde_pos + 1 || tilde_end > tilde_pos + 1 {
            parts.push(WordPart::TildeLiteral {
                tilde_pos,
                user_end,
                end: tilde_end,
            });
        } else {
            parts.push(WordPart::TildeLiteral {
                tilde_pos,
                user_end: tilde_pos + 1,
                end: tilde_pos + 1,
            });
        }
    }
    while i < end {
        match raw[i] {
            b'\'' => {
                i += 1;
                while i < end && raw[i] != b'\'' {
                    qbuf.push(raw[i]);
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < end && raw[i] != b'"' {
                    match raw[i] {
                        b'\\' if i + 1 < end => {
                            let next = raw[i + 1];
                            if matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n' | b'}') {
                                if next != b'\n' {
                                    qbuf.push(next);
                                }
                                i += 2;
                            } else {
                                qbuf.push(b'\\');
                                i += 1;
                            }
                        }
                        b'$' => {
                            flush_quoted_buf(&mut qbuf, &mut parts);
                            let (kind, consumed) =
                                classify_dollar_from_slice(&raw[i..end], true, base + i);
                            parts.push(WordPart::Expansion { kind, quoted: true });
                            i += consumed;
                        }
                        b'`' => {
                            flush_quoted_buf(&mut qbuf, &mut parts);
                            let bt_end = find_backtick_end(raw, i + 1, end);
                            let body = unescape_backtick(&raw[i + 1..bt_end], true);
                            let program = crate::syntax::parse(&body).unwrap_or_default();
                            parts.push(WordPart::Expansion {
                                kind: ExpansionKind::Command {
                                    program: Rc::new(program),
                                },
                                quoted: true,
                            });
                            i = if bt_end < end { bt_end + 1 } else { bt_end };
                        }
                        _ => {
                            qbuf.push(raw[i]);
                            i += 1;
                        }
                    }
                }
                if i < end {
                    i += 1;
                }
            }
            b'\\' => {
                i += 1;
                if i < end && raw[i] != b'\n' {
                    qbuf.push(raw[i]);
                }
                if i < end {
                    i += 1;
                }
            }
            b'$' => {
                flush_quoted_buf(&mut qbuf, &mut parts);
                let (kind, consumed) = classify_dollar_from_slice(&raw[i..end], false, base + i);
                parts.push(WordPart::Expansion {
                    kind,
                    quoted: false,
                });
                i += consumed;
            }
            b'`' => {
                flush_quoted_buf(&mut qbuf, &mut parts);
                let bt_end = find_backtick_end(raw, i + 1, end);
                let body = unescape_backtick(&raw[i + 1..bt_end], false);
                let program = crate::syntax::parse(&body).unwrap_or_default();
                parts.push(WordPart::Expansion {
                    kind: ExpansionKind::Command {
                        program: Rc::new(program),
                    },
                    quoted: false,
                });
                i = if bt_end < end { bt_end + 1 } else { bt_end };
            }
            _ => {
                let lit_start = i;
                while i < end && !is_quote(raw[i]) {
                    i += 1;
                }
                flush_quoted_buf(&mut qbuf, &mut parts);
                let span = &raw[lit_start..i];
                let has_glob = span.iter().any(|&b| is_glob_char(b));
                let newlines = span.iter().filter(|&&b| b == b'\n').count() as u16;
                parts.push(WordPart::Literal {
                    start: base + lit_start,
                    end: base + i,
                    has_glob,
                    newlines,
                });
            }
        }
    }
    flush_quoted_buf(&mut qbuf, &mut parts);
    parts.into_boxed_slice()
}

fn find_closing_brace(raw: &[u8], start: usize, end: usize) -> usize {
    let mut i = start;
    while i < end {
        match raw[i] {
            b'}' => return i,
            b'\\' => {
                i += 2;
            }
            b'\'' => {
                i += 1;
                while i < end && raw[i] != b'\'' {
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < end && raw[i] != b'"' {
                    if raw[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'$' if i + 1 < end && raw[i + 1] == b'{' => {
                i += 2;
                let inner = find_closing_brace(raw, i, end);
                i = if inner < end { inner + 1 } else { inner };
            }
            b'$' if i + 1 < end && raw[i + 1] == b'(' => {
                if i + 2 < end && raw[i + 2] == b'(' {
                    i += 3;
                    let inner = find_arith_end(raw, i, end);
                    i = if inner + 2 <= end { inner + 2 } else { end };
                } else {
                    i += 2;
                    let inner = find_closing_paren(raw, i, end);
                    i = if inner < end { inner + 1 } else { inner };
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    end
}

fn find_closing_paren(raw: &[u8], start: usize, end: usize) -> usize {
    let mut i = start;
    let mut depth = 1usize;
    while i < end {
        match raw[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
                i += 1;
            }
            b'\'' => {
                i += 1;
                while i < end && raw[i] != b'\'' {
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < end && raw[i] != b'"' {
                    if raw[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'\\' => {
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
    end
}

fn find_arith_end(raw: &[u8], start: usize, end: usize) -> usize {
    let mut i = start;
    let mut depth = 1usize;
    while i < end {
        match raw[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                if depth == 1 && i + 1 < end && raw[i + 1] == b')' {
                    return i;
                }
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'\'' => {
                i += 1;
                while i < end && raw[i] != b'\'' {
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < end && raw[i] != b'"' {
                    if raw[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i < end {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    end
}

fn find_backtick_end(raw: &[u8], start: usize, end: usize) -> usize {
    let mut i = start;
    while i < end {
        if raw[i] == b'`' {
            return i;
        }
        if raw[i] == b'\\' {
            i += 2;
        } else {
            i += 1;
        }
    }
    end
}

fn unescape_backtick(raw: &[u8], in_double_quotes: bool) -> Vec<u8> {
    let mut result = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\' && i + 1 < raw.len() {
            let next = raw[i + 1];
            let special = if in_double_quotes {
                matches!(next, b'$' | b'`' | b'\\' | b'"' | b'\n')
            } else {
                matches!(next, b'$' | b'`' | b'\\')
            };
            if special {
                result.push(next);
                i += 2;
                continue;
            }
        }
        result.push(raw[i]);
        i += 1;
    }
    result
}

pub(super) struct SavedAliasState {
    layers: Vec<AliasLayer<'static>>,
    depth: usize,
    expanding_aliases: HashSet<Vec<u8>>,
    trailing_blank_pending: bool,
}

pub(super) fn parse_here_doc_delimiter(raw: &[u8]) -> (Box<[u8]>, bool) {
    let mut delimiter = Vec::new();
    let mut index = 0usize;
    let mut expand = true;

    while index < raw.len() {
        match raw[index] {
            b'\'' => {
                expand = false;
                index += 1;
                while index < raw.len() {
                    if raw[index] == b'\'' {
                        index += 1;
                        break;
                    }
                    delimiter.push(raw[index]);
                    index += 1;
                }
            }
            b'"' => {
                expand = false;
                index += 1;
                while index < raw.len() {
                    match raw[index] {
                        b'"' => {
                            index += 1;
                            break;
                        }
                        b'\\' if index + 1 < raw.len() => {
                            let next = raw[index + 1];
                            if matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n') {
                                index += 1;
                                delimiter.push(raw[index]);
                                index += 1;
                            } else {
                                delimiter.push(b'\\');
                                index += 1;
                            }
                        }
                        ch => {
                            delimiter.push(ch);
                            index += 1;
                        }
                    }
                }
            }
            b'$' if index + 1 < raw.len() && raw[index + 1] == b'\'' => {
                expand = false;
                index += 2;
                while index < raw.len() {
                    match raw[index] {
                        b'\'' => {
                            index += 1;
                            break;
                        }
                        b'\\' if index + 1 < raw.len() => {
                            index += 1;
                            delimiter.push(raw[index]);
                            index += 1;
                        }
                        ch => {
                            delimiter.push(ch);
                            index += 1;
                        }
                    }
                }
            }
            b'\\' => {
                expand = false;
                index += 1;
                if index < raw.len() {
                    delimiter.push(raw[index]);
                    index += 1;
                }
            }
            ch => {
                delimiter.push(ch);
                index += 1;
            }
        }
    }

    (delimiter.into_boxed_slice(), expand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_i32_bytes_empty_returns_none() {
        assert_eq!(parse_i32_bytes(b""), None);
    }

    #[test]
    fn parse_i32_bytes_non_digit_returns_none() {
        assert_eq!(parse_i32_bytes(b"abc"), None);
        assert_eq!(parse_i32_bytes(b"12x"), None);
        assert_eq!(parse_i32_bytes(b"-1"), None);
    }

    #[test]
    fn parse_i32_bytes_valid_numbers() {
        assert_eq!(parse_i32_bytes(b"0"), Some(0));
        assert_eq!(parse_i32_bytes(b"42"), Some(42));
        assert_eq!(parse_i32_bytes(b"1000"), Some(1000));
    }

    #[test]
    fn classify_dollar_literal_dollar() {
        let (kind, consumed) = classify_dollar_from_slice(b"$", false, 0);
        assert!(matches!(kind, ExpansionKind::LiteralDollar));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn classify_dollar_empty_braces() {
        let (kind, consumed) = classify_dollar_from_slice(b"${}", false, 0);
        assert!(matches!(
            kind,
            ExpansionKind::Braced {
                op: BracedOp::None,
                ..
            }
        ));
        assert_eq!(consumed, 3);
    }

    #[test]
    fn classify_dollar_unterminated_braces() {
        let (kind, consumed) = classify_dollar_from_slice(b"${x", false, 0);
        assert!(matches!(kind, ExpansionKind::SimpleVar { .. }));
        assert_eq!(consumed, 3);
    }

    #[test]
    fn classify_dollar_arith_literal() {
        let (kind, consumed) = classify_dollar_from_slice(b"$((42))", false, 0);
        assert!(matches!(kind, ExpansionKind::ArithmeticLiteral { .. }));
        assert_eq!(consumed, 7);
    }

    #[test]
    fn classify_dollar_arith_complex() {
        let (kind, consumed) = classify_dollar_from_slice(b"$(($x+1))", false, 0);
        assert!(matches!(kind, ExpansionKind::Arithmetic { .. }));
        assert_eq!(consumed, 9);
    }

    #[test]
    fn classify_dollar_unterminated_arith() {
        let (kind, consumed) = classify_dollar_from_slice(b"$((1+2", false, 0);
        assert!(matches!(kind, ExpansionKind::ArithmeticLiteral { .. }));
        assert_eq!(consumed, 6);
    }

    #[test]
    fn classify_dollar_command_sub() {
        let (kind, consumed) = classify_dollar_from_slice(b"$(echo hi)", false, 0);
        assert!(matches!(kind, ExpansionKind::Command { .. }));
        assert_eq!(consumed, 10);
    }

    #[test]
    fn classify_dollar_unterminated_cmd() {
        let (kind, consumed) = classify_dollar_from_slice(b"$(echo", false, 0);
        assert!(matches!(kind, ExpansionKind::Command { .. }));
        assert_eq!(consumed, 6);
    }

    #[test]
    fn classify_dollar_simple_var() {
        let (kind, consumed) = classify_dollar_from_slice(b"$abc", false, 0);
        assert!(matches!(
            kind,
            ExpansionKind::SimpleVar { start: 1, end: 4 }
        ));
        assert_eq!(consumed, 4);
    }

    #[test]
    fn classify_dollar_unknown_char() {
        let (kind, consumed) = classify_dollar_from_slice(b"$.", false, 0);
        assert!(matches!(kind, ExpansionKind::LiteralDollar));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn classify_dollar_braced_invalid_name_is_not_simple_var() {
        let (kind, _consumed) = classify_dollar_from_slice(b"${/}", false, 0);
        assert!(
            matches!(kind, ExpansionKind::Braced { .. }),
            "${{\"/\"}} with invalid name char must stay Braced, got {kind:?}"
        );

        let (kind, _consumed) = classify_dollar_from_slice(b"${}", false, 0);
        assert!(
            matches!(kind, ExpansionKind::Braced { .. }),
            "empty ${{}} must stay Braced, got {kind:?}"
        );
    }

    #[test]
    fn classify_dollar_braced_trailing_junk_is_not_simple_var() {
        let (kind, _consumed) = classify_dollar_from_slice(b"${x!y}", false, 0);
        assert!(
            matches!(kind, ExpansionKind::Braced { .. }),
            "${{x!y}} with trailing junk must stay Braced, got {kind:?}"
        );
    }

    #[test]
    fn classify_braced_name_empty() {
        let n = classify_braced_name(b"", 0, 0);
        assert!(matches!(n, BracedName::Var { start: 0, end: 0 }));
    }

    #[test]
    fn classify_braced_name_positional() {
        let n = classify_braced_name(b"12", 0, 2);
        assert!(matches!(n, BracedName::Positional { index: 12, .. }));
    }

    #[test]
    fn classify_braced_name_digit_overflow() {
        let n = classify_braced_name(b"99999999999999999", 0, 17);
        assert!(matches!(n, BracedName::Var { .. }));
    }

    #[test]
    fn classify_braced_name_special() {
        let n = classify_braced_name(b"?", 0, 1);
        assert!(matches!(n, BracedName::Special { ch: b'?', .. }));
    }

    #[test]
    fn parse_u32_digits_empty() {
        assert_eq!(parse_u32_digits(b""), None);
    }

    #[test]
    fn parse_u32_digits_non_digit() {
        assert_eq!(parse_u32_digits(b"12x"), None);
    }

    #[test]
    fn parse_u32_digits_overflow() {
        assert_eq!(parse_u32_digits(b"99999999999999999"), None);
    }

    #[test]
    fn classify_braced_op_all() {
        assert_eq!(classify_braced_op(b"${x:-w}", 3).0, BracedOp::DefaultColon);
        assert_eq!(classify_braced_op(b"${x:=w}", 3).0, BracedOp::AssignColon);
        assert_eq!(classify_braced_op(b"${x:?w}", 3).0, BracedOp::ErrorColon);
        assert_eq!(classify_braced_op(b"${x:+w}", 3).0, BracedOp::AltColon);
        assert_eq!(classify_braced_op(b"${x-w}", 3).0, BracedOp::Default);
        assert_eq!(classify_braced_op(b"${x=w}", 3).0, BracedOp::Assign);
        assert_eq!(classify_braced_op(b"${x?w}", 3).0, BracedOp::Error);
        assert_eq!(classify_braced_op(b"${x+w}", 3).0, BracedOp::Alt);
        assert_eq!(
            classify_braced_op(b"${x%%w}", 3).0,
            BracedOp::TrimSuffixLong
        );
        assert_eq!(classify_braced_op(b"${x%w}", 3).0, BracedOp::TrimSuffix);
        assert_eq!(
            classify_braced_op(b"${x##w}", 3).0,
            BracedOp::TrimPrefixLong
        );
        assert_eq!(classify_braced_op(b"${x#w}", 3).0, BracedOp::TrimPrefix);
        assert_eq!(classify_braced_op(b"${x}", 3).0, BracedOp::None);
        assert_eq!(classify_braced_op(b"${x:}", 3).0, BracedOp::None);
    }

    #[test]
    fn parse_braced_name_end_cases() {
        assert_eq!(parse_braced_name_end(b""), 0);
        assert_eq!(parse_braced_name_end(b"123"), 3);
        assert_eq!(parse_braced_name_end(b"?"), 1);
        assert_eq!(parse_braced_name_end(b"$"), 1);
        assert_eq!(parse_braced_name_end(b"abc_def"), 7);
        assert_eq!(parse_braced_name_end(b"."), 0);
    }

    #[test]
    fn build_word_parts_for_slice_tilde_at_start() {
        let raw = b"${x:-~/path}";
        let parts = build_word_parts_for_slice(raw, 5, 11, 0);
        assert!(
            matches!(parts[0], WordPart::TildeLiteral { .. }),
            "tilde at start of braced word should produce TildeLiteral, got {:?}",
            parts[0]
        );
    }

    #[test]
    fn build_word_parts_for_slice_bare_tilde() {
        let raw = b"${x:-~}";
        let parts = build_word_parts_for_slice(raw, 5, 6, 0);
        assert!(
            matches!(parts[0], WordPart::TildeLiteral { .. }),
            "bare tilde should produce TildeLiteral, got {:?}",
            parts[0]
        );
    }

    #[test]
    fn build_word_parts_for_slice_dquote_with_dollar() {
        let raw = b"${x:-\"$y\"}";
        let parts = build_word_parts_for_slice(raw, 5, 9, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn build_word_parts_for_slice_dquote_with_backslash() {
        let raw = b"${x:-\"a\\$b\"}";
        let parts = build_word_parts_for_slice(raw, 5, 11, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn build_word_parts_for_slice_dquote_with_backtick() {
        let raw = b"${x:-\"`echo y`\"}";
        let parts = build_word_parts_for_slice(raw, 5, 15, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn build_word_parts_for_slice_top_level_backslash() {
        let raw = b"${x:-a\\nb}";
        let parts = build_word_parts_for_slice(raw, 5, 9, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn build_word_parts_for_slice_top_level_dollar() {
        let raw = b"${x:-$y}";
        let parts = build_word_parts_for_slice(raw, 5, 7, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn build_word_parts_for_slice_top_level_backtick() {
        let raw = b"${x:-`echo z`}";
        let parts = build_word_parts_for_slice(raw, 5, 13, 0);
        assert!(parts.len() >= 1);
    }

    #[test]
    fn find_closing_brace_with_nested() {
        assert_eq!(find_closing_brace(b"${a${b}}", 2, 8), 7);
        assert_eq!(find_closing_brace(b"${a'}'}", 2, 7), 6);
        assert_eq!(find_closing_brace(b"${a\\'}", 2, 6), 5);
        assert_eq!(find_closing_brace(b"${a\"b\"}", 2, 7), 6);
    }

    #[test]
    fn find_closing_brace_with_arith_and_cmd() {
        assert_eq!(find_closing_brace(b"${a$((1+2))}", 2, 12), 11);
        assert_eq!(find_closing_brace(b"${a$(echo)}", 2, 11), 10);
    }

    #[test]
    fn find_closing_brace_with_dquote_backslash() {
        assert_eq!(find_closing_brace(b"${a\"\\\"\"}", 2, 8), 7);
    }

    #[test]
    fn find_closing_brace_unterminated() {
        assert_eq!(find_closing_brace(b"${a", 2, 3), 3);
    }

    #[test]
    fn find_closing_paren_nested() {
        assert_eq!(find_closing_paren(b"$(a(b))", 2, 7), 6);
    }

    #[test]
    fn find_closing_paren_with_quotes() {
        assert_eq!(find_closing_paren(b"$(a')')", 2, 7), 6);
        assert_eq!(find_closing_paren(b"$(a\")\")", 2, 7), 6);
        assert_eq!(find_closing_paren(b"$(a\\))", 2, 6), 5);
    }

    #[test]
    fn find_closing_paren_with_dquote_backslash() {
        assert_eq!(find_closing_paren(b"$(a\"\\\"\")", 2, 8), 7);
    }

    #[test]
    fn find_closing_paren_unterminated() {
        assert_eq!(find_closing_paren(b"$(a", 2, 3), 3);
    }

    #[test]
    fn find_arith_end_nested() {
        assert_eq!(find_arith_end(b"$((1+(2)))", 3, 10), 8);
    }

    #[test]
    fn find_arith_end_with_quotes() {
        assert_eq!(find_arith_end(b"$(('1'))", 3, 8), 6);
        assert_eq!(find_arith_end(b"$((\"1\"))", 3, 8), 6);
    }

    #[test]
    fn find_arith_end_with_dquote_backslash() {
        assert_eq!(find_arith_end(b"$((\"1\\\"\"))", 3, 11), 8);
    }

    #[test]
    fn find_arith_end_unterminated() {
        assert_eq!(find_arith_end(b"$((1+2", 3, 6), 6);
    }

    #[test]
    fn find_backtick_end_basic() {
        assert_eq!(find_backtick_end(b"`echo`", 1, 6), 5);
    }

    #[test]
    fn find_backtick_end_with_escape() {
        assert_eq!(find_backtick_end(b"`a\\`b`", 1, 6), 5);
    }

    #[test]
    fn find_backtick_end_unterminated() {
        assert_eq!(find_backtick_end(b"`abc", 1, 4), 4);
    }

    #[test]
    fn unescape_backtick_outside_dquote() {
        assert_eq!(
            unescape_backtick(b"a\\$b\\`c\\\\d\\e", false),
            b"a$b`c\\d\\e"
        );
    }

    #[test]
    fn unescape_backtick_in_dquote() {
        assert_eq!(
            unescape_backtick(b"a\\$b\\`c\\\\d\\\"e\\f", true),
            b"a$b`c\\d\"e\\f"
        );
    }

    fn bx(s: &[u8]) -> Box<[u8]> {
        s.to_vec().into_boxed_slice()
    }

    #[test]
    fn heredoc_delimiter_helpers() {
        assert_eq!(parse_here_doc_delimiter(b"\"EOF\""), (bx(b"EOF"), false));
        assert_eq!(parse_here_doc_delimiter(b"\\EOF"), (bx(b"EOF"), false));
    }

    #[test]
    fn heredoc_delimiter_backslash_inside_double_quotes() {
        assert_eq!(
            parse_here_doc_delimiter(b"\"ab\\\"cd\""),
            (bx(b"ab\"cd"), false)
        );
        assert_eq!(
            parse_here_doc_delimiter(b"\"a\\\\b\""),
            (bx(b"a\\b"), false)
        );
        assert_eq!(parse_here_doc_delimiter(b"\"a\\$b\""), (bx(b"a$b"), false));
    }

    #[test]
    fn heredoc_delimiter_dollar_single_quote() {
        assert_eq!(parse_here_doc_delimiter(b"$'EOF'"), (bx(b"EOF"), false));
        assert_eq!(
            parse_here_doc_delimiter(b"$'ab\\'cd'"),
            (bx(b"ab'cd"), false)
        );
    }

    #[test]
    fn heredoc_delimiter_backslash_preserves_non_special_in_dquotes() {
        assert_eq!(
            parse_here_doc_delimiter(b"\"E\\OF\""),
            (bx(b"E\\OF"), false)
        );
        assert_eq!(
            parse_here_doc_delimiter(b"\"a\\nb\""),
            (bx(b"a\\nb"), false)
        );

        assert_eq!(parse_here_doc_delimiter(b"\"a\\$b\""), (bx(b"a$b"), false));
        assert_eq!(
            parse_here_doc_delimiter(b"\"a\\\\b\""),
            (bx(b"a\\b"), false)
        );
        assert_eq!(
            parse_here_doc_delimiter(b"\"a\\\"b\""),
            (bx(b"a\"b"), false)
        );
        assert_eq!(parse_here_doc_delimiter(b"\"a\\`b\""), (bx(b"a`b"), false));
    }

    #[test]
    fn heredoc_delimiter_parse_dollar_single_quote() {
        let (delim, expand) = parse_here_doc_delimiter(b"$'EOF'");
        assert_eq!(&*delim, b"EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_parse_double_quoted() {
        let (delim, expand) = parse_here_doc_delimiter(b"\"EOF\"");
        assert_eq!(&*delim, b"EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_parse_backslash_escape() {
        let (delim, expand) = parse_here_doc_delimiter(b"E\\OF");
        assert_eq!(&*delim, b"EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_double_quote_with_backslash() {
        let (delim, expand) = parse_here_doc_delimiter(b"\"E\\$OF\"");
        assert_eq!(&*delim, b"E$OF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_dollar_single_quote_with_escape() {
        let (delim, expand) = parse_here_doc_delimiter(b"$'E\\'OF'");
        assert_eq!(&*delim, b"E'OF");
        assert!(!expand);
    }

    #[test]
    fn display_name_for_keywords_and_word() {
        assert_eq!(&*Token::If.display_name(), b"if");
        assert_eq!(&*Token::Then.display_name(), b"then");
        assert_eq!(&*Token::Else.display_name(), b"else");
        assert_eq!(&*Token::Elif.display_name(), b"elif");
        assert_eq!(&*Token::Fi.display_name(), b"fi");
        assert_eq!(&*Token::Do.display_name(), b"do");
        assert_eq!(&*Token::Done.display_name(), b"done");
        assert_eq!(&*Token::Case.display_name(), b"case");
        assert_eq!(&*Token::Esac.display_name(), b"esac");
        assert_eq!(&*Token::In.display_name(), b"in");
        assert_eq!(&*Token::While.display_name(), b"while");
        assert_eq!(&*Token::Until.display_name(), b"until");
        assert_eq!(&*Token::For.display_name(), b"for");
        assert_eq!(&*Token::Function.display_name(), b"function");
        assert_eq!(&*Token::Bang.display_name(), b"!");
        assert_eq!(&*Token::LBrace.display_name(), b"{");
        assert_eq!(&*Token::RBrace.display_name(), b"}");
        assert_eq!(
            &*Token::Word(bx(b"foo"), Box::new([])).display_name(),
            b"word"
        );
    }

    #[test]
    fn token_into_word_some_and_none() {
        use crate::syntax::word_parts::WordPart;
        let empty_parts: Box<[WordPart]> = Box::new([]);
        assert_eq!(
            Token::Word(bx(b"hi"), Box::new([])).into_word(),
            Some((bx(b"hi"), empty_parts))
        );
        assert_eq!(Token::Eof.into_word(), None);
        assert_eq!(Token::Semi.into_word(), None);
    }

    #[test]
    fn classify_dollar_literal_dollar_at_end() {
        let (kind, consumed) = classify_dollar_from_slice(b"$", false, 10);
        assert!(matches!(kind, ExpansionKind::LiteralDollar));
        assert_eq!(consumed, 1);
    }

    #[test]
    fn classify_dollar_simple_var_in_braces() {
        let (kind, consumed) = classify_dollar_from_slice(b"${HOME}", false, 0);
        assert!(matches!(kind, ExpansionKind::SimpleVar { .. }));
        assert_eq!(consumed, 7);
    }

    #[test]
    fn classify_dollar_arithmetic_literal() {
        let (kind, consumed) = classify_dollar_from_slice(b"$((42))", false, 0);
        assert!(matches!(kind, ExpansionKind::ArithmeticLiteral { .. }));
        assert_eq!(consumed, 7);
    }

    #[test]
    fn backtick_end_unterminated_returns_len() {
        let raw = b"no closing backtick here";
        let result = find_backtick_end(raw, 0, raw.len());
        assert_eq!(result, 24);
    }

    #[test]
    fn backtick_end_with_backslash_escape() {
        let raw = b"echo \\\\ok`rest";
        let result = find_backtick_end(raw, 0, raw.len());
        assert_eq!(result, 9);
    }

    #[test]
    fn flush_literal_merges_contiguous_spans() {
        let raw = b"abcdef";
        let mut parts = Vec::new();
        flush_literal(raw, 0, 3, &mut parts);
        flush_literal(raw, 3, 6, &mut parts);
        assert_eq!(parts.len(), 1);
        if let WordPart::Literal { start, end, .. } = &parts[0] {
            assert_eq!(*start, 0);
            assert_eq!(*end, 6);
        } else {
            panic!("expected Literal");
        }
    }

    #[test]
    fn flush_literal_glob_detection_in_merge() {
        let raw = b"abc*ef";
        let mut parts = Vec::new();
        flush_literal(raw, 0, 3, &mut parts);
        flush_literal(raw, 3, 6, &mut parts);
        assert_eq!(parts.len(), 1);
        if let WordPart::Literal { has_glob, .. } = &parts[0] {
            assert!(*has_glob);
        } else {
            panic!("expected Literal");
        }
    }

    #[test]
    fn build_word_parts_impl_tilde_expansion() {
        let raw = b"~user/path";
        let parts = build_word_parts_impl(raw, 0, raw.len(), 0, true);
        assert!(!parts.is_empty());
        assert!(matches!(parts[0], WordPart::TildeLiteral { .. }));
    }

    #[test]
    fn build_word_parts_impl_dquote_non_special_backslash() {
        let raw = b"\"hello\\wworld\"";
        let parts = build_word_parts_impl(raw, 0, raw.len(), 0, false);
        assert!(!parts.is_empty());
        let has_quoted = parts
            .iter()
            .any(|p| matches!(p, WordPart::QuotedLiteral { .. }));
        assert!(has_quoted);
    }

    #[test]
    fn build_word_parts_impl_unquoted_backslash() {
        let raw = b"hello\\ world";
        let parts = build_word_parts_impl(raw, 0, raw.len(), 0, false);
        assert!(!parts.is_empty());
    }

    #[test]
    fn build_word_parts_impl_dollar_literal() {
        let raw = b"$";
        let parts = build_word_parts_impl(raw, 0, raw.len(), 0, false);
        assert!(!parts.is_empty());
        assert!(parts.iter().any(|p| matches!(
            p,
            WordPart::Expansion {
                kind: ExpansionKind::LiteralDollar,
                ..
            }
        )));
    }

    fn parse_words(input: &[u8]) -> Vec<(Box<[u8]>, Box<[WordPart]>)> {
        let prog = crate::syntax::parse(input).expect("parse");
        let cmd = &prog.items[0].and_or.first.commands[0];
        match cmd {
            crate::syntax::ast::Command::Simple(sc) => sc
                .words
                .iter()
                .map(|w| (w.raw.clone(), w.parts.clone()))
                .collect(),
            _ => panic!("expected simple"),
        }
    }

    #[test]
    fn scan_raw_word_parts_tilde_at_word_end() {
        let words = parse_words(b"echo ~\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(
            parts
                .iter()
                .any(|p| matches!(p, WordPart::TildeLiteral { .. }))
        );
    }

    #[test]
    fn scan_raw_word_parts_tilde_user_at_end() {
        let words = parse_words(b"echo ~user\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(
            parts
                .iter()
                .any(|p| matches!(p, WordPart::TildeLiteral { .. }))
        );
    }

    #[test]
    fn scan_raw_word_parts_tilde_user_path_at_end() {
        let words = parse_words(b"echo ~user/path\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(
            parts
                .iter()
                .any(|p| matches!(p, WordPart::TildeLiteral { .. }))
        );
    }

    #[test]
    fn scan_raw_word_parts_dollar_at_end() {
        let words = parse_words(b"echo hello$\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(parts.iter().any(|p| matches!(
            p,
            WordPart::Expansion {
                kind: ExpansionKind::LiteralDollar,
                ..
            }
        )));
    }

    #[test]
    fn scan_raw_word_parts_braced_simple_var() {
        let words = parse_words(b"echo ${HOME}\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(parts.iter().any(|p| matches!(
            p,
            WordPart::Expansion {
                kind: ExpansionKind::SimpleVar { .. },
                ..
            }
        )));
    }

    #[test]
    fn scan_raw_word_parts_arith_literal() {
        let words = parse_words(b"echo $((42))\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(parts.iter().any(|p| matches!(
            p,
            WordPart::Expansion {
                kind: ExpansionKind::ArithmeticLiteral { .. },
                ..
            }
        )));
    }

    #[test]
    fn heredoc_delimiter_with_backtick() {
        let prog = crate::syntax::parse(b"cat <<`EOF`\nhello\n`EOF`\n").expect("parse");
        assert!(!prog.items.is_empty());
    }

    #[test]
    fn heredoc_delimiter_with_hash() {
        let prog = crate::syntax::parse(b"cat <<\\#END\nhello\n#END\n").expect("parse");
        assert!(!prog.items.is_empty());
    }

    #[test]
    fn heredoc_delimiter_with_dollar() {
        let prog = crate::syntax::parse(b"cat <<$EOF\nhello\n$EOF\n").expect("parse");
        assert!(!prog.items.is_empty());
    }

    #[test]
    fn heredoc_delimiter_backslash_at_eof() {
        let result = crate::syntax::parse(b"cat <<EOF\\\n");
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn heredoc_delimiter_backslash_newline_then_delim() {
        let result = crate::syntax::parse(b"cat <<\\\n;\n");
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn scan_raw_word_parts_resume_after_partial() {
        let words = parse_words(b"echo ~user$HOME\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(!parts.is_empty());
    }

    #[test]
    fn build_dollar_parts_backtick_in_dquotes() {
        let words = parse_words(b"echo \"`echo hi`\"\n");
        assert!(words.len() >= 2);
        let (_, parts) = &words[1];
        assert!(!parts.is_empty());
    }
}
