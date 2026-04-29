use std::rc::Rc;

use crate::shell::vars::CachedVarBinding;

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
        /// `declaration_context::apply_assignment_context_to_argv_word`) for
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
        /// Lazily-populated cache of the `VarTable` slot that
        /// `raw[start..end]` resolves to at execution time. Filled
        /// on the first expansion and reused on all subsequent ones
        /// inside the same shell, avoiding the `ShellMap` lookup.
        cache: CachedVarBinding,
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
    /// Process substitution `<(list)` / `>(list)` per
    /// `docs/features/process-substitution.md`. The `direction`
    /// distinguishes the read form (`<(...)`, parent reads from the
    /// substitution fd) from the write form (`>(...)`, parent writes
    /// to it). Recognized only when `bash_procsub` is on at parse
    /// time; the lexer rejects the tokens with the option off
    /// (§ 9.1).
    ProcSubstitution {
        program: Rc<Program>,
        direction: ProcSubDirection,
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

/// Which side of the process-substitution pipe the parent shell
/// holds. See `docs/features/process-substitution.md` § 5.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcSubDirection {
    /// `<(list)` — child writes to the pipe, parent reads. The
    /// substituted path opens read-only.
    Read,
    /// `>(list)` — child reads from the pipe, parent writes. The
    /// substituted path opens write-only.
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BracedName {
    Var {
        start: usize,
        end: usize,
        /// Lazily-populated cache of the `VarTable` slot for the
        /// variable name. See [`ExpansionKind::SimpleVar::cache`].
        cache: CachedVarBinding,
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
            BracedName::Var { start, end, .. }
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
