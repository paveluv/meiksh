use super::token::{token_to_keyword_name, Parser, Token};
use super::ParseError;

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

pub(super) fn split_assignment(input: &str) -> Option<(&str, &str)> {
    let (name, value) = input.split_once('=')?;
    if !super::is_name(name) {
        return None;
    }
    Some((name, value))
}

impl<'src, 'a> Parser<'src, 'a> {
    pub(super) fn eat_keyword(&mut self, expected: Token, name: &str) -> Result<(), ParseError> {
        self.set_keyword_mode(true);
        if *self.peek_token()? == expected {
            self.advance_token();
            self.skip_linebreaks_t()?;
            Ok(())
        } else {
            Err(self.error(format!("expected '{name}'")))
        }
    }

    pub(super) fn skip_separators_t(&mut self) -> Result<(), ParseError> {
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

    pub(super) fn skip_linebreaks_t(&mut self) -> Result<(), ParseError> {
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

    pub(super) fn parse_program_until(
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
                if super::is_name(&raw) && matches!(self.peek_token()?, Token::LParen) {
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
        if !super::is_name(&name) {
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
        if !super::is_name(&name) {
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

    pub(super) fn next_complete_command(
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
