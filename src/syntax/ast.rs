use std::rc::Rc;

use super::ParseError;
use super::assignment_context::{
    apply_assignment_context_to_argv_word, build_assignment_value_parts,
    find_command_decl_util_boundary, is_command_utility, is_declaration_utility,
};
use super::token::{Parser, Token};
use super::word_parts::WordPart;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Program {
    pub(crate) items: Vec<ListItem>,
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
    pub(crate) rest: Vec<(LogicalOp, Pipeline)>,
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
    pub(crate) commands: Vec<Command>,
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
    Redirected(Box<Command>, Vec<Redirection>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SimpleCommand {
    pub(crate) assignments: Vec<Assignment>,
    pub(crate) words: Vec<Word>,
    pub(crate) redirections: Vec<Redirection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Assignment {
    pub(crate) name: Vec<u8>,
    pub(crate) value: Word,
}

#[derive(Clone, Debug)]
pub(crate) struct Word {
    pub(crate) raw: Vec<u8>,
    pub(crate) parts: Vec<WordPart>,
    pub(crate) line: usize,
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
    pub(crate) name: Vec<u8>,
    pub(crate) body: Rc<Command>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IfCommand {
    pub(crate) condition: Program,
    pub(crate) then_branch: Program,
    pub(crate) elif_branches: Vec<ElifBranch>,
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
    pub(crate) name: Vec<u8>,
    pub(crate) items: Option<Vec<Word>>,
    pub(crate) body: Program,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaseCommand {
    pub(crate) word: Word,
    pub(crate) arms: Vec<CaseArm>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaseArm {
    pub(crate) patterns: Vec<Word>,
    pub(crate) body: Program,
    pub(crate) fallthrough: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct HereDoc {
    pub(crate) delimiter: Vec<u8>,
    pub(crate) body: Vec<u8>,
    /// Pre-computed `WordPart` list for `body` when `expand == true`.
    /// Populated by the parser; consumed by the expander (Phase 7) so no
    /// runtime byte-scanning of heredoc bodies is needed. Empty when
    /// `expand == false` (quoted delimiter).
    pub(crate) body_parts: Vec<crate::syntax::word_parts::WordPart>,
    pub(crate) expand: bool,
    pub(crate) strip_tabs: bool,
    pub(crate) body_line: usize,
}

impl PartialEq for HereDoc {
    fn eq(&self, other: &Self) -> bool {
        self.delimiter == other.delimiter
            && self.body == other.body
            && self.body_parts == other.body_parts
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

/// Apply parser-driven assignment-context expansion to argv words of a
/// simple command whose name is a declaration utility (or a `command`-
/// prefixed form thereof). See [`super::assignment_context`] for
/// rationale and behavior.
fn apply_declaration_utility_rewrite(words: &mut [Word]) {
    let Some(name) = words.first() else {
        return;
    };
    let rewrite_from = if is_declaration_utility(name) {
        Some(1)
    } else if is_command_utility(name) {
        find_command_decl_util_boundary(words)
    } else {
        None
    };
    if let Some(start) = rewrite_from {
        for word in &mut words[start..] {
            apply_assignment_context_to_argv_word(word);
        }
    }
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

    fn take_word(&mut self) -> (Vec<u8>, Vec<WordPart>) {
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

        Ok(Program { items })
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
        Ok(AndOr { first, rest })
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
            commands,
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
                let name: Vec<u8> = self.peek_token()?.display_name().to_vec();
                self.advance_token();
                // Keyword-as-command recovery: `display_name()` returns a
                // pure literal reserved-word byte string (`fi`, `then`,
                // `!`, `{`, ...) with no quoting, expansion, or glob
                // metacharacters, so a single `WordPart::Literal` spanning
                // the whole raw faithfully represents it. Emitting it
                // keeps every Word in the AST satisfying the invariant
                // that `parts` is non-empty when `raw` is non-empty.
                let parts: Vec<WordPart> = vec![WordPart::Literal {
                    start: 0,
                    end: name.len(),
                    has_glob: false,
                    newlines: 0,
                    assignment: false,
                }];
                self.parse_simple_command_with_first_word(name, parts, line)
                    .map(Command::Simple)
            }
        }
    }

    fn parse_simple_command_with_first_word(
        &mut self,
        first_raw: Vec<u8>,
        first_parts: Vec<WordPart>,
        first_line: usize,
    ) -> Result<SimpleCommand, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirections = Vec::new();

        if let Some((name, value_raw)) = split_assignment(&first_raw) {
            let value_parts = build_assignment_value_parts(value_raw);
            assignments.push(Assignment {
                name: name.to_vec(),
                value: Word {
                    raw: value_raw.to_vec(),
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

        apply_declaration_utility_rewrite(&mut words);

        Ok(SimpleCommand {
            assignments,
            words,
            redirections,
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

        apply_declaration_utility_rewrite(&mut words);

        Ok(SimpleCommand {
            assignments,
            words,
            redirections,
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
                    let value_parts = build_assignment_value_parts(value_raw);
                    assignments.push(Assignment {
                        name: name.to_vec(),
                        value: Word {
                            raw: value_raw.to_vec(),
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
            let body_parts = if expand {
                super::token::build_heredoc_parts(&body)
            } else {
                Vec::new()
            };
            let target_parts =
                super::token::build_word_parts_for_slice(&delimiter, 0, delimiter.len(), 0, false);
            return Ok(Some(Redirection {
                fd,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: delimiter.clone(),
                    parts: target_parts,
                    line,
                },
                here_doc: Some(HereDoc {
                    delimiter,
                    body,
                    body_parts,
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
            Ok(Command::Redirected(Box::new(command), redirections))
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
            elif_branches,
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
            Some(items)
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
                let (pattern_raw, pattern_parts) =
                    if matches!(self.peek_token()?, Token::Word(_, _)) {
                        self.take_word()
                    } else if let Some(name) = self.peek_token()?.keyword_name() {
                        // Keyword-as-pattern recovery mirrors the
                        // keyword-as-command path in `parse_command`: the
                        // reserved-word byte string is a pure literal, so a
                        // single `Literal` spanning the full raw faithfully
                        // represents it and keeps `parts` non-empty
                        // (required by the expander after the empty-parts
                        // fallback was retired).
                        let w: Vec<u8> = name.to_vec();
                        let parts: Vec<WordPart> = vec![WordPart::Literal {
                            start: 0,
                            end: w.len(),
                            has_glob: false,
                            newlines: 0,
                            assignment: false,
                        }];
                        self.advance_token();
                        (w, parts)
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
                patterns,
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
        Ok(Command::Case(CaseCommand { word, arms }))
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
        Ok(Some(Program { items }))
    }
}

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
mod tests {
    use super::*;

    use std::rc::Rc;

    use crate::hash::ShellMap;
    use crate::syntax::byte_class::alias_has_trailing_blank;
    use crate::syntax::{parse, parse_with_aliases};

    fn bx(s: &[u8]) -> Vec<u8> {
        s.to_vec()
    }

    fn alias_map(pairs: &[(&[u8], &[u8])]) -> ShellMap<Box<[u8]>, Box<[u8]>> {
        pairs
            .iter()
            .map(|(k, v)| (Box::from(*k), Box::from(*v)))
            .collect()
    }

    fn parse_test(source: &str) -> Result<Program, super::super::ParseError> {
        parse(source.as_bytes())
    }

    fn parse_with_aliases_test(
        source: &str,
        aliases: &ShellMap<Box<[u8]>, Box<[u8]>>,
    ) -> Result<Program, super::super::ParseError> {
        parse_with_aliases(source.as_bytes(), aliases)
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
            Command::Simple(cmd) if cmd.assignments.len() == 1 && &*cmd.words[0].raw == b"echo"
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
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == b"'ok'"
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
                    && &*cmd.redirections[0].target.raw == b"EOF"
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some(&b"hello $USER\n"[..])
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse_test("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.delimiter) == Some(&b"EOF"[..])
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse_test("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some(&b"one\n"[..])
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.strip_tabs) == Some(true)
        ));
    }

    #[test]
    fn parses_extended_redirection_forms() {
        let program = parse_test("cat 3<in 2>out 4>>log 5<>rw 0<&3 1>&2 2>|force").expect("parse");
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
                    && &*redirections[0].target.raw == b"out"
        ));

        let not_a_group = parse_test("{echo hi; }").expect("parse brace word");
        assert!(matches!(
            &not_a_group.items[0].and_or.first.commands[0],
            Command::Simple(simple) if &*simple.words[0].raw == b"{echo"
        ));

        let closer_literal = parse_test("echo }").expect("parse literal closer");
        assert!(matches!(
            &closer_literal.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"echo"[..], &b"}"[..]]
        ));
    }

    #[test]
    fn parses_function_definition() {
        let program = parse_test("greet() { echo hi; }").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(function) if &*function.name == b"greet"
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
        assert_eq!(&*error.message, b"expected command");

        let error = parse_test("echo hi | ! cat").expect_err("bang after pipe should fail");
        assert_eq!(&*error.message, b"expected command");
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = parse_test("echo 'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, b"unterminated single quote");
    }

    #[test]
    fn rejects_unterminated_dollar_single_quote() {
        let error = parse_test("echo $'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, b"unterminated dollar-single-quotes");
        let error = parse_test(r"echo $'backslash at end\").expect_err("parse should fail");
        assert_eq!(&*error.message, b"unterminated dollar-single-quotes");
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
                if &*for_command.name == b"item" && for_command.items.as_ref().map(|s| s.len()) == Some(3)
        ));

        let positional = parse_test("for item; do echo $item; done").expect("parse");
        assert!(matches!(
            &positional.items[0].and_or.first.commands[0],
            Command::For(for_command) if &*for_command.name == b"item" && for_command.items.is_none()
        ));

        let linebreak_before_in =
            parse_test("for item\nin a b; do echo $item; done").expect("parse linebreak before in");
        assert!(matches!(
            &linebreak_before_in.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if &*for_command.name == b"item"
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                        == Some(vec![&b"a"[..], &b"b"[..]])
        ));

        let reserved_words_as_items = parse_test("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                    == Some(vec![&b"do"[..], &b"done"[..]])
        ));
    }

    #[test]
    fn parses_case_commands() {
        let program =
            parse_test("case $name in foo|bar) echo hit ;; baz) echo miss ;; esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_command)
                if &*case_command.word.raw == b"$name"
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
            &*super::super::ParseError {
                message: Box::from(&b"x"[..]),
                line: None,
            }
            .message,
            b"x"
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
    fn alias_expansion_in_simple_commands() {
        let aliases = alias_map(&[(b"say", b"printf hi")]);
        let program = parse_with_aliases_test("say", &aliases).expect("parse alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"printf"[..], &b"hi"[..]]
        ));

        let aliases = alias_map(&[(b"cond", b"if")]);
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
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"echo"[..], &b"!"[..]]
        ));

        let program = parse_test("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"!true"[..]]
        ));

        let program = parse_test("! true").expect("parse negation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn trailing_blank_aliases_expand_next_simple_command_word() {
        let aliases = alias_map(&[(b"say", b"printf %s "), (b"word", b"ok")]);
        let program = parse_with_aliases_test("say word", &aliases).expect("parse chained alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"printf"[..], &b"%s"[..], &b"ok"[..]]
        ));
    }

    #[test]
    fn self_referential_aliases_do_not_loop_indefinitely() {
        let aliases = alias_map(&[(b"loop", b"loop ")]);
        let program = parse_with_aliases_test("loop ok", &aliases).expect("self alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"loop"[..], &b"ok"[..]]
        ));
        assert!(alias_has_trailing_blank(b"value "));
        assert!(!alias_has_trailing_blank(b"value"));
    }

    #[test]
    fn alias_expansion_after_assignment_and_redirection() {
        let aliases = alias_map(&[(b"foo", b"echo aliased")]);
        let program =
            parse_with_aliases_test("VAR=value foo", &aliases).expect("alias after assignment");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec![&b"echo"[..], &b"aliased"[..]]
                    && simple.assignments.len() == 1
        ));

        let program =
            parse_with_aliases_test("</dev/null foo", &aliases).expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec![&b"echo"[..], &b"aliased"[..]]
                    && simple.redirections.len() == 1
        ));
    }

    #[test]
    fn lparen_after_simple_command_is_syntax_error() {
        let aliases = alias_map(&[(b"foo", b"echo aliased")]);
        let err = parse_with_aliases_test("foo () { true; }", &aliases).unwrap_err();
        assert!(
            err.message.iter().any(|&b| b == b'('),
            "error should mention '(': {:?}",
            err.message
        );
    }

    #[test]
    fn tokenizer_keeps_dollar_paren_as_single_word() {
        let program = parse_test("echo $(cmd arg)").expect("parse dollar paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == b"$(cmd arg)"
        ));
    }

    #[test]
    fn tokenizer_keeps_dollar_double_paren_as_single_word() {
        let program = parse_test("echo $((1 + 2))").expect("parse dollar arith");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == b"$((1 + 2))"
        ));

        let nested = parse_test("echo $((1 + (2 * 3)))").expect("parse nested arith");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if &*cmd.words[1].raw == b"$((1 + (2 * 3)))"
        ));

        let error = parse_test("echo $((1 + 2").expect_err("unterminated arith");
        assert_eq!(&*error.message, b"unterminated arithmetic expansion");
    }

    #[test]
    fn tokenizer_keeps_dollar_brace_as_single_word() {
        let program = parse_test("echo ${VAR:-default}").expect("parse dollar brace");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == b"${VAR:-default}"
        ));

        let nested = parse_test("echo ${VAR:-${INNER}}").expect("parse nested brace");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"${VAR:-${INNER}}"
        ));
    }

    #[test]
    fn tokenizer_keeps_backtick_as_single_word() {
        let program = parse_test("echo `cmd arg`").expect("parse backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == b"`cmd arg`"
        ));

        let error = parse_test("echo `unterminated").expect_err("unterminated backtick");
        assert_eq!(&*error.message, b"unterminated backquote");
    }

    #[test]
    fn tokenizer_nested_constructs_in_brace_body() {
        let program = parse_test("echo ${VAR:-'a}b'}").expect("parse brace sq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"${VAR:-'a}b'}"
        ));

        let program = parse_test("echo ${VAR:-\"a}b\"}").expect("parse brace dq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"${VAR:-\"a}b\"}"
        ));

        let program = parse_test("echo ${VAR:-\\}}").expect("parse brace escaped");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"${VAR:-\\}}"
        ));

        let error = parse_test("echo ${VAR:-unclosed").expect_err("unterminated brace body");
        assert_eq!(&*error.message, b"unterminated parameter expansion");

        let error = parse_test("echo $(unclosed").expect_err("unterminated paren body");
        assert_eq!(&*error.message, b"unterminated command substitution");
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
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == b"hello"
        ));
    }

    #[test]
    fn if_empty_condition_is_parse_error() {
        let error = parse_test("if then fi").expect_err("empty if condition");
        assert!(
            error
                .message
                .windows(b"expected command list after 'if'".len())
                .any(|w| w == b"expected command list after 'if'")
        );
    }

    #[test]
    fn elif_empty_condition_is_parse_error() {
        let error =
            parse_test("if true; then true; elif then true; fi").expect_err("empty elif condition");
        assert!(
            error
                .message
                .windows(b"expected command list after 'elif'".len())
                .any(|w| w == b"expected command list after 'elif'")
        );
    }

    #[test]
    fn while_empty_condition_is_parse_error() {
        let error = parse_test("while do true; done").expect_err("empty while condition");
        assert!(
            error
                .message
                .windows(b"expected command list after 'while'".len())
                .any(|w| w == b"expected command list after 'while'")
        );
    }

    #[test]
    fn until_empty_condition_is_parse_error() {
        let error = parse_test("until do true; done").expect_err("empty until condition");
        assert!(
            error
                .message
                .windows(b"expected command list after 'until'".len())
                .any(|w| w == b"expected command list after 'until'")
        );
    }

    #[test]
    fn time_default_pipeline() {
        let program = parse_test("time echo hello").expect("parse time default");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Default);
        assert!(!pipeline.negated);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == b"echo")
        );
    }

    #[test]
    fn time_posix_pipeline() {
        let program = parse_test("time -p echo hello").expect("parse time -p");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Posix);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == b"echo")
        );
    }

    #[test]
    fn function_keyword_basic() {
        let program = parse_test("function foo { echo hi; }").expect("parse function keyword");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == b"foo"
        ));
    }

    #[test]
    fn function_keyword_with_parens() {
        let program =
            parse_test("function foo() { echo hi; }").expect("parse function keyword parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == b"foo"
        ));
    }

    #[test]
    fn function_keyword_invalid_name() {
        let error = parse_test("function 123").expect_err("bad function name");
        assert_eq!(&*error.message, b"expected function name");
    }

    #[test]
    fn clone_covers_all_command_variants() {
        let simple = Command::Simple(SimpleCommand {
            assignments: vec![Assignment {
                name: bx(b"X"),
                value: Word {
                    raw: bx(b"1"),
                    parts: Vec::new(),
                    line: 0,
                },
            }],
            words: vec![Word {
                raw: bx(b"echo"),
                parts: Vec::new(),
                line: 0,
            }],
            redirections: vec![Redirection {
                fd: Some(2),
                kind: RedirectionKind::Write,
                target: Word {
                    raw: bx(b"err"),
                    parts: Vec::new(),
                    line: 0,
                },
                here_doc: None,
            }],
        });
        let s = simple.clone();
        assert!(matches!(&s, Command::Simple(sc) if &*sc.words[0].raw == b"echo"));

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
                line: 0,
            }],
        });
        assert!(matches!(subshell.clone(), Command::Subshell(_)));

        let group = Command::Group(Program { items: vec![] });
        assert!(matches!(group.clone(), Command::Group(_)));

        let func = Command::FunctionDef(FunctionDef {
            name: bx(b"f"),
            body: Rc::new(s.clone()),
        });
        assert!(matches!(&func, Command::FunctionDef(fd) if &*fd.name == b"f"));

        let if_cmd = Command::If(IfCommand {
            condition: Program { items: vec![] },
            then_branch: Program { items: vec![] },
            elif_branches: vec![ElifBranch {
                condition: Program { items: vec![] },
                body: Program { items: vec![] },
            }],
            else_branch: Some(Program { items: vec![] }),
        });
        assert!(matches!(if_cmd, Command::If(_)));

        let loop_cmd = Command::Loop(LoopCommand {
            kind: LoopKind::While,
            condition: Program { items: vec![] },
            body: Program { items: vec![] },
        });
        assert!(matches!(loop_cmd, Command::Loop(_)));

        let for_cmd = Command::For(ForCommand {
            name: bx(b"i"),
            items: Some(vec![Word {
                raw: bx(b"a"),
                parts: Vec::new(),
                line: 0,
            }]),
            body: Program { items: vec![] },
        });
        assert!(matches!(&for_cmd, Command::For(fc) if &*fc.name == b"i"));

        let case_cmd = Command::Case(CaseCommand {
            word: Word {
                raw: bx(b"x"),
                parts: Vec::new(),
                line: 0,
            },
            arms: vec![CaseArm {
                patterns: vec![Word {
                    raw: bx(b"*"),
                    parts: Vec::new(),
                    line: 0,
                }],
                body: Program { items: vec![] },
                fallthrough: false,
            }],
        });
        assert!(matches!(case_cmd, Command::Case(_)));

        let redir = Command::Redirected(
            Box::new(s.clone()),
            vec![Redirection {
                fd: None,
                kind: RedirectionKind::Write,
                target: Word {
                    raw: bx(b"out"),
                    parts: Vec::new(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: bx(b"EOF"),
                    body: bx(b"test\n"),
                    body_parts: Vec::new(),
                    expand: true,
                    strip_tabs: false,
                    body_line: 0,
                }),
            }],
        );
        assert!(matches!(redir, Command::Redirected(_, _)));
    }

    #[test]
    fn alias_expansion_produces_non_word_tokens() {
        let aliases = alias_map(&[(b"both", b"echo a; echo b")]);
        let program =
            parse_with_aliases_test("both", &aliases).expect("parse alias with semicolon");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn alias_expansion_interns_reserved_word_tokens() {
        let aliases = alias_map(&[(
            b"myif",
            b"if true; then echo ok; elif false; then echo no; else echo fb; fi",
        )]);
        let program =
            parse_with_aliases_test("myif", &aliases).expect("alias if/then/elif/else/fi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));

        let aliases = alias_map(&[(b"mywhile", b"while false; do echo loop; done")]);
        let program = parse_with_aliases_test("mywhile", &aliases).expect("alias while/do/done");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let aliases = alias_map(&[(b"myuntil", b"until true; do echo u; done")]);
        let program = parse_with_aliases_test("myuntil", &aliases).expect("alias until");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let aliases = alias_map(&[(b"myfor", b"for x in a b; do echo $x; done")]);
        let program = parse_with_aliases_test("myfor", &aliases).expect("alias for/in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));

        let aliases = alias_map(&[(b"mycase", b"case x in a) echo a;; esac")]);
        let program = parse_with_aliases_test("mycase", &aliases).expect("alias case/esac");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));

        let aliases = alias_map(&[(b"myfn", b"function myfunc { echo hi; }")]);
        let program = parse_with_aliases_test("myfn", &aliases).expect("alias function/{/}");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(_)
        ));

        let aliases = alias_map(&[(b"myneg", b"! true")]);
        let program = parse_with_aliases_test("myneg", &aliases).expect("alias bang");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn alias_not_expanded_in_reserved_word_position() {
        let aliases = alias_map(&[(b"bla", b"in")]);
        let result = parse_with_aliases_test("for i bla a b c; do echo $i; done", &aliases);
        assert!(
            result.is_err(),
            "alias should not expand to 'in' in reserved word position"
        );
    }

    #[test]
    fn alias_shadows_keyword_at_command_position() {
        let aliases = alias_map(&[(b"if", b"hello")]);
        let program = parse_with_aliases_test("if", &aliases).expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(s) if s.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec![&b"hello"[..]]
        ));
    }

    #[test]
    fn backslash_newline_mid_word_produces_stripped_form() {
        let program = parse_test("ec\\\nho ok").expect("continuation in command");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == b"echo" && &*cmd.words[1].raw == b"ok"
        ));

        let program = parse_test("echo a\\\nb\\\nc").expect("multiple continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"abc"
        ));
    }

    #[test]
    fn backslash_newline_before_comment_does_not_start_comment() {
        let program = parse_test("a\\\n#b").expect("continuation before hash");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == b"a#b"
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
            Command::Simple(cmd) if &*cmd.words[1].raw == b"\"ab\\\ncd\""
        ));
    }

    #[test]
    fn backslash_newline_inside_single_quotes_preserved() {
        let program = parse_test("echo 'ab\\\ncd'").expect("squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_backticks_preserved() {
        let program = parse_test("echo `ab\\\ncd`").expect("backtick continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"`ab\\\ncd`"
        ));
    }

    #[test]
    fn backslash_newline_inside_dollar_single_quote_preserved() {
        let program = parse_test("echo $'ab\\\ncd'").expect("dollar-squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"$'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_command_substitution_preserved() {
        let program = parse_test("echo $(ab\\\ncd)").expect("cmdsub continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"$(ab\\\ncd)"
        ));
    }

    #[test]
    fn backslash_newline_mixed_unquoted_and_dquoted() {
        let program = parse_test("echo hel\\\nlo\"wor\\\nld\"").expect("mixed continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == b"hello\"wor\\\nld\""
        ));
    }

    #[test]
    fn arithmetic_expansion_with_quoting() {
        let program = parse_test("echo $(( 1 + 2 ))").expect("basic arithmetic");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program = parse_test("echo $(( \")\" ))").expect("arithmetic with quoted paren");
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
        assert!(
            err.message
                .windows(b"unterminated".len())
                .any(|w| w == b"unterminated")
        );
    }

    #[test]
    fn io_number_recognised_inside_alias() {
        let aliases: ShellMap<Box<[u8]>, Box<[u8]>> =
            alias_map(&[(b"redir", b"echo hello 2>/dev/null")]);
        let program = parse_with_aliases_test("redir", &aliases).expect("alias with IO number");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if {
                let has_echo = cmd.words.iter().any(|w| &*w.raw == b"echo");
                let has_redir_fd2 = cmd.redirections.iter().any(|r|
                    r.fd == Some(2) && r.kind == RedirectionKind::Write
                );

                let no_word_2 = !cmd.words.iter().any(|w| &*w.raw == b"2");
                has_echo && has_redir_fd2 && no_word_2
            }
        ));
    }

    #[test]
    fn comment_with_close_paren_inside_command_substitution() {
        let program =
            parse_test("echo $(echo hello # )\necho world\n)").expect("comment with ) in $(...)");

        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn backslash_newline_continuation_in_alias() {
        let aliases: ShellMap<Box<[u8]>, Box<[u8]>> = alias_map(&[(b"foo", b"echo hell\\\no")]);
        let program = parse_with_aliases_test("foo", &aliases).expect("alias with continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == [&b"echo"[..], &b"hello"[..]]
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
        assert_eq!(&*doc.body, b"EO\\\nF\nreal body\n");
        assert!(!doc.expand);
    }

    #[test]
    fn backslash_newline_before_comment_in_command_substitution() {
        let program = parse_test("echo $(echo foo \\\n# comment with )\necho bar)\n")
            .expect("continuation before comment in $(...)");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn heredoc_body_inside_alias_expansion() {
        let aliases: ShellMap<Box<[u8]>, Box<[u8]>> =
            alias_map(&[(b"x", b"cat <<EOF\nhello\nEOF")]);
        let program = parse_with_aliases_test("x\n", &aliases).expect("heredoc inside alias");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 1, "word count");
        assert_eq!(&*cmd.words[0].raw, b"cat");
        assert_eq!(cmd.redirections.len(), 1, "redirection count");
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0]
            .here_doc
            .as_ref()
            .expect("heredoc body should be present");
        assert_eq!(&*doc.body, b"hello\n");
        assert_eq!(&*doc.delimiter, b"EOF");
        assert!(doc.expand);
    }

    #[test]
    fn continuation_splits_keyword_if() {
        let program =
            parse_test("i\\\nf true; then echo ha; fi\n").expect("if split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_then() {
        let program =
            parse_test("if true; th\\\nen echo ha; fi\n").expect("then split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.then_branch.items.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_while() {
        let program =
            parse_test("wh\\\nile false; do :; done\n").expect("while split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_do() {
        let program =
            parse_test("while false; d\\\no :; done\n").expect("do split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_done() {
        let program =
            parse_test("while false; do :; do\\\nne\n").expect("done split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_for() {
        let program =
            parse_test("fo\\\nr i in a; do echo $i; done\n").expect("for split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_case() {
        let program =
            parse_test("cas\\\ne x in x) echo y;; esac\n").expect("case split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_alias_name() {
        let aliases: ShellMap<Box<[u8]>, Box<[u8]>> = alias_map(&[(b"foo", b"echo aliased")]);
        let program =
            parse_with_aliases_test("fo\\\no\n", &aliases).expect("alias split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[0].raw, b"echo");
        assert_eq!(&*cmd.words[1].raw, b"aliased");
    }

    #[test]
    fn continuation_in_word() {
        let program = parse_test("echo he\\\nllo\n").expect("word continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[1].raw, b"hello");
    }

    #[test]
    fn continuation_splits_double_semicolon() {
        let program =
            parse_test("case x in x) echo y;\\\n;esac\n").expect(";; split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(cmd) => cmd,
            other => panic!("expected Case, got {other:?}"),
        };
        assert_eq!(cmd.arms.len(), 1);
        assert!(!cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_and_if() {
        let program = parse_test("true &\\\n& echo ok\n").expect("&& split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::And);
    }

    #[test]
    fn continuation_splits_or_if() {
        let program = parse_test("false |\\\n| echo ok\n").expect("|| split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::Or);
    }

    #[test]
    fn continuation_splits_heredoc_operator() {
        let program = parse_test("cat <\\\n<EOF\nhello\nEOF\n").expect("<< split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections.len(), 1);
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.body, b"hello\n");
    }

    #[test]
    fn continuation_splits_append_operator() {
        let program = parse_test("echo hi >\\\n> /dev/null\n").expect(">> split by continuation");
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
        let program = parse_test("\\\nif true; then echo ha; fi\n").expect("continuation at start");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_esac() {
        let program =
            parse_test("case x in x) echo y;; es\\\nac\n").expect("esac split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_elif() {
        let program = parse_test("if false; then :; el\\\nif true; then echo ok; fi\n")
            .expect("elif split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.elif_branches.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_else() {
        let program = parse_test("if false; then :; el\\\nse echo ok; fi\n")
            .expect("else split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert!(cmd.else_branch.is_some());
    }

    #[test]
    fn continuation_splits_keyword_fi() {
        let program =
            parse_test("if true; then echo ok; f\\\ni\n").expect("fi split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_in() {
        let program =
            parse_test("for i i\\\nn a b; do echo $i; done\n").expect("in split by continuation");
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
        let program = parse_test("cat <\\\n&0 < /dev/null\n").expect("<& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupInput)
        );
    }

    #[test]
    fn continuation_splits_dup_output() {
        let program = parse_test("echo hi >\\\n&2\n").expect(">& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupOutput)
        );
    }

    #[test]
    fn continuation_splits_read_write() {
        let program = parse_test("echo ok <\\\n> /dev/null\n").expect("<> split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ReadWrite)
        );
    }

    #[test]
    fn continuation_splits_clobber_write() {
        let program = parse_test("echo ok >\\\n| /dev/null\n").expect(">| split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ClobberWrite)
        );
    }

    #[test]
    fn continuation_splits_heredoc_strip_tabs() {
        let program =
            parse_test("cat <\\\n<-EOF\n\thello\n\tEOF\n").expect("<<- split by continuation");
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
        let program = parse_test("case x in x) echo first;\\\n& y) echo second;; esac\n")
            .expect(";& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected Case, got {other:?}"),
        };
        assert!(cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_bang_negation() {
        let program = parse_test("!\\\n true\n").expect("! with continuation");
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
        assert_eq!(&*doc.delimiter, b"EOF");
        assert_eq!(&*doc.body, b"hello\n");
    }

    #[test]
    fn continuation_splits_assignment() {
        let program =
            parse_test("x\\\n=hello echo $x\n").expect("assignment split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.assignments.len(), 1);
        assert_eq!(&*cmd.assignments[0].name, b"x");
    }

    #[test]
    fn continuation_splits_io_number() {
        let program =
            parse_test("echo ok 2\\\n>/dev/null\n").expect("IO number split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.fd == Some(2)));
    }

    #[test]
    fn continuation_inside_double_quotes() {
        let program =
            parse_test("echo \"he\\\nllo\"\n").expect("continuation inside double quotes");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_inside_backticks() {
        let program = parse_test("echo `echo he\\\nllo`\n").expect("continuation inside backticks");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_arith_close() {
        let program = parse_test("echo $((1+2)\\\n)\n").expect("arith close split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with(b"$(("));
    }

    #[test]
    fn continuation_splits_dollar_paren() {
        let program = parse_test("echo $\\\n(echo inner)\n").expect("$( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_dollar_brace() {
        let program = parse_test("x=hello; echo $\\\n{x}\n").expect("${ split by continuation");
        let cmd = match &program.items[1].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert_eq!(&*cmd.words[1].raw, b"${x}");
    }

    #[test]
    fn continuation_splits_dollar_double_paren() {
        let program = parse_test("echo $(\\\n(1+2))\n").expect("$(( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with(b"$(("));
    }

    #[test]
    fn continuation_splits_dollar_single_quote() {
        let program = parse_test("echo $\\\n'hello'\n").expect("$' split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn arithmetic_unmatched_close_paren() {
        let program = parse_test("echo $(( 1 ) + 2 ))").expect("arithmetic with unmatched )");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn heredoc_with_operators_on_same_line() {
        let program =
            parse_test("cat <<EOF | wc -l\nhello\nEOF\n").expect("heredoc with pipe on line");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.commands.len(), 2);
    }

    #[test]
    fn heredoc_with_redirect_on_same_line() {
        let program =
            parse_test("cat <<EOF >out\nhello\nEOF\n").expect("heredoc with redirect on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
    }

    #[test]
    fn heredoc_with_append_redirect_on_same_line() {
        let program =
            parse_test("cat <<EOF >>out\nhello\nEOF\n").expect("heredoc with append on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::Append)
        );
    }

    #[test]
    fn heredoc_with_fd_dup_on_same_line() {
        let program = parse_test("cat <<EOF 2>&1\nhello\nEOF\n").expect("heredoc with dup on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupOutput)
        );
    }

    #[test]
    fn heredoc_with_clobber_on_same_line() {
        let program =
            parse_test("cat <<EOF >|out\nhello\nEOF\n").expect("heredoc with clobber on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ClobberWrite)
        );
    }

    #[test]
    fn heredoc_with_and_on_same_line() {
        let program =
            parse_test("cat <<EOF && echo ok\nhello\nEOF\n").expect("heredoc with && on line");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::And);
    }

    #[test]
    fn heredoc_with_or_on_same_line() {
        let program =
            parse_test("cat <<EOF || echo fail\nhello\nEOF\n").expect("heredoc with || on line");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::Or);
    }

    #[test]
    fn heredoc_with_semicolon_on_same_line() {
        let program =
            parse_test("cat <<EOF ; echo after\nhello\nEOF\n").expect("heredoc with ; on line");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn heredoc_with_background_on_same_line() {
        let program =
            parse_test("cat <<EOF & echo after\nhello\nEOF\n").expect("heredoc with & on line");
        assert!(program.items[0].asynchronous);
    }

    #[test]
    fn heredoc_with_word_on_same_line() {
        let program = parse_test("cmd <<EOF arg\nhello\nEOF\n").expect("heredoc with word on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[1].raw, b"arg");
    }

    #[test]
    fn heredoc_with_io_number_on_same_line() {
        let program =
            parse_test("cmd <<EOF 2>err\nhello\nEOF\n").expect("heredoc with io number on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.fd == Some(2)));
    }

    #[test]
    fn multiple_heredocs_on_same_line() {
        let program = parse_test("cat <<A <<B\nbody1\nA\nbody2\nB\n")
            .expect("multiple heredocs on same line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
    }

    #[test]
    fn heredoc_with_less_and_on_same_line() {
        let program = parse_test("cat <<EOF <&3\nhello\nEOF\n").expect("heredoc with <& on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupInput)
        );
    }

    #[test]
    fn heredoc_with_less_great_on_same_line() {
        let program =
            parse_test("cat <<EOF <>file\nhello\nEOF\n").expect("heredoc with <> on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ReadWrite)
        );
    }

    #[test]
    fn heredoc_with_less_on_same_line() {
        let program = parse_test("cat <<EOF <input\nhello\nEOF\n").expect("heredoc with < on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::Read)
        );
    }

    #[test]
    fn heredoc_with_dsemi_on_same_line() {
        let program =
            parse_test("case x in\nx) cat <<EOF ;;\nhello\nEOF\nesac\n").expect("heredoc dsemi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn heredoc_with_strip_tabs_on_subsequent() {
        let program = parse_test("cat <<A <<-B\nbody1\nA\n\tbody2\nB\n")
            .expect("heredoc with <<- on same line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
        assert!(
            cmd.redirections[1]
                .here_doc
                .as_ref()
                .map_or(false, |h| h.strip_tabs)
        );
    }

    #[test]
    fn heredoc_continuation_in_body() {
        let program = parse_test("cat <<EOF\nline\\\ncontinued\nEOF\n")
            .expect("heredoc continuation in body");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        let body = &cmd.redirections[0].here_doc.as_ref().unwrap().body;
        assert_eq!(&**body, b"linecontinued\n");
    }

    #[test]
    fn heredoc_escaped_backslash_at_eol_is_not_continuation() {
        let program = parse_test("cat <<EOF\n\\$val\n\\\\\nEOF\n")
            .expect("escaped backslash at end of line should not be continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        let body = &cmd.redirections[0].here_doc.as_ref().unwrap().body;
        assert_eq!(&**body, b"\\$val\n\\\\\n");
    }

    #[test]
    fn reclassify_queued_alias_on_heredoc_line() {
        let aliases = alias_map(&[(b"myalias", b"echo hello")]);
        let program = parse_with_aliases_test("cat <<EOF | myalias\nbody\nEOF\n", &aliases)
            .expect("alias on heredoc line");
        let pipeline = &program.items[0].and_or.first;
        assert!(pipeline.commands.len() >= 2);
    }

    #[test]
    fn single_quote_in_alias_expansion() {
        let aliases = alias_map(&[(b"sq", b"echo 'hello world'")]);
        let program = parse_with_aliases_test("sq", &aliases).expect("alias with single quote");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.words
                .iter()
                .any(|w| w.raw.windows(b"hello".len()).any(|win| win == b"hello"))
        );
    }

    #[test]
    fn consume_paren_body_with_nested_parens() {
        let program = parse_test("echo $( (echo inner) )").expect("nested parens in $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.words[1]
                .raw
                .windows(b"inner".len())
                .any(|w| w == b"inner")
        );
    }

    #[test]
    fn consume_paren_body_with_comment() {
        let program =
            parse_test("echo $(# comment\necho ok)").expect("comment inside command subst");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.windows(b"ok".len()).any(|w| w == b"ok"));
    }

    #[test]
    fn consume_paren_body_with_backslash() {
        let program =
            parse_test("echo $(printf '%s\\n' hello)").expect("backslash inside command subst");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.windows(b"\\n".len()).any(|w| w == b"\\n"));
    }

    #[test]
    fn consume_quoted_element_backtick() {
        let program = parse_test("echo \"`echo inner`\"").expect("backtick in double quote");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.words[1]
                .raw
                .windows(b"`echo inner`".len())
                .any(|w| w == b"`echo inner`")
        );
    }

    #[test]
    fn dollar_construct_in_double_quotes() {
        let program = parse_test("echo \"$(echo inner)\"").expect("dollar-paren in double quotes");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.words[1]
                .raw
                .windows(b"$(echo inner)".len())
                .any(|w| w == b"$(echo inner)")
        );
    }

    #[test]
    fn scan_raw_word_trailing_backslash_eof() {
        let program = parse_test("echo \\").expect("trailing backslash at eof");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words.iter().any(|w| &*w.raw == b"\\"));
    }

    #[test]
    fn scan_raw_word_continuation_at_word_start_produces_word() {
        let program = parse_test("\\\necho hello").expect("continuation at start then word");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[0].raw, b"echo");
    }

    #[test]
    fn error_expected_close_brace() {
        let result = parse_test("{ echo ok");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains(&b'}'));
    }

    #[test]
    fn error_expected_close_paren_subshell() {
        let result = parse_test("(echo ok");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains(&b')'));
    }

    #[test]
    fn error_unexpected_lparen_after_word() {
        let result = parse_test("echo hello(");
        assert!(result.is_err());
    }

    #[test]
    fn function_def_via_word_lparen_rparen() {
        let program = parse_test("myfn() { echo hi; }").expect("function def");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == b"myfn"
        ));
    }

    #[test]
    fn command_starts_with_redirect() {
        let program = parse_test(">out echo hello").expect("redirect before command");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(!cmd.redirections.is_empty());
        assert!(!cmd.words.is_empty());
    }

    #[test]
    fn case_with_semi_amp_fallthrough() {
        let program =
            parse_test("case x in\nx) echo ok ;&\ny) echo y ;;\nesac\n").expect("case fallthrough");
        let case = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected case, got {other:?}"),
        };
        assert!(case.arms[0].fallthrough);
    }

    #[test]
    fn case_with_plain_semi_before_esac() {
        let result = parse_test("case x in\nx) echo ok ;\nesac\n");
        assert!(
            result.is_err()
                || matches!(
                    result.as_ref().unwrap().items[0].and_or.first.commands[0],
                    Command::Case(_)
                )
        );
    }

    #[test]
    fn for_loop_without_in_clause() {
        let program = parse_test("for x do echo $x; done").expect("for loop without in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(f) if f.items.is_none()
        ));
    }

    #[test]
    fn for_loop_with_non_word_after_in() {
        let program = parse_test("for x in ; do echo $x; done").expect("for in with empty list");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(f) if f.items.as_ref().map_or(false, |i| i.is_empty())
        ));
    }

    #[test]
    fn function_keyword_def() {
        let program = parse_test("function myfn { echo hi; }").expect("function keyword def");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == b"myfn"
        ));
    }

    #[test]
    fn function_keyword_def_with_optional_parens() {
        let program =
            parse_test("function myfn() { echo hi; }").expect("function keyword with parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == b"myfn"
        ));
    }

    #[test]
    fn heredoc_with_paren_on_same_line() {
        let program =
            parse_test("(cat <<EOF\nhello\nEOF\n)").expect("heredoc with paren on same line");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Subshell(_)
        ));
    }

    #[test]
    fn heredoc_with_semi_amp_on_same_line() {
        let program = parse_test("case x in\nx) cat <<EOF ;&\nhello\nEOF\ny) echo y ;;\nesac\n")
            .expect("heredoc semiamp on same line");
        let case = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected case, got {other:?}"),
        };
        assert!(case.arms[0].fallthrough);
    }

    #[test]
    fn heredoc_with_rparen_on_same_line() {
        let program =
            parse_test("(cat <<EOF )\nhello\nEOF\n").expect("heredoc with rparen on same line");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Subshell(_)
        ));
    }

    #[test]
    fn heredoc_with_lparen_on_same_line() {
        let program = parse_test("cat <<EOF ; (echo sub)\nhello\nEOF\n")
            .expect("heredoc with lparen on same line");
        assert!(
            program.items.len() >= 2 || {
                let pipeline = &program.items[0].and_or.first;
                pipeline.commands.len() >= 1
            }
        );
    }

    #[test]
    fn heredoc_empty_delimiter_error() {
        let result = parse_test("cat << \nhello\n");
        assert!(result.is_err());
    }

    #[test]
    fn reclassify_queued_keyword_on_heredoc_line() {
        let program = parse_test("cat <<EOF ; if true; then echo yes; fi\nbody\nEOF\n")
            .expect("keyword on heredoc line");
        assert!(program.items.len() >= 2);
    }

    #[test]
    fn reclassify_queued_alias_with_trailing_blank() {
        let aliases = alias_map(&[(b"ll", b"ls -l ")]);
        let program = parse_with_aliases_test("cat <<EOF | ll foo\nbody\nEOF\n", &aliases)
            .expect("alias with trailing blank on heredoc line");
        let pipeline = &program.items[0].and_or.first;
        assert!(pipeline.commands.len() >= 2);
    }

    #[test]
    fn consume_paren_body_backslash_non_newline() {
        let program =
            parse_test("echo $(printf '\\t')").expect("backslash non-newline in paren body");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.windows(b"\\t".len()).any(|w| w == b"\\t"));
    }

    #[test]
    fn error_case_expected_word() {
        let result = parse_test("case ; in esac");
        assert!(result.is_err());
    }

    #[test]
    fn error_for_expected_variable_name() {
        let result = parse_test("for ; do echo hi; done");
        assert!(result.is_err());
    }

    #[test]
    fn error_case_pattern_missing_rparen() {
        let result = parse_test("case x in\nx echo ;;\nesac\n");
        assert!(result.is_err());
    }

    #[test]
    fn error_function_keyword_expected_name() {
        let result = parse_test("function ; { echo hi; }");
        assert!(result.is_err());
    }

    #[test]
    fn error_function_keyword_invalid_name() {
        let result = parse_test("function 123 { echo hi; }");
        assert!(result.is_err());
    }

    #[test]
    fn split_assignment_non_name_returns_none() {
        assert!(split_assignment(b"123=val").is_none());
    }

    #[test]
    fn list_item_partial_eq() {
        let p1 = parse_test("echo a").unwrap();
        let p2 = parse_test("echo a").unwrap();
        assert_eq!(p1.items[0], p2.items[0]);
    }

    #[test]
    fn word_partial_eq() {
        let p1 = parse_test("echo hello").unwrap();
        let p2 = parse_test("echo hello").unwrap();
        let cmd1 = match &p1.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        };
        let cmd2 = match &p2.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        };
        assert_eq!(cmd1.words[0], cmd2.words[0]);
    }

    #[test]
    fn heredoc_partial_eq() {
        let p1 = parse_test("cat <<EOF\nhello\nEOF\n").unwrap();
        let p2 = parse_test("cat <<EOF\nhello\nEOF\n").unwrap();
        let r1 = &match &p1.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        }
        .redirections[0]
            .here_doc;
        let r2 = &match &p2.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        }
        .redirections[0]
            .here_doc;
        assert_eq!(r1, r2);
    }

    #[test]
    fn error_redirect_only_no_command() {
        let result = parse_test("< ;");
        assert!(result.is_err());
    }

    #[test]
    fn consume_paren_body_backslash_escape_non_newline() {
        let program = parse_test("echo $(echo a\\b)").expect("backslash escape inside $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.windows(b"\\b".len()).any(|w| w == b"\\b"));
    }

    #[test]
    fn consume_paren_body_backtick_inside_command_substitution() {
        let program = parse_test("echo $(echo `echo inner`)").expect("backtick inside $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.words[1]
                .raw
                .windows(b"`echo inner`".len())
                .any(|w| w == b"`echo inner`")
        );
    }

    #[test]
    fn nested_alias_pop_exhausted_layers_break() {
        let aliases = alias_map(&[(b"outer", b"inner rest"), (b"inner", b"echo")]);
        let program =
            parse_with_aliases_test("outer\n", &aliases).expect("nested alias should parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, b"echo");
            assert_eq!(&*cmd.words[1].raw, b"rest");
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn skip_continuations_pushback_between_operators() {
        let program = parse_test("echo a >\\\nout\n").expect("continuation in redirect");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn unterminated_single_quote_in_alias_layer() {
        let aliases = alias_map(&[(b"bad", b"echo 'unterminated")]);
        let result = parse_with_aliases_test("bad\n", &aliases);
        assert!(result.is_err());
    }

    #[test]
    fn empty_heredoc_delimiter_error_2() {
        let result = parse_test("cat << \n");
        assert!(result.is_err());
    }

    #[test]
    fn heredoc_line_huge_ionumber_becomes_word() {
        let big = "99999999999999999999";
        let src = format!("cat <<EOF {big}hello\nbody\nEOF\n");
        let result = parse_test(&src);
        assert!(result.is_ok());
    }

    #[test]
    fn produce_word_eof_on_empty_prefix_and_delim() {
        let program = parse_test("echo $()\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(cmd.words.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn case_fall_through_semi_amp() {
        let program = parse_test("case x in a) echo a ;& b) echo b ;; esac").expect("parse");
        if let Command::Case(c) = &program.items[0].and_or.first.commands[0] {
            assert!(c.arms[0].fallthrough);
            assert!(!c.arms[1].fallthrough);
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn case_semi_before_non_esac_error() {
        let result = parse_test("case x in a) echo a ; b) echo b ;; esac");
        assert!(result.is_err());
    }

    #[test]
    fn case_arm_without_separator_before_esac() {
        let program = parse_test("case x in a) echo a\nesac").expect("parse");
        if let Command::Case(c) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(c.arms.len(), 1);
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn for_loop_break_on_non_word() {
        let program = parse_test("for i in a b; do echo $i; done").expect("parse");
        if let Command::For(f) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(f.items.as_ref().unwrap().len(), 2);
        } else {
            panic!("expected for command");
        }
    }

    #[test]
    fn syntax_error_unexpected_lparen_after_name_without_rparen() {
        let result = parse_test("foo(bar");
        assert!(result.is_err());
    }

    #[test]
    fn empty_command_at_if_position() {
        let result = parse_test("if ; then true; fi");
        assert!(result.is_err());
    }

    #[test]
    fn for_loop_non_word_token_breaks_word_list() {
        let program = parse_test("for i in a b\ndo echo $i; done").expect("parse");
        if let Command::For(f) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(f.items.as_ref().unwrap().len(), 2);
        } else {
            panic!("expected for command");
        }
    }

    #[test]
    fn reclassify_queued_token_trailing_blank_alias_on_heredoc_line() {
        let aliases = alias_map(&[(b"mycmd", b"echo "), (b"myarg", b"hello")]);
        let program =
            parse_with_aliases_test("cat <<EOF mycmd myarg\nbody\nEOF\n", &aliases).expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn heredoc_line_skip_continuations_between_tokens() {
        let program = parse_test("cat <<EOF >\\\nout\nbody\nEOF\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn backslash_newline_at_word_start_before_delim() {
        let program = parse_test("echo \\\n\n").expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn skip_continuations_pushback_on_heredoc_line() {
        let program = parse_test("cat <<EOF >\\out\nbody\nEOF\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn heredoc_line_large_number_not_ionumber() {
        let big = "999999999999";
        let src = format!("cat <<EOF {big}word\nbody\nEOF\n");
        let program = parse_test(&src).expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn empty_heredoc_delimiter_after_ltlt() {
        let result = parse_test("cat <<\n");
        assert!(result.is_err());
    }

    #[test]
    fn reclassify_trailing_blank_alias_on_heredoc_line() {
        let aliases = alias_map(&[(b"myalias", b"echo ")]);
        let program = parse_with_aliases_test("cat <<EOF myalias world\nbody\nEOF\n", &aliases)
            .expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn produce_word_returns_eof_on_empty_continuation() {
        let program = parse_test("echo \\\n  \n").expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn alias_trailing_blank_triggers_reclassify_on_heredoc_line() {
        let aliases = alias_map(&[(b"A", b"cat "), (b"B", b"extra")]);
        let program = parse_with_aliases_test("A <<EOF B\nhello\nEOF\n", &aliases).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, b"cat");
            assert_eq!(&*cmd.words[1].raw, b"extra");
            assert_eq!(cmd.redirections.len(), 1);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn second_heredoc_empty_delimiter_error() {
        assert!(parse_test("cat <<EOF <<\nhello\nEOF\n").is_err());
    }

    #[test]
    fn heredoc_line_overflow_ionumber_becomes_word() {
        let src = "cat <<EOF 99999999999>out\nhello\nEOF\n";
        let program = parse_test(src).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[1].raw, b"99999999999");
            assert_eq!(cmd.redirections.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn alias_expanding_to_blanks_produces_eof_in_produce_word() {
        let aliases = alias_map(&[(b"A", b"   ")]);
        let program = parse_with_aliases_test("A ; echo done\n", &aliases).expect("parse");
        assert!(program.items.is_empty());
    }

    #[test]
    fn alias_ineligible_word_on_heredoc_line_skips_expansion() {
        let aliases = alias_map(&[(b"A", b"cat "), (b"'B'", b"extra")]);
        let program =
            parse_with_aliases_test("A <<EOF 'B'\nhello\nEOF\n", &aliases).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, b"cat");
            assert_eq!(&*cmd.words[1].raw, b"'B'");
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn unterminated_case_arm_breaks_loop() {
        assert!(parse_test("case x in x) echo hi").is_err());
    }

    #[test]
    fn case_pattern_accepts_keyword_word() {
        let program = parse_test("case if in if) echo ok;; esac").expect("parse");
        assert_eq!(program.items.len(), 1);
        if let Command::Case(case) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*case.arms[0].patterns[0].raw, b"if");
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn function_keyword_without_parens() {
        let program = parse_test("function foo { echo hi; }").expect("parse");
        assert_eq!(program.items.len(), 1);
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == b"foo"
        ));
    }

    #[test]
    fn self_referential_alias_does_not_loop() {
        let aliases = alias_map(&[(b"a", b"a")]);
        let program =
            parse_with_aliases_test("a\n", &aliases).expect("self-referential alias should parse");
        assert_eq!(program.items.len(), 1);
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, b"a");
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn empty_subshell_is_syntax_error() {
        assert!(parse(b"( )\n").is_err());
    }

    #[test]
    fn empty_brace_group_is_syntax_error() {
        assert!(parse(b"{ }\n").is_err());
    }

    #[test]
    fn empty_do_group_is_syntax_error() {
        assert!(parse(b"for i in a; do done\n").is_err());
        assert!(parse(b"while true; do done\n").is_err());
        assert!(parse(b"until true; do done\n").is_err());
    }

    #[test]
    fn empty_then_clause_is_syntax_error() {
        assert!(parse(b"if true; then fi\n").is_err());
        assert!(parse(b"if true; then true; elif true; then fi\n").is_err());
        assert!(parse(b"if true; then true; else fi\n").is_err());
    }

    #[test]
    fn leading_semicolon_is_syntax_error() {
        assert!(parse(b";\n").is_err());
        assert!(parse(b"; echo hi\n").is_err());
    }

    #[test]
    fn multiple_prefix_assignments_before_command() {
        let program = parse_test("A=1 B=2 echo hi").unwrap();
        let cmd = &program.items[0].and_or.first.commands[0];
        if let Command::Simple(sc) = cmd {
            assert_eq!(sc.assignments.len(), 2);
            assert_eq!(&*sc.assignments[0].name, b"A");
            assert_eq!(&*sc.assignments[0].value.raw, b"1");
            assert_eq!(&*sc.assignments[1].name, b"B");
            assert_eq!(&*sc.assignments[1].value.raw, b"2");
            assert_eq!(sc.words.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }
}
