use std::rc::Rc;

use super::ParseError;
use super::token::{Parser, Token};
use super::word_parts::WordPart;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Program {
    pub(crate) items: Box<[ListItem]>,
}

#[derive(Clone, Debug)]
pub(crate) struct ListItem {
    pub(crate) and_or: AndOr,
    pub(crate) asynchronous: bool,
    pub(crate) line: usize,
}

impl PartialEq for ListItem {
    fn eq(&self, other: &Self) -> bool {
        self.and_or == other.and_or && self.asynchronous == other.asynchronous
    }
}
impl Eq for ListItem {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AndOr {
    pub(crate) first: Pipeline,
    pub(crate) rest: Box<[(LogicalOp, Pipeline)]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogicalOp {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimedMode {
    Off,
    Default,
    Posix,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pipeline {
    pub(crate) negated: bool,
    pub(crate) timed: TimedMode,
    pub(crate) commands: Box<[Command]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
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
pub(crate) struct SimpleCommand {
    pub(crate) assignments: Box<[Assignment]>,
    pub(crate) words: Box<[Word]>,
    pub(crate) redirections: Box<[Redirection]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Assignment {
    pub(crate) name: Box<[u8]>,
    pub(crate) value: Word,
}

#[derive(Clone, Debug)]
pub(crate) struct Word {
    pub(crate) raw: Box<[u8]>,
    pub(crate) parts: Box<[WordPart]>,
    pub(crate) line: usize,
}

impl Word {
    #[cfg(test)]
    pub(crate) fn from_raw(raw: &[u8]) -> Self {
        Word {
            raw: raw.to_vec().into_boxed_slice(),
            parts: Box::new([]),
            line: 0,
        }
    }
}

impl PartialEq for Word {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl Eq for Word {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Redirection {
    pub(crate) fd: Option<i32>,
    pub(crate) kind: RedirectionKind,
    pub(crate) target: Word,
    pub(crate) here_doc: Option<HereDoc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionDef {
    pub(crate) name: Box<[u8]>,
    pub(crate) body: Rc<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IfCommand {
    pub(crate) condition: Program,
    pub(crate) then_branch: Program,
    pub(crate) elif_branches: Box<[ElifBranch]>,
    pub(crate) else_branch: Option<Program>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ElifBranch {
    pub(crate) condition: Program,
    pub(crate) body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoopCommand {
    pub(crate) kind: LoopKind,
    pub(crate) condition: Program,
    pub(crate) body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ForCommand {
    pub(crate) name: Box<[u8]>,
    pub(crate) items: Option<Box<[Word]>>,
    pub(crate) body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaseCommand {
    pub(crate) word: Word,
    pub(crate) arms: Box<[CaseArm]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaseArm {
    pub(crate) patterns: Box<[Word]>,
    pub(crate) body: Program,
    pub(crate) fallthrough: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct HereDoc {
    pub(crate) delimiter: Box<[u8]>,
    pub(crate) body: Box<[u8]>,
    pub(crate) expand: bool,
    pub(crate) strip_tabs: bool,
    pub(crate) body_line: usize,
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
pub(crate) enum LoopKind {
    While,
    Until,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RedirectionKind {
    Read,
    Write,
    ClobberWrite,
    Append,
    HereDoc,
    ReadWrite,
    DupInput,
    DupOutput,
}

pub(super) fn split_assignment(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let pos = input.iter().position(|&b| b == b'=')?;
    let name = &input[..pos];
    let value = &input[pos + 1..];
    if !super::is_name(name) {
        return None;
    }
    Some((name, value))
}

fn build_assignment_value_parts(
    _raw: &[u8],
    _parts: &[WordPart],
    _eq_plus_one: usize,
) -> Box<[WordPart]> {
    Box::new([])
}

impl<'a> Parser<'a> {
    pub(super) fn eat_keyword(&mut self, expected: Token, name: &[u8]) -> Result<(), ParseError> {
        self.set_keyword_position();
        if *self.peek_token()? == expected {
            self.advance_token();
            self.set_command_position();
            self.skip_linebreaks_t()?;
            Ok(())
        } else {
            let mut msg = Vec::with_capacity(12 + name.len());
            msg.extend_from_slice(b"expected '");
            msg.extend_from_slice(name);
            msg.push(b'\'');
            Err(self.error(&msg))
        }
    }

    pub(super) fn skip_separators_t(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_token()? {
                Token::Newline | Token::Semi => {
                    self.advance_token();
                }
                _ => break,
            }
        }
        Ok(())
    }

    pub(super) fn skip_linebreaks_t(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_token()? {
                Token::Newline => {
                    self.advance_token();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn take_word(&mut self) -> (Box<[u8]>, Box<[WordPart]>) {
        self.next_token().into_word().unwrap()
    }

    pub(super) fn parse_program_until(
        &mut self,
        stop: fn(&Token) -> bool,
        stop_on_closer: bool,
        stop_on_dsemi: bool,
    ) -> Result<Program, ParseError> {
        let mut items = Vec::new();
        self.set_command_position();
        self.skip_linebreaks_t()?;

        loop {
            self.set_command_position();

            if stop_on_dsemi && matches!(self.peek_token()?, Token::DSemi | Token::SemiAmp) {
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
            self.set_command_position();
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
            self.set_command_position();
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
        self.set_command_position();

        let timed = if matches!(self.peek_token()?, Token::Word(w, _) if &**w == b"time") {
            self.advance_token();
            self.set_command_position();
            if matches!(self.peek_token()?, Token::Word(w, _) if &**w == b"-p") {
                self.advance_token();
                self.set_command_position();
                TimedMode::Posix
            } else {
                TimedMode::Default
            }
        } else {
            TimedMode::Off
        };

        let negated = if matches!(self.peek_token()?, Token::Bang) {
            self.advance_token();
            self.set_command_position();
            true
        } else {
            false
        };

        let mut commands = vec![self.parse_command()?];
        loop {
            match self.peek_token()? {
                Token::Pipe => {
                    self.advance_token();
                    self.set_command_position();
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
        self.set_command_position();
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
                self.set_command_position();
                self.skip_separators_t()?;
                let body = self.parse_program_until(|_| false, true, false)?;
                if body.items.is_empty() {
                    return Err(self.error(b"expected command list in brace group"));
                }
                if !matches!(self.peek_token()?, Token::RBrace) {
                    return Err(self.error(b"expected '}'"));
                }
                self.advance_token();
                self.set_command_position();
                self.skip_separators_t()?;
                Ok(Command::Group(body))
            }
            Token::LParen => {
                self.advance_token();
                let body = self.parse_program_until(|_| false, true, false)?;
                if body.items.is_empty() {
                    return Err(self.error(b"expected command list in subshell"));
                }
                if !matches!(self.peek_token()?, Token::RParen) {
                    return Err(self.error(b"expected ')' to close subshell"));
                }
                self.advance_token();
                Ok(Command::Subshell(body))
            }
            Token::Bang => Err(self.error(b"expected command")),
            Token::Word(_, _) => {
                let (raw, parts) = self.take_word();
                self.set_argument_position();
                if super::is_name(&raw) && matches!(self.peek_token()?, Token::LParen) {
                    self.advance_token();
                    if matches!(self.peek_token()?, Token::RParen) {
                        self.advance_token();
                        self.set_command_position();
                        self.skip_linebreaks_t().ok();
                        let body = self.parse_command()?;
                        return Ok(Command::FunctionDef(FunctionDef {
                            name: raw,
                            body: Rc::new(body),
                        }));
                    }
                    return Err(self.error(b"syntax error near unexpected token `('"));
                }
                self.parse_simple_command_with_first_word(raw, parts, line)
                    .map(Command::Simple)
            }
            Token::IoNumber(_)
            | Token::Less
            | Token::Great
            | Token::DGreat
            | Token::LessAnd
            | Token::GreatAnd
            | Token::LessGreat
            | Token::Clobber
            | Token::HereDoc { .. } => self
                .parse_simple_command_with_first_redir()
                .map(Command::Simple),
            Token::Eof => Err(self.error(b"expected command")),
            Token::Newline
            | Token::Semi
            | Token::DSemi
            | Token::SemiAmp
            | Token::Amp
            | Token::Pipe
            | Token::OrIf
            | Token::AndIf
            | Token::RParen => Err(self.error(b"expected command")),
            _ => {
                let name = self.peek_token()?.display_name();
                self.advance_token();
                self.parse_simple_command_with_first_word(name, Box::new([]), line)
                    .map(Command::Simple)
            }
        }
    }

    fn parse_simple_command_with_first_word(
        &mut self,
        first_raw: Box<[u8]>,
        first_parts: Box<[WordPart]>,
        first_line: usize,
    ) -> Result<SimpleCommand, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirections = Vec::new();

        if let Some((name, value_raw)) = split_assignment(&first_raw) {
            let value_parts = build_assignment_value_parts(&first_raw, &first_parts, name.len() + 1);
            assignments.push(Assignment {
                name: name.to_vec().into_boxed_slice(),
                value: Word {
                    raw: value_raw.to_vec().into_boxed_slice(),
                    parts: value_parts,
                    line: first_line,
                },
            });
        } else {
            words.push(Word {
                raw: first_raw,
                parts: first_parts,
                line: first_line,
            });
        }

        self.simple_command_scan_loop(&mut assignments, &mut words, &mut redirections)?;

        if !words.is_empty() && matches!(self.peek_token()?, Token::LParen) {
            return Err(self.error(b"syntax error near unexpected token `('"));
        }

        Ok(SimpleCommand {
            assignments: assignments.into_boxed_slice(),
            words: words.into_boxed_slice(),
            redirections: redirections.into_boxed_slice(),
        })
    }

    fn parse_simple_command_with_first_redir(&mut self) -> Result<SimpleCommand, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirections = Vec::new();

        if let Some(redir) = self.try_parse_redirection()? {
            redirections.push(redir);
        }

        self.simple_command_scan_loop(&mut assignments, &mut words, &mut redirections)?;

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
            let at_command_pos =
                words.is_empty() && (!assignments.is_empty() || !redirections.is_empty());
            if at_command_pos {
                self.set_command_position();
            } else {
                self.set_argument_position();
            }
            let line = self.current_line();

            if let Some(redir) = self.try_parse_redirection()? {
                redirections.push(redir);
                continue;
            }

            match self.peek_token()? {
                Token::Word(_, _) => {}
                _ => break,
            }

            let (raw, parts) = self.take_word();
            if words.is_empty() {
                if let Some((name, value_raw)) = split_assignment(&raw) {
                    let value_parts = build_assignment_value_parts(&raw, &parts, name.len() + 1);
                    assignments.push(Assignment {
                        name: name.to_vec().into_boxed_slice(),
                        value: Word {
                            raw: value_raw.to_vec().into_boxed_slice(),
                            parts: value_parts,
                            line,
                        },
                    });
                    continue;
                }
            }
            words.push(Word { raw, parts, line });
        }
        Ok(())
    }

    fn try_parse_redirection(&mut self) -> Result<Option<Redirection>, ParseError> {
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
            if let Token::IoNumber(n) = self.next_token() {
                fd = Some(n);
            }
        }

        let line = self.current_line();
        let _ = self.peek_token()?;
        let tok = self.next_token();

        if let Token::HereDoc {
            delimiter,
            body,
            strip_tabs,
            expand,
            body_line,
        } = tok
        {
            return Ok(Some(Redirection {
                fd,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: delimiter.clone(),
                    parts: Box::new([]),
                    line,
                },
                here_doc: Some(HereDoc {
                    delimiter,
                    body,
                    expand,
                    strip_tabs,
                    body_line,
                }),
            }));
        }

        let kind = if matches!(tok, Token::Less) {
            RedirectionKind::Read
        } else if matches!(tok, Token::Great) {
            RedirectionKind::Write
        } else if matches!(tok, Token::DGreat) {
            RedirectionKind::Append
        } else if matches!(tok, Token::LessAnd) {
            RedirectionKind::DupInput
        } else if matches!(tok, Token::GreatAnd) {
            RedirectionKind::DupOutput
        } else if matches!(tok, Token::LessGreat) {
            RedirectionKind::ReadWrite
        } else {
            RedirectionKind::ClobberWrite
        };
        self.set_argument_position();
        let target_line = self.current_line();
        match self.peek_token()? {
            Token::Word(_, _) => {
                let (w, wp) = self.take_word();
                Ok(Some(Redirection {
                    fd,
                    kind,
                    target: Word {
                        raw: w,
                        parts: wp,
                        line: target_line,
                    },
                    here_doc: None,
                }))
            }
            _ => Err(self.error(b"expected redirection target")),
        }
    }

    fn parse_command_redirections(&mut self, command: Command) -> Result<Command, ParseError> {
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
        let condition = self.parse_program_until(|tok| matches!(tok, Token::Then), false, false)?;
        if condition.items.is_empty() {
            return Err(self.error(b"expected command list after 'if'"));
        }
        self.eat_keyword(Token::Then, b"then")?;

        fn at_elif_else_fi(tok: &Token) -> bool {
            matches!(tok, Token::Elif | Token::Else | Token::Fi)
        }
        let then_branch = self.parse_program_until(at_elif_else_fi, false, false)?;
        if then_branch.items.is_empty() {
            return Err(self.error(b"expected command list after 'then'"));
        }
        let mut elif_branches = Vec::new();

        self.set_keyword_position();
        while matches!(self.peek_token()?, Token::Elif) {
            self.advance_token();
            self.set_command_position();
            self.skip_separators_t()?;
            let cond = self.parse_program_until(|tok| matches!(tok, Token::Then), false, false)?;
            if cond.items.is_empty() {
                return Err(self.error(b"expected command list after 'elif'"));
            }
            self.eat_keyword(Token::Then, b"then")?;
            let body = self.parse_program_until(at_elif_else_fi, false, false)?;
            if body.items.is_empty() {
                return Err(self.error(b"expected command list after 'then'"));
            }
            elif_branches.push(ElifBranch {
                condition: cond,
                body,
            });
            self.set_keyword_position();
        }

        let else_branch = if matches!(self.peek_token()?, Token::Else) {
            self.advance_token();
            self.set_command_position();
            self.skip_separators_t()?;
            let body = self.parse_program_until(|tok| matches!(tok, Token::Fi), false, false)?;
            if body.items.is_empty() {
                return Err(self.error(b"expected command list after 'else'"));
            }
            Some(body)
        } else {
            None
        };

        self.eat_keyword(Token::Fi, b"fi")?;
        Ok(Command::If(IfCommand {
            condition,
            then_branch,
            elif_branches: elif_branches.into_boxed_slice(),
            else_branch,
        }))
    }

    fn parse_loop_command(&mut self, kind: LoopKind) -> Result<Command, ParseError> {
        let keyword = match kind {
            LoopKind::While => &b"while"[..],
            LoopKind::Until => &b"until"[..],
        };
        let condition = self.parse_program_until(|tok| matches!(tok, Token::Do), false, false)?;
        if condition.items.is_empty() {
            let mut msg = Vec::with_capacity(30 + keyword.len());
            msg.extend_from_slice(b"expected command list after '");
            msg.extend_from_slice(keyword);
            msg.push(b'\'');
            return Err(self.error(&msg));
        }
        self.eat_keyword(Token::Do, b"do")?;
        let body = self.parse_program_until(|tok| matches!(tok, Token::Done), false, false)?;
        if body.items.is_empty() {
            return Err(self.error(b"expected command list in do group"));
        }
        self.eat_keyword(Token::Done, b"done")?;
        Ok(Command::Loop(LoopCommand {
            kind,
            condition,
            body,
        }))
    }

    fn parse_for_command(&mut self) -> Result<Command, ParseError> {
        self.set_argument_position();
        let (name, _name_parts) = match self.peek_token()? {
            Token::Word(_, _) => self.take_word(),
            _ => return Err(self.error(b"expected for loop variable name")),
        };
        if !super::is_name(&name) {
            return Err(self.error(b"expected for loop variable name"));
        }

        self.set_keyword_position();
        self.skip_linebreaks_t()?;
        let items = if matches!(self.peek_token()?, Token::In) {
            self.advance_token();
            let mut items = Vec::new();
            self.set_argument_position();
            while matches!(self.peek_token()?, Token::Word(_, _)) {
                let word_line = self.current_line();
                let (w, wp) = self.take_word();
                items.push(Word {
                    raw: w,
                    parts: wp,
                    line: word_line,
                });
            }
            Some(items.into_boxed_slice())
        } else {
            None
        };

        self.set_keyword_position();
        self.skip_separators_t()?;
        self.eat_keyword(Token::Do, b"do")?;
        let body = self.parse_program_until(|tok| matches!(tok, Token::Done), false, false)?;
        if body.items.is_empty() {
            return Err(self.error(b"expected command list in do group"));
        }
        self.eat_keyword(Token::Done, b"done")?;
        Ok(Command::For(ForCommand { name, items, body }))
    }

    fn parse_case_command(&mut self) -> Result<Command, ParseError> {
        self.set_argument_position();
        let line = self.current_line();
        let (word_raw, word_parts) = match self.peek_token()? {
            Token::Word(_, _) => self.take_word(),
            _ => return Err(self.error(b"expected case word")),
        };
        let word = Word {
            raw: word_raw,
            parts: word_parts,
            line,
        };

        self.set_keyword_position();
        self.skip_linebreaks_t()?;
        self.eat_keyword(Token::In, b"in")?;
        self.set_keyword_position();
        self.skip_linebreaks_t()?;

        let mut arms = Vec::new();
        loop {
            self.set_keyword_position();
            if matches!(self.peek_token()?, Token::Esac | Token::Eof) {
                break;
            }

            if matches!(self.peek_token()?, Token::LParen) {
                self.advance_token();
            }

            let mut patterns = Vec::new();
            loop {
                self.set_argument_position();
                let pat_line = self.current_line();
                let (pattern_raw, pattern_parts) = if matches!(self.peek_token()?, Token::Word(_, _)) {
                    self.take_word()
                } else if let Some(name) = self.peek_token()?.keyword_name() {
                    let w: Box<[u8]> = name.to_vec().into_boxed_slice();
                    self.advance_token();
                    (w, Box::new([]) as Box<[WordPart]>)
                } else {
                    return Err(self.error(b"expected case pattern"));
                };
                patterns.push(Word {
                    raw: pattern_raw,
                    parts: pattern_parts,
                    line: pat_line,
                });

                if matches!(self.peek_token()?, Token::Pipe) {
                    self.advance_token();
                    continue;
                }
                break;
            }

            if !matches!(self.peek_token()?, Token::RParen) {
                return Err(self.error(b"expected ')' after case pattern"));
            }
            self.advance_token();
            self.set_command_position();
            self.skip_separators_t()?;

            let body = self.parse_program_until(|tok| matches!(tok, Token::Esac), false, true)?;

            self.set_keyword_position();
            let (fallthrough, has_explicit_sep) = match self.peek_token()? {
                Token::DSemi => {
                    self.advance_token();
                    (false, true)
                }
                Token::SemiAmp => {
                    self.advance_token();
                    (true, true)
                }
                _ => (false, false),
            };

            arms.push(CaseArm {
                patterns: patterns.into_boxed_slice(),
                body,
                fallthrough,
            });

            self.set_keyword_position();
            if has_explicit_sep {
                self.skip_separators_t()?;
            } else if !matches!(self.peek_token()?, Token::Esac) {
                break;
            }
        }

        self.eat_keyword(Token::Esac, b"esac")?;
        Ok(Command::Case(CaseCommand {
            word,
            arms: arms.into_boxed_slice(),
        }))
    }

    fn parse_function_keyword(&mut self) -> Result<Command, ParseError> {
        self.set_argument_position();
        let (name, _name_parts) = match self.peek_token()? {
            Token::Word(_, _) => self.take_word(),
            _ => return Err(self.error(b"expected function name")),
        };
        if !super::is_name(&name) {
            return Err(self.error(b"expected function name"));
        }
        self.set_command_position();
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
            body: Rc::new(body),
        }))
    }

    pub(super) fn next_complete_command(&mut self) -> Result<Option<Program>, ParseError> {
        self.skip_linebreaks_t()?;
        if matches!(self.peek_token()?, Token::Eof) {
            return Ok(None);
        }
        let mut items = Vec::new();
        loop {
            self.set_command_position();
            let line = self.current_line();
            let and_or = self.parse_and_or()?;
            let asynchronous = matches!(self.peek_token()?, Token::Amp);
            if asynchronous {
                self.advance_token();
            }

            let terminated = match self.peek_token()? {
                Token::Newline => {
                    self.advance_token();
                    true
                }
                Token::Semi => {
                    self.advance_token();
                    self.set_command_position();
                    matches!(self.peek_token()?, Token::Newline | Token::Eof)
                }
                _ => false,
            };

            items.push(ListItem {
                and_or,
                asynchronous,
                line,
            });

            if terminated || matches!(self.peek_token()?, Token::Eof) {
                break;
            }
        }
        Ok(Some(Program {
            items: items.into_boxed_slice(),
        }))
    }
}
