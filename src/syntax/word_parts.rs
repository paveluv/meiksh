use std::rc::Rc;

use super::ast::Program;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WordPart {
    Literal {
        start: usize,
        end: usize,
        has_glob: bool,
        newlines: u16,
    },
    QuotedLiteral {
        bytes: Box<[u8]>,
        newlines: u16,
    },
    TildeLiteral {
        user_end: usize,
        end: usize,
    },
    Expansion {
        kind: ExpansionKind,
        quoted: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExpansionKind {
    SimpleVar {
        start: usize,
        end: usize,
    },
    Positional {
        index: u8,
    },
    SpecialVar {
        ch: u8,
    },
    ShellName,
    Braced {
        name: BracedName,
        op: BracedOp,
        parts: Box<[WordPart]>,
    },
    Command {
        program: Rc<Program>,
    },
    Arithmetic {
        parts: Box<[WordPart]>,
    },
    ArithmeticLiteral {
        start: usize,
        end: usize,
    },
    LiteralDollar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BracedName {
    Var {
        start: usize,
        end: usize,
    },
    Positional {
        start: usize,
        end: usize,
        index: u32,
    },
    Special {
        start: usize,
        end: usize,
        ch: u8,
    },
}

impl BracedName {
    pub(crate) fn name_range(&self) -> (usize, usize) {
        match self {
            BracedName::Var { start, end }
            | BracedName::Positional { start, end, .. }
            | BracedName::Special { start, end, .. } => (*start, *end),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BracedOp {
    None,
    Length,
    Default,
    DefaultColon,
    Assign,
    AssignColon,
    Error,
    ErrorColon,
    Alt,
    AltColon,
    TrimSuffix,
    TrimSuffixLong,
    TrimPrefix,
    TrimPrefixLong,
}
