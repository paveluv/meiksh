use std::rc::Rc;

use super::ast::Program;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WordPart {
    Literal {
        start: usize,
        end: usize,
        has_glob: bool,
        newlines: u16,
        /// True iff this literal is at index 0 of the enclosing `Word.parts`,
        /// the source bytes `[start..end)` end with an unquoted unescaped `=`
        /// at position `end-1`, and bytes `[start..end-1)` form a POSIX NAME
        /// with every byte unquoted and unescaped. Set exclusively by the
        /// parser at AST-build time (via
        /// `assignment_context::apply_assignment_context_to_argv_word`) for
        /// argv words attached to a declaration-utility call. Consumed by
        /// the declaration-utility expander to identify and split argv
        /// tokens like `A=value` without re-parsing.
        assignment: bool,
    },
    QuotedLiteral {
        bytes: Vec<u8>,
        newlines: u16,
    },
    TildeLiteral {
        tilde_pos: usize,
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
        parts: Vec<WordPart>,
    },
    Command {
        program: Rc<Program>,
    },
    Arithmetic {
        parts: Vec<WordPart>,
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
