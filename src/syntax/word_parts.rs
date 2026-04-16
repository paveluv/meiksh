use std::rc::Rc;

use super::ast::Program;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WordPart {
    Literal { start: usize, end: usize },
    QuotedLiteral { bytes: Box<[u8]> },
    Tilde { end: usize },
    Expand { kind: ExpansionKind, quoted: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExpansionKind {
    SimpleVar { start: usize, end: usize },
    SpecialVar { ch: u8 },
    Braced {
        name_start: usize,
        name_end: usize,
        op: BracedOp,
        parts: Box<[WordPart]>,
    },
    Command { program: Rc<Program> },
    Arithmetic { parts: Box<[WordPart]> },
    LiteralDollar,
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
