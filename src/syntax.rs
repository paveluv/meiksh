//! Single-pass POSIX shell parser.
//!
//! # Architecture
//!
//! This is a scannerless recursive-descent parser: there is no separate
//! tokenizer.  The central routine [`Parser::scan_word`] reads characters
//! directly from the source (or from an alias-expansion overlay) and
//! simultaneously drives two tries — one for keywords, one for aliases —
//! so that the scanned word is classified in the same pass that reads it.
//! The result, [`ScanResult`], tells callers whether the word is a plain
//! word, a reserved keyword, or an alias match, without ever re-reading
//! the bytes.
//!
//! # Key invariant — single pass
//!
//! Every source character is read and compared at most once during
//! parsing.  To avoid re-scanning when a word is read but belongs to a
//! downstream parser, a one-slot **pushback buffer** (`Parser::pushed_back`)
//! stores the [`ScanResult`] so the next `scan_word` call retrieves it
//! directly.  When the pushback carries a higher classification than the
//! consumer needs (e.g. a `Keyword` retrieved by a caller that passed
//! `keyword_ok = false`), `scan_word` **downgrades** it on the fly.
//!
//! # Alias expansion
//!
//! Aliases are expanded by [`Parser::expand_alias_at_command_position`],
//! which scans the first word at command position with both tries active.
//! If the word matches an alias, a new [`AliasLayer`] is pushed onto the
//! input stack and the scan loops.  If not, the word (already classified)
//! is pushed back.  Downstream parsers — `parse_pipeline`, `parse_command`,
//! etc. — also call `expand_alias_at_command_position`, but the pushback
//! guard (`if self.pushed_back.is_some() { return Ok(()); }`) makes every
//! call after the first an O(1) no-op.
//!
//! # Lifetime `'src`
//!
//! All AST nodes borrow directly from the source string **or** from
//! strings leaked via [`Parser::intern`] (used for backslash-newline
//! stripping and alias values).  There is no arena; interned strings are
//! individually heap-allocated and leaked to `'static`, then transmuted
//! to `'src` — safe because the AST never outlives the source.

use std::collections::{HashMap, VecDeque};
use std::fmt;

// ============================================================
// AST types
// ============================================================
//
// POSIX shell grammar, bottom-up:
//
//   Program      = ListItem*
//   ListItem     = AndOr ('&')?          (optionally asynchronous)
//   AndOr        = Pipeline ('&&'|'||' Pipeline)*
//   Pipeline     = ['time' ['-p']] ['!'] Command ('|' Command)*
//   Command      = Simple | Subshell | Group | If | Loop | For | Case
//                 | FunctionDef | Redirected(Command, Redir*)
//   SimpleCommand = Assignment* Word* Redirection*
//

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program<'src> {
    pub items: Box<[ListItem<'src>]>,
}

/// One entry in a command list.  `asynchronous` is true when the item
/// is terminated by `&`, causing it to run in the background.
/// `line` records the source line for diagnostic messages.
/// Equality ignores `line` (same rationale as `Word`).
#[derive(Clone, Debug)]
pub struct ListItem<'src> {
    pub and_or: AndOr<'src>,
    pub asynchronous: bool,
    pub line: usize,
}

impl PartialEq for ListItem<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.and_or == other.and_or && self.asynchronous == other.asynchronous
    }
}
impl Eq for ListItem<'_> {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndOr<'src> {
    pub first: Pipeline<'src>,
    pub rest: Box<[(LogicalOp, Pipeline<'src>)]>,
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
pub struct Pipeline<'src> {
    pub negated: bool,
    pub timed: TimedMode,
    pub commands: Box<[Command<'src>]>,
}

/// A single shell command.  `Redirected` wraps a compound command with
/// trailing redirections (simple commands carry redirections inline).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command<'src> {
    Simple(SimpleCommand<'src>),
    Subshell(Program<'src>),
    Group(Program<'src>),
    FunctionDef(FunctionDef<'src>),
    If(IfCommand<'src>),
    Loop(LoopCommand<'src>),
    For(ForCommand<'src>),
    Case(CaseCommand<'src>),
    Redirected(Box<Command<'src>>, Box<[Redirection<'src>]>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimpleCommand<'src> {
    pub assignments: Box<[Assignment<'src>]>,
    pub words: Box<[Word<'src>]>,
    pub redirections: Box<[Redirection<'src>]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment<'src> {
    pub name: &'src str,
    pub value: Word<'src>,
}

/// A shell word — the raw source text before expansion.
/// `line` records where the word appeared for diagnostics.
/// Equality ignores `line` so tests can compare ASTs without position noise.
#[derive(Clone, Debug)]
pub struct Word<'src> {
    pub raw: &'src str,
    pub line: usize,
}

impl PartialEq for Word<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl Eq for Word<'_> {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redirection<'src> {
    pub fd: Option<i32>,
    pub kind: RedirectionKind,
    pub target: Word<'src>,
    pub here_doc: Option<HereDoc<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionDef<'src> {
    pub name: &'src str,
    pub body: Box<Command<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfCommand<'src> {
    pub condition: Program<'src>,
    pub then_branch: Program<'src>,
    pub elif_branches: Box<[ElifBranch<'src>]>,
    pub else_branch: Option<Program<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElifBranch<'src> {
    pub condition: Program<'src>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopCommand<'src> {
    pub kind: LoopKind,
    pub condition: Program<'src>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForCommand<'src> {
    pub name: &'src str,
    pub items: Option<Box<[Word<'src>]>>,
    pub body: Program<'src>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCommand<'src> {
    pub word: Word<'src>,
    pub arms: Box<[CaseArm<'src>]>,
}

/// One arm of a `case` statement.  `fallthrough` is true when the arm
/// is terminated by `;&` instead of `;;`, meaning execution continues
/// into the next arm's body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseArm<'src> {
    pub patterns: Box<[Word<'src>]>,
    pub body: Program<'src>,
    pub fallthrough: bool,
}

/// A here-document body.
///
/// * `expand` — `true` unless the delimiter was quoted (e.g. `<<'EOF'`),
///   in which case the body is taken literally.
/// * `strip_tabs` — `true` for `<<-`, which strips leading tabs from
///   each body line and from the delimiter line.
/// * `body_line` — the source line where the body starts, for diagnostics.
///
/// Tab stripping and `\\\n` continuation are left in the raw body here;
/// normalization happens at expansion time in `exec.rs`.
#[derive(Clone, Debug)]
pub struct HereDoc<'src> {
    pub delimiter: &'src str,
    pub body: &'src str,
    pub expand: bool,
    pub strip_tabs: bool,
    pub body_line: usize,
}

impl PartialEq for HereDoc<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.delimiter == other.delimiter
            && self.body == other.body
            && self.expand == other.expand
            && self.strip_tabs == other.strip_tabs
    }
}
impl Eq for HereDoc<'_> {}

// ============================================================
// into_static
// ============================================================
//
// Shell function bodies are stored in `Shell::functions` and must
// outlive any single parse.  `into_static` converts a `Command<'src>`
// to `Command<'static>` by leaking every borrowed slice via `leak_str`.
// This is intentionally leaky — function bodies are typically few and
// small, and the alternative (an arena with complex lifetimes) is not
// worth the ergonomic cost.

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

impl<'src> Command<'src> {
    pub fn into_static(self) -> Command<'static> {
        self.convert_static()
    }

    fn convert_static(self) -> Command<'static> {
        match self {
            Command::Simple(cmd) => Command::Simple(SimpleCommand {
                assignments: cmd
                    .assignments
                    .into_vec()
                    .into_iter()
                    .map(|a| Assignment {
                        name: leak_str(a.name),
                        value: Word {
                            raw: leak_str(a.value.raw),
                            line: a.value.line,
                        },
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                words: cmd
                    .words
                    .into_vec()
                    .into_iter()
                    .map(|w| Word {
                        raw: leak_str(w.raw),
                        line: w.line,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                redirections: cmd
                    .redirections
                    .into_vec()
                    .into_iter()
                    .map(redir_convert)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }),
            Command::Subshell(p) => Command::Subshell(program_convert(p)),
            Command::Group(p) => Command::Group(program_convert(p)),
            Command::FunctionDef(f) => Command::FunctionDef(FunctionDef {
                name: leak_str(f.name),
                body: Box::new(f.body.convert_static()),
            }),
            Command::If(c) => Command::If(IfCommand {
                condition: program_convert(c.condition),
                then_branch: program_convert(c.then_branch),
                elif_branches: c
                    .elif_branches
                    .into_vec()
                    .into_iter()
                    .map(|b| ElifBranch {
                        condition: program_convert(b.condition),
                        body: program_convert(b.body),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                else_branch: c.else_branch.map(program_convert),
            }),
            Command::Loop(c) => Command::Loop(LoopCommand {
                kind: c.kind,
                condition: program_convert(c.condition),
                body: program_convert(c.body),
            }),
            Command::For(c) => Command::For(ForCommand {
                name: leak_str(c.name),
                items: c.items.map(|items| {
                    items
                        .into_vec()
                        .into_iter()
                        .map(|w| Word {
                            raw: leak_str(w.raw),
                            line: w.line,
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                }),
                body: program_convert(c.body),
            }),
            Command::Case(c) => Command::Case(CaseCommand {
                word: Word {
                    raw: leak_str(c.word.raw),
                    line: c.word.line,
                },
                arms: c
                    .arms
                    .into_vec()
                    .into_iter()
                    .map(|arm| CaseArm {
                        patterns: arm
                            .patterns
                            .into_vec()
                            .into_iter()
                            .map(|w| Word {
                                raw: leak_str(w.raw),
                                line: w.line,
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        body: program_convert(arm.body),
                        fallthrough: arm.fallthrough,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }),
            Command::Redirected(cmd, redirs) => Command::Redirected(
                Box::new(cmd.convert_static()),
                redirs
                    .into_vec()
                    .into_iter()
                    .map(redir_convert)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }
    }
}

fn program_convert(p: Program<'_>) -> Program<'static> {
    Program {
        items: p
            .items
            .into_vec()
            .into_iter()
            .map(|item| ListItem {
                and_or: AndOr {
                    first: pipeline_convert(item.and_or.first),
                    rest: item
                        .and_or
                        .rest
                        .into_vec()
                        .into_iter()
                        .map(|(op, pl)| (op, pipeline_convert(pl)))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
                asynchronous: item.asynchronous,
                line: item.line,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn pipeline_convert(p: Pipeline<'_>) -> Pipeline<'static> {
    Pipeline {
        negated: p.negated,
        timed: p.timed,
        commands: p
            .commands
            .into_vec()
            .into_iter()
            .map(|c| c.convert_static())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn redir_convert(r: Redirection<'_>) -> Redirection<'static> {
    Redirection {
        fd: r.fd,
        kind: r.kind,
        target: Word {
            raw: leak_str(r.target.raw),
            line: r.target.line,
        },
        here_doc: r.here_doc.map(|hd| HereDoc {
            delimiter: leak_str(hd.delimiter),
            body: leak_str(hd.body),
            expand: hd.expand,
            strip_tabs: hd.strip_tabs,
            body_line: hd.body_line,
        }),
    }
}

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

// ============================================================
// AliasTrie — dynamic trie for O(1)/char alias lookup
// ============================================================
//
// Built from the shell's alias HashMap before each parse.  Each byte
// of an alias name transitions to a new state; terminal states map to
// an index into `values`.  State 0 is the dead state (no match),
// state 1 is the root.
//
// The trie is only rebuilt when the alias table changes (tracked by
// `generation`).  During `scan_word`, the trie is stepped in parallel
// with the keyword trie so that alias classification comes for free.

#[derive(Clone, Debug)]
pub struct AliasTrie {
    /// `transitions[state][byte]` → next state (0 = dead).
    transitions: Vec<[u16; 128]>,
    /// If `terminal[state]` is Some(i), the word ending here is alias #i.
    terminal: Vec<Option<usize>>,
    /// The alias expansion strings, indexed by terminal values.
    values: Vec<String>,
    pub generation: u64,
}

impl Default for AliasTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl AliasTrie {
    pub const NONE: u16 = 0;

    pub fn new() -> Self {
        Self {
            transitions: vec![[0u16; 128]],
            terminal: vec![None],
            values: Vec::new(),
            generation: 0,
        }
    }

    pub fn build(aliases: &HashMap<String, String>) -> Self {
        let mut trie = Self {
            transitions: vec![[0u16; 128], [0u16; 128]],
            terminal: vec![None, None],
            values: Vec::new(),
            generation: 0,
        };
        for (name, value) in aliases {
            if name.is_empty() {
                continue;
            }
            let mut state = 1u16;
            let mut valid = true;
            for &b in name.as_bytes() {
                let idx = b as usize;
                if idx >= 128 {
                    valid = false;
                    break;
                }
                let next = trie.transitions[state as usize][idx];
                if next != 0 {
                    state = next;
                } else {
                    let new_state = trie.transitions.len() as u16;
                    trie.transitions.push([0u16; 128]);
                    trie.terminal.push(None);
                    trie.transitions[state as usize][idx] = new_state;
                    state = new_state;
                }
            }
            if valid {
                let val_idx = trie.values.len();
                trie.values.push(value.clone());
                trie.terminal[state as usize] = Some(val_idx);
            }
        }
        trie
    }

    pub fn root(&self) -> u16 {
        if self.transitions.len() > 1 {
            1
        } else {
            0
        }
    }

    #[inline]
    pub fn step(&self, state: u16, ch: u8) -> u16 {
        if state == 0 || (ch as usize) >= 128 {
            return 0;
        }
        self.transitions[state as usize][ch as usize]
    }

    pub fn terminal(&self, state: u16) -> Option<usize> {
        if state == 0 {
            return None;
        }
        self.terminal.get(state as usize).copied().flatten()
    }

    pub fn value(&self, index: usize) -> &str {
        &self.values[index]
    }
}

// ============================================================
// Keyword trie — compile-time constant for reserved words
// ============================================================
//
// A hand-written trie encoded as a match table: `kw_step(state, byte)`
// returns the next state.  State 0 is dead (no match), state 1 is root.
// `kw_terminal(state)` returns Some(Keyword) at accepting states.
//
// The trie covers the 14 POSIX reserved words: case, do, done, elif,
// else, esac, fi, for, function, if, in, then, until, while.
// It is stepped inside `scan_word` one byte at a time, in parallel with
// the alias trie, adding zero overhead to word scanning.

const KW_NONE: u8 = 0;
const KW_ROOT: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keyword {
    Case,
    Do,
    Done,
    Elif,
    Else,
    Esac,
    Fi,
    For,
    Function,
    If,
    In,
    Then,
    Until,
    While,
}

#[inline]
fn kw_step(state: u8, ch: u8) -> u8 {
    match (state, ch) {
        (1, b'c') => 2,
        (1, b'd') => 6,
        (1, b'e') => 10,
        (1, b'f') => 19,
        (1, b'i') => 30,
        (1, b't') => 33,
        (1, b'u') => 37,
        (1, b'w') => 42,
        (2, b'a') => 3,
        (3, b's') => 4,
        (4, b'e') => 5,
        (6, b'o') => 7,
        (7, b'n') => 8,
        (8, b'e') => 9,
        (10, b'l') => 11,
        (10, b's') => 16,
        (11, b'i') => 12,
        (11, b's') => 14,
        (12, b'f') => 13,
        (14, b'e') => 15,
        (16, b'a') => 17,
        (17, b'c') => 18,
        (19, b'i') => 20,
        (19, b'o') => 21,
        (19, b'u') => 23,
        (21, b'r') => 22,
        (23, b'n') => 24,
        (24, b'c') => 25,
        (25, b't') => 26,
        (26, b'i') => 27,
        (27, b'o') => 28,
        (28, b'n') => 29,
        (30, b'f') => 31,
        (30, b'n') => 32,
        (33, b'h') => 34,
        (34, b'e') => 35,
        (35, b'n') => 36,
        (37, b'n') => 38,
        (38, b't') => 39,
        (39, b'i') => 40,
        (40, b'l') => 41,
        (42, b'h') => 43,
        (43, b'i') => 44,
        (44, b'l') => 45,
        (45, b'e') => 46,
        _ => 0,
    }
}

fn kw_terminal(state: u8) -> Option<Keyword> {
    match state {
        5 => Some(Keyword::Case),
        7 => Some(Keyword::Do),
        9 => Some(Keyword::Done),
        13 => Some(Keyword::Elif),
        15 => Some(Keyword::Else),
        18 => Some(Keyword::Esac),
        20 => Some(Keyword::Fi),
        22 => Some(Keyword::For),
        29 => Some(Keyword::Function),
        31 => Some(Keyword::If),
        32 => Some(Keyword::In),
        36 => Some(Keyword::Then),
        41 => Some(Keyword::Until),
        46 => Some(Keyword::While),
        _ => None,
    }
}

// ============================================================
// Internal types
// ============================================================

/// A heredoc whose delimiter has been parsed but whose body hasn't
/// been read yet.  Bodies are read at the next newline (POSIX requires
/// heredoc bodies to follow the complete command on the next line).
struct PendingHereDoc {
    delimiter: String,
    strip_tabs: bool,
    expand: bool,
}

/// An overlay on the input stream produced by alias expansion.
/// When an alias is expanded, its value is pushed as a new layer.
/// The parser reads from the topmost layer until exhausted, then
/// falls back to the layer beneath (or the main source).
struct AliasLayer<'src> {
    text: &'src str,
    pos: usize,
    /// POSIX: if an alias value ends with a blank, the next word at
    /// command position is also subject to alias expansion.
    trailing_blank: bool,
}

/// Result of scanning one word from the source.
/// Carries keyword/alias trie results so callers never re-scan.
enum ScanResult<'src> {
    /// A plain word (no keyword or alias match).
    Word(&'src str),
    /// A reserved word recognized by the keyword trie.
    Keyword(Keyword, &'src str),
    /// A word that matched an alias in the alias trie.
    Alias { index: usize, raw: &'src str },
    /// Nothing was scanned (EOF or delimiter at current position).
    None,
}

fn keyword_name(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Case => "case",
        Keyword::Do => "do",
        Keyword::Done => "done",
        Keyword::Elif => "elif",
        Keyword::Else => "else",
        Keyword::Esac => "esac",
        Keyword::Fi => "fi",
        Keyword::For => "for",
        Keyword::Function => "function",
        Keyword::If => "if",
        Keyword::In => "in",
        Keyword::Then => "then",
        Keyword::Until => "until",
        Keyword::While => "while",
    }
}

/// Characters that terminate a word AND prevent one from starting.
/// Includes `#` because an unquoted `#` at the beginning of a token
/// starts a comment.
fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>' | b'#'
    )
}

/// Characters that terminate a word mid-scan.  Same as `is_delim` but
/// without `#`: a `#` inside a word (e.g. after `\\\n` continuation) is
/// a literal character, not a comment.  `#` only starts a comment at
/// token boundaries, which `scan_word` handles via the `word_started` flag.
fn is_word_break(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>'
    )
}

fn alias_has_trailing_blank(s: &str) -> bool {
    matches!(s.as_bytes().last(), Some(b' ' | b'\t'))
}

/// An alias word must not contain quoting characters — those would have
/// been consumed by the quote-scanning arms before the trie could match.
fn is_alias_word(word: &str) -> bool {
    !word.is_empty() && !word.chars().any(|ch| matches!(ch, '\'' | '"' | '\\'))
}

// ============================================================
// Parser
// ============================================================
//
// State overview:
//
//  source                – the entire input string
//  pos / line            – current read position and line counter
//  alias_stack           – stack of alias expansion overlays (read before `source`)
//  expanding_aliases     – names currently being expanded (prevents recursion)
//  alias_depth           – depth counter for alias nesting limit (1024)
//  alias_trailing_blank_pending – set when an exhausted alias layer had a trailing
//                          blank, signaling the next word at command position
//                          should also attempt alias expansion
//  pending_heredocs      – heredocs whose delimiters were parsed but bodies not yet read
//  read_heredocs         – bodies read at newline, waiting to be attached to AST nodes
//  pushed_back           – one-slot pushback to avoid re-scanning (see module doc)

pub struct Parser<'src> {
    source: &'src str,
    pos: usize,
    line: usize,
    alias_stack: Vec<AliasLayer<'src>>,
    alias_depth: usize,
    expanding_aliases: Vec<String>,
    alias_trailing_blank_pending: bool,
    pending_heredocs: Vec<PendingHereDoc>,
    read_heredocs: VecDeque<HereDoc<'src>>,
    pushed_back: Option<ScanResult<'src>>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            alias_stack: Vec::new(),
            alias_depth: 0,
            expanding_aliases: Vec::new(),
            alias_trailing_blank_pending: false,
            pending_heredocs: Vec::new(),
            read_heredocs: VecDeque::new(),
            pushed_back: None,
        }
    }

    pub fn current_line(&self) -> usize {
        self.line
    }

    /// Leak a string to get a `&'static str` and transmute the lifetime to
    /// `'src`.  Used for words that differ from the source slice (e.g. after
    /// stripping `\\\n` continuations) and for interning alias expansion text.
    /// Safe because the AST is dropped before or with the parser.
    fn intern(&mut self, s: String) -> &'src str {
        Box::leak(s.into_boxed_str())
    }

    fn error(&self, message: impl Into<Box<str>>) -> ParseError {
        ParseError {
            message: message.into(),
            line: Some(self.line),
        }
    }

    // ---- Input layer management ----
    //
    // The parser reads from a virtual stream that can have alias-expansion
    // overlays on top of the main source.  The topmost non-exhausted alias
    // layer is read first; when it runs out, we fall back to the layer
    // below (or to `self.source`).  Every read primitive below follows
    // this pattern: pop exhausted layers, then read from the topmost layer
    // or from `self.source`.

    fn in_alias(&self) -> bool {
        !self.alias_stack.is_empty()
    }

    /// Remove fully-consumed alias layers.  When popping a layer that had
    /// a trailing blank, record it so `parse_simple_command` can trigger
    /// alias expansion on the next word.
    fn pop_exhausted_layers(&mut self) {
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

    /// Return the next byte from the virtual stream without consuming it.
    fn peek_byte(&mut self) -> Option<u8> {
        self.pop_exhausted_layers();
        if let Some(layer) = self.alias_stack.last() {
            return layer.text.as_bytes().get(layer.pos).copied();
        }
        self.source.as_bytes().get(self.pos).copied()
    }

    /// Look ahead `offset` bytes from the current position in the current
    /// layer.  Does **not** span across layer boundaries.
    fn peek_byte_at_offset(&self, offset: usize) -> Option<u8> {
        if let Some(layer) = self.alias_stack.last() {
            if layer.pos < layer.text.len() {
                return layer.text.as_bytes().get(layer.pos + offset).copied();
            }
        }
        self.source.as_bytes().get(self.pos + offset).copied()
    }

    /// Consume one byte, tracking newlines for line counting.
    fn advance_byte(&mut self) {
        self.pop_exhausted_layers();
        if let Some(layer) = self.alias_stack.last_mut() {
            layer.pos += 1;
        } else if self.pos < self.source.len() {
            if self.source.as_bytes()[self.pos] == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
    }

    /// True only when every layer and the main source are exhausted **and**
    /// there is nothing in the pushback buffer.
    fn at_eof(&mut self) -> bool {
        if self.pushed_back.is_some() {
            return false;
        }
        self.pop_exhausted_layers();
        self.alias_stack.is_empty() && self.pos >= self.source.len()
    }

    fn push_back(&mut self, result: ScanResult<'src>) {
        debug_assert!(self.pushed_back.is_none(), "double pushback");
        self.pushed_back = Some(result);
    }

    // ---- Whitespace / separator handling ----
    //
    // POSIX distinguishes three levels of whitespace skipping:
    //   skip_blanks:             spaces, tabs, and backslash-newline
    //   skip_blanks_and_comments: blanks + `#`-comments
    //   skip_linebreaks:         blanks + comments + newlines (reads heredocs)
    //   skip_separators:         blanks + comments + newlines + lone `;`

    fn skip_blanks(&mut self) {
        loop {
            self.pop_exhausted_layers();
            match self.peek_byte() {
                Some(b' ' | b'\t') => self.advance_byte(),
                Some(b'\\') if !self.in_alias() => {
                    if self.source.as_bytes().get(self.pos + 1) == Some(&b'\n') {
                        self.pos += 2;
                        self.line += 1;
                    } else {
                        break;
                    }
                }
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

    /// Skip newlines and lone `;` separators.  Does NOT consume `;;` or `;&`
    /// — those are case-arm terminators that must be handled by
    /// `parse_program_until`'s stop condition.
    fn skip_separators(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_blanks_and_comments();
            match self.peek_byte() {
                Some(b'\n') => {
                    self.advance_byte();
                    self.read_pending_heredocs()?;
                }
                Some(b';') if self.peek_byte_at_offset(1) != Some(b';')
                    && self.peek_byte_at_offset(1) != Some(b'&') =>
                {
                    self.advance_byte();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn skip_linebreaks(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_blanks_and_comments();
            if self.peek_byte() == Some(b'\n') {
                self.advance_byte();
                self.read_pending_heredocs()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn consume_amp(&mut self) -> bool {
        self.skip_blanks();
        if self.peek_byte() == Some(b'&') && self.peek_byte_at_offset(1) != Some(b'&') {
            self.advance_byte();
            true
        } else {
            false
        }
    }

    // ---- Quote/expansion scanning ----
    //
    // These routines advance the position past quoted/expanded regions
    // inside a word.  They are called from `scan_word` when a quoting
    // character is encountered.  They don't produce AST nodes — they
    // simply ensure the cursor moves past the entire quoted region so
    // that `scan_word` can slice the full raw word at the end.

    /// Advance past `'...'`.  The opening `'` must be the current byte.
    fn skip_single_quote(&mut self) -> Result<(), ParseError> {
        self.advance_byte();
        loop {
            match self.peek_byte() {
                None => {
                    return Err(ParseError {
                        message: "unterminated single quote".into(),
                        line: Some(self.line),
                    })
                }
                Some(b'\'') => {
                    self.advance_byte();
                    return Ok(());
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_double_quote(&mut self) -> Result<(), ParseError> {
        self.advance_byte();
        loop {
            match self.peek_byte() {
                None => {
                    return Err(ParseError {
                        message: "unterminated double quote".into(),
                        line: Some(self.line),
                    })
                }
                Some(b'"') => {
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    self.advance_byte();
                    if self.peek_byte().is_some() {
                        self.advance_byte();
                    }
                }
                Some(b'$') if matches!(self.peek_byte_at_offset(1), Some(b'(' | b'{')) => {
                    self.skip_dollar_construct()?;
                }
                Some(b'`') => {
                    self.advance_byte();
                    self.skip_backtick_inner()?;
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_dollar_construct(&mut self) -> Result<(), ParseError> {
        let next = self.peek_byte_at_offset(1);
        if next == Some(b'(') {
            if self.peek_byte_at_offset(2) == Some(b'(') {
                self.advance_byte();
                self.advance_byte();
                self.advance_byte();
                let mut depth = 1usize;
                loop {
                    match self.peek_byte() {
                        None => {
                            return Err(self.error("unterminated arithmetic expansion"))
                        }
                        Some(b'(') => {
                            depth += 1;
                            self.advance_byte();
                        }
                        Some(b')') => {
                            if depth == 1 && self.peek_byte_at_offset(1) == Some(b')') {
                                self.advance_byte();
                                self.advance_byte();
                                return Ok(());
                            }
                            depth = depth.saturating_sub(1);
                            self.advance_byte();
                        }
                        Some(_) => self.advance_byte(),
                    }
                }
            }
            self.advance_byte();
            self.advance_byte();
            return self.skip_paren_body();
        }
        if next == Some(b'{') {
            self.advance_byte();
            self.advance_byte();
            return self.skip_brace_body();
        }
        self.advance_byte();
        Ok(())
    }

    fn skip_dollar_single_quote(&mut self) -> Result<(), ParseError> {
        self.advance_byte();
        self.advance_byte();
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated dollar-single-quotes")),
                Some(b'\'') => {
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    self.advance_byte();
                    if self.peek_byte().is_some() {
                        self.advance_byte();
                    }
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_paren_body(&mut self) -> Result<(), ParseError> {
        let mut depth = 1usize;
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated command substitution")),
                Some(b'(') => {
                    depth += 1;
                    self.advance_byte();
                }
                Some(b')') => {
                    depth -= 1;
                    self.advance_byte();
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some(b'\'' | b'"' | b'\\' | b'$' | b'`') => {
                    self.skip_quoted_element()?;
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_brace_body(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated parameter expansion")),
                Some(b'}') => {
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\'' | b'"' | b'\\' | b'$' | b'`') => {
                    self.skip_quoted_element()?;
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_backtick_inner(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_byte() {
                None => return Err(self.error("unterminated backquote")),
                Some(b'`') => {
                    self.advance_byte();
                    return Ok(());
                }
                Some(b'\\') => {
                    self.advance_byte();
                    if self.peek_byte().is_some() {
                        self.advance_byte();
                    }
                }
                Some(_) => self.advance_byte(),
            }
        }
    }

    fn skip_quoted_element(&mut self) -> Result<(), ParseError> {
        match self.peek_byte() {
            Some(b'\'') => self.skip_single_quote(),
            Some(b'"') => self.skip_double_quote(),
            Some(b'\\') => {
                self.advance_byte();
                if self.peek_byte().is_some() {
                    self.advance_byte();
                }
                Ok(())
            }
            Some(b'$') if self.peek_byte_at_offset(1) == Some(b'\'') => {
                self.skip_dollar_single_quote()
            }
            Some(b'$') if matches!(self.peek_byte_at_offset(1), Some(b'(' | b'{')) => {
                self.skip_dollar_construct()
            }
            Some(b'`') => {
                self.advance_byte();
                self.skip_backtick_inner()
            }
            _ => {
                self.advance_byte();
                Ok(())
            }
        }
    }

    // ---- Word scanning ----

    /// Scan one word from the input, classifying it as a plain word, keyword,
    /// or alias match via two tries stepped in parallel with the character
    /// consumption loop.
    ///
    /// # Arguments
    ///
    /// * `keyword_ok` — whether the caller's grammar position allows reserved
    ///   words.  If false, even a keyword-matching word is returned as `Word`.
    /// * `alias_ok` — whether alias expansion is allowed here.  If false,
    ///   alias trie matches are ignored.
    /// * `alias_trie` — the current alias trie (may be empty).
    ///
    /// # Pushback
    ///
    /// If `self.pushed_back` already holds a result from a prior scan, that
    /// result is returned immediately — **downgraded** if the current caller's
    /// `keyword_ok`/`alias_ok` flags are more restrictive than the original
    /// scanner's.  This ensures a word is never read from the source twice.
    ///
    /// # Cross-layer boundary
    ///
    /// A single word cannot span across alias layer boundaries.  If the
    /// source pointer changes mid-word (e.g. an alias layer is exhausted
    /// and we fall back to `self.source`), the loop breaks and the word
    /// is sliced from the layer where it started.
    fn scan_word(
        &mut self,
        keyword_ok: bool,
        alias_ok: bool,
        alias_trie: &AliasTrie,
    ) -> Result<ScanResult<'src>, ParseError> {
        // --- Pushback fast-path ---
        if let Some(prev) = self.pushed_back.take() {
            return Ok(match prev {
                ScanResult::Keyword(_, raw) if !keyword_ok => ScanResult::Word(raw),
                ScanResult::Alias { raw, .. } if !alias_ok => ScanResult::Word(raw),
                other => other,
            });
        }

        self.skip_blanks_and_comments();
        if self.at_eof() || matches!(self.peek_byte(), Some(b) if is_delim(b)) {
            return Ok(ScanResult::None);
        }

        // Snapshot the source slice and start position so we can extract
        // the raw `&str` at the end.  A word is always fully contained
        // within one source buffer (main source or alias text).
        let source_text = if self.in_alias() {
            self.alias_stack.last().unwrap().text
        } else {
            self.source
        };
        let start = if self.in_alias() {
            self.alias_stack.last().unwrap().pos
        } else {
            self.pos
        };

        // Trie cursors — initialized to root if enabled, dead state if not.
        let mut kw: u8 = if keyword_ok { KW_ROOT } else { KW_NONE };
        let mut al: u16 = if alias_ok { alias_trie.root() } else { AliasTrie::NONE };
        let mut has_continuation = false;
        // `word_started` tracks whether we've consumed any actual word
        // characters.  A `#` only starts a comment if `word_started` is
        // false (i.e. at the beginning of a potential word).
        let mut word_started = false;
        let mut had_quote = false;

        // --- Main scan loop — each byte is read exactly once ---
        loop {
            self.pop_exhausted_layers();
            let cur_in_alias = self.in_alias();
            let cur_text = if cur_in_alias {
                self.alias_stack.last().unwrap().text
            } else {
                self.source
            };
            let cur_pos = if cur_in_alias {
                self.alias_stack.last().unwrap().pos
            } else {
                self.pos
            };

            // If the underlying buffer changed (layer exhausted) or the
            // position jumped backwards, this word ends here.
            if !std::ptr::eq(cur_text, source_text) || cur_pos < start {
                break;
            }

            match cur_text.as_bytes().get(cur_pos) {
                None => break,
                Some(&b) if is_word_break(b) => break,
                // `#` at the start of a token is a comment, not a word char
                Some(b'#') if !word_started => break,
                // Backslash-newline (line continuation) — splice the two
                // lines together.  Kills both trie cursors since the word
                // now spans a physical line break and can't be a keyword
                // or alias.
                Some(&b'\\')
                    if !cur_in_alias
                        && self.source.as_bytes().get(self.pos + 1) == Some(&b'\n') =>
                {
                    has_continuation = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.pos += 2;
                    self.line += 1;
                }
                // Quoting — kills trie cursors since quoted words can't
                // match keywords or aliases.
                Some(&b'\'') => {
                    word_started = true;
                    had_quote = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.skip_single_quote()?;
                }
                Some(&b'"') => {
                    word_started = true;
                    had_quote = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.skip_double_quote()?;
                }
                Some(&b'$') if matches!(cur_text.as_bytes().get(cur_pos + 1), Some(b'\'')) => {
                    word_started = true;
                    had_quote = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.skip_dollar_single_quote()?;
                }
                Some(&b'$') if matches!(cur_text.as_bytes().get(cur_pos + 1), Some(b'(' | b'{'))
                    =>
                {
                    word_started = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.skip_dollar_construct()?;
                }
                Some(&b'`') => {
                    word_started = true;
                    had_quote = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.advance_byte();
                    self.skip_backtick_inner()?;
                }
                Some(&b'\\') => {
                    word_started = true;
                    had_quote = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.advance_byte();
                    if self.peek_byte().is_some() {
                        self.advance_byte();
                    }
                }
                Some(&b'$') => {
                    word_started = true;
                    kw = KW_NONE;
                    al = AliasTrie::NONE;
                    self.advance_byte();
                }
                // Plain character — step both tries.
                Some(_) => {
                    word_started = true;
                    kw = kw_step(kw, cur_text.as_bytes()[cur_pos]);
                    al = alias_trie.step(al, cur_text.as_bytes()[cur_pos]);
                    self.advance_byte();
                }
            }
        }

        // Compute the end position in the same source buffer we started in.
        // If the current layer differs (alias exhausted), use the buffer's
        // full length.
        let end = {
            let cur_in_alias = self.in_alias();
            let cur_text = if cur_in_alias {
                self.alias_stack.last().unwrap().text
            } else {
                self.source
            };
            if std::ptr::eq(cur_text, source_text) {
                if cur_in_alias {
                    self.alias_stack.last().unwrap().pos
                } else {
                    self.pos
                }
            } else {
                source_text.len()
            }
        };

        if start == end {
            return Ok(ScanResult::None);
        }

        let raw = &source_text[start..end];

        // If the word contained `\\\n`, strip the continuations and intern
        // the result since it no longer matches the source exactly.
        let raw = if has_continuation {
            let stripped = raw.replace("\\\n", "");
            self.intern(stripped)
        } else {
            raw
        };

        // Classify: alias before keyword (POSIX 2.3.1).
        // Quoting disables both (a quoted `if` is a plain word, not a keyword).
        if !had_quote {
            if alias_ok {
                if let Some(idx) = alias_trie.terminal(al) {
                    return Ok(ScanResult::Alias { index: idx, raw });
                }
            }
            if keyword_ok {
                if let Some(matched_kw) = kw_terminal(kw) {
                    return Ok(ScanResult::Keyword(matched_kw, raw));
                }
            }
        }

        Ok(ScanResult::Word(raw))
    }

    /// Try to consume a specific plain word (like "}" or "time"). Returns true if consumed.
    /// Uses keyword_ok=true so that keywords pushed back retain their classification.
    fn consume_word_if(&mut self, expected: &str, alias_trie: &AliasTrie) -> Result<bool, ParseError> {
        match self.scan_word(true, false, alias_trie)? {
            ScanResult::Word(w) if w == expected => Ok(true),
            ScanResult::None => Ok(false),
            other => {
                self.push_back(other);
                Ok(false)
            }
        }
    }

    fn expect_keyword(
        &mut self,
        expected: Keyword,
        alias_trie: &AliasTrie,
    ) -> Result<(), ParseError> {
        match self.scan_word(true, false, alias_trie)? {
            ScanResult::Keyword(kw, _) if kw == expected => {
                self.skip_separators()?;
                Ok(())
            }
            _ => Err(self.error(format!("expected '{}'", keyword_name(expected)))),
        }
    }

    fn expect_word(
        &mut self,
        expected: &str,
        alias_trie: &AliasTrie,
    ) -> Result<(), ParseError> {
        match self.scan_word(false, false, alias_trie)? {
            ScanResult::Word(w) if w == expected => Ok(()),
            _ => Err(self.error(format!("expected '{expected}'"))),
        }
    }

    fn consume_any_word(&mut self, alias_trie: &AliasTrie) -> Result<Option<&'src str>, ParseError> {
        match self.scan_word(false, false, alias_trie)? {
            ScanResult::Word(w) | ScanResult::Keyword(_, w) | ScanResult::Alias { raw: w, .. } => Ok(Some(w)),
            ScanResult::None => Ok(None),
        }
    }

    /// Peek at the next keyword: returns `true` and consumes it if it matches
    /// `expected`, otherwise pushes it back. Single scan, no save/restore.
    fn check_keyword(
        &mut self,
        expected: Keyword,
        alias_trie: &AliasTrie,
    ) -> Result<bool, ParseError> {
        match self.scan_word(true, false, alias_trie)? {
            ScanResult::Keyword(kw, _) if kw == expected => Ok(true),
            ScanResult::None => Ok(false),
            other => {
                self.push_back(other);
                Ok(false)
            }
        }
    }

    /// Peek at the next keyword without consuming it. Returns the keyword
    /// if present, pushing back the result either way.
    fn peek_next_keyword(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Option<Keyword>, ParseError> {
        match self.scan_word(true, false, alias_trie)? {
            ScanResult::Keyword(kw, raw) => {
                self.push_back(ScanResult::Keyword(kw, raw));
                Ok(Some(kw))
            }
            ScanResult::None => Ok(None),
            other => {
                self.push_back(other);
                Ok(None)
            }
        }
    }

    // ---- Heredoc reading ----
    //
    // POSIX: heredoc bodies appear after the newline that ends the command
    // containing `<<`.  When the parser encounters `<<DELIM`, it registers
    // a PendingHereDoc.  At the next newline (in `skip_separators` or
    // `skip_linebreaks`), `read_pending_heredocs` reads lines from the
    // main source until the delimiter is found, and stashes the body in
    // `read_heredocs`.  Later, `fill_heredoc_bodies` walks the AST to
    // attach each body to its `Redirection` node.

    fn read_pending_heredocs(&mut self) -> Result<(), ParseError> {
        while !self.pending_heredocs.is_empty() {
            let spec = self.pending_heredocs.remove(0);
            let body_line = self.line;
            let body = self.read_here_doc_body(&spec.delimiter, spec.strip_tabs)?;
            let delim = self.intern(spec.delimiter);
            self.read_heredocs.push_back(HereDoc {
                delimiter: delim,
                body,
                expand: spec.expand,
                strip_tabs: spec.strip_tabs,
                body_line,
            });
        }
        Ok(())
    }

    /// Read the body of a heredoc from the main source.
    ///
    /// Lines are consumed until one matches `delimiter` (after optional tab
    /// stripping).  Backslash-newline (`\\\n`) within a line is treated as
    /// a continuation: the physical lines are joined and the combined result
    /// is compared against the delimiter.  The raw body slice (including
    /// continuations and tabs) is returned for deferred normalization.
    fn read_here_doc_body(
        &mut self,
        delimiter: &str,
        strip_tabs: bool,
    ) -> Result<&'src str, ParseError> {
        let body_start = self.pos;
        let mut continued_line = String::new();
        let mut continuation_start: Option<usize> = None;
        loop {
            let line_start = self.pos;
            while self.pos < self.source.len() && self.source.as_bytes()[self.pos] != b'\n' {
                self.pos += 1;
            }
            let line_text = &self.source[line_start..self.pos];
            let has_newline = self.pos < self.source.len();
            if has_newline {
                self.pos += 1;
                self.line += 1;
            }

            if line_text.ends_with('\\') && has_newline {
                if continuation_start.is_none() {
                    continuation_start = Some(line_start);
                }
                continued_line.push_str(&line_text[..line_text.len() - 1]);
                continue;
            }

            let (compare_line_owned, effective_start);
            let compare = if !continued_line.is_empty() {
                continued_line.push_str(line_text);
                compare_line_owned = std::mem::take(&mut continued_line);
                effective_start = continuation_start.take().unwrap_or(line_start);
                if strip_tabs {
                    compare_line_owned.trim_start_matches('\t')
                } else {
                    &compare_line_owned
                }
            } else {
                effective_start = line_start;
                continuation_start = None;
                if strip_tabs {
                    line_text.trim_start_matches('\t')
                } else {
                    line_text
                }
            };
            if compare == delimiter {
                return Ok(&self.source[body_start..effective_start]);
            }

            if !has_newline {
                return Err(ParseError {
                    message: "unterminated here-document".into(),
                    line: Some(self.line),
                });
            }
        }
    }

    // ---- Alias expansion ----
    //
    // Alias expansion is only attempted at "command position" — the first
    // word of a simple command, or the first word after a trailing-blank
    // alias.  Recursive alias expansion is prevented by tracking which
    // names are currently being expanded in `expanding_aliases`.
    //
    // When an alias is found, its value is interned and pushed as a new
    // AliasLayer.  The parser will then read characters from that layer
    // until it's exhausted.  If the alias value ends with a blank,
    // `trailing_blank` is set so the next command-position word also
    // gets alias expansion (POSIX 2.3.1).

    /// Expand aliases at command position.  Scans one word with both keyword
    /// and alias tries active; if it matches an alias and expansion is allowed,
    /// pushes the expansion layer and loops (for chained aliases).  When the
    /// first non-alias result is reached, it is **pushed back** so the
    /// downstream consumer retrieves it without re-scanning.
    ///
    /// If `pushed_back` already contains a result, this is an O(1) no-op —
    /// the caller has already identified the first word.
    fn expand_alias_at_command_position(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<(), ParseError> {
        if self.pushed_back.is_some() {
            return Ok(());
        }
        loop {
            match self.scan_word(true, true, alias_trie)? {
                ScanResult::Alias { index, raw }
                    if is_alias_word(raw)
                        && !self.expanding_aliases.iter().any(|n| n == raw)
                        && self.alias_depth < 1024 =>
                {
                    let alias_value = alias_trie.value(index);
                    let trailing_blank = alias_has_trailing_blank(alias_value);
                    let interned = self.intern(alias_value.to_string());
                    self.expanding_aliases.push(raw.to_string());
                    self.alias_stack.push(AliasLayer {
                        text: interned,
                        pos: 0,
                        trailing_blank,
                    });
                    self.alias_depth += 1;
                    continue;
                }
                ScanResult::Alias { raw, .. } => {
                    self.push_back(ScanResult::Word(raw));
                    return Ok(());
                }
                ScanResult::None => return Ok(()),
                other => {
                    self.push_back(other);
                    return Ok(());
                }
            }
        }
    }

    // ---- Parsing methods ----
    //
    // The grammar is parsed by mutually recursive functions, each consuming
    // the portion of input they're responsible for:
    //
    //   parse_program_until  — sequence of ListItems, stopping at a keyword,
    //                          closer token, or `;;`/`;&`
    //   parse_and_or         — Pipeline (&&/|| Pipeline)*
    //   parse_pipeline       — [time [-p]] [!] Command (| Command)*
    //   parse_command        — Command with trailing redirections
    //   parse_command_inner  — dispatches keyword/subshell/group/simple
    //   parse_simple_command — assignments, words, redirections
    //   parse_if/for/case/while/until — compound commands
    //
    // All parsing methods accept an `alias_trie` reference and call
    // `expand_alias_at_command_position` at the top of each command
    // position.  The pushback guard makes redundant calls O(1) no-ops.

    /// Parse a list of commands until a stop condition is met.
    ///
    /// * `stop_kw` — returns true for keywords that end this block
    ///   (e.g. `fi`, `done`, `esac`, `elif`, `else`).
    /// * `stop_on_closer` — stop at `}` (for brace groups) or `)` (for subshells).
    /// * `stop_on_dsemi` — stop at `;;` or `;&` (for case arms).
    fn parse_program_until(
        &mut self,
        stop_kw: fn(Keyword) -> bool,
        stop_on_closer: bool,
        stop_on_dsemi: bool,
        alias_trie: &AliasTrie,
    ) -> Result<Program<'src>, ParseError> {
        let mut items = Vec::new();
        self.skip_separators()?;

        loop {
            self.expand_alias_at_command_position(alias_trie)?;

            // Single scan: stop-keyword check + first-word classification.
            // After expand_alias (which restores position), this scans the
            // first non-alias word with keyword trie active.
            match self.scan_word(true, false, alias_trie)? {
                // Stop keyword: push back so the caller can consume with
                // expect_keyword (which provides its own error message).
                ScanResult::Keyword(kw, raw) if stop_kw(kw) => {
                    self.push_back(ScanResult::Keyword(kw, raw));
                    break;
                }
                // } closer: push back so the caller can consume with expect_word
                ScanResult::Word(raw) if stop_on_closer && raw == "}" => {
                    self.push_back(ScanResult::Word(raw));
                    break;
                }
                ScanResult::None => {
                    // At EOF or delimiter byte
                    if self.at_eof() {
                        break;
                    }
                    if stop_on_closer && self.peek_byte() == Some(b')') {
                        break;
                    }
                    if stop_on_dsemi
                        && self.peek_byte() == Some(b';')
                        && matches!(
                            self.peek_byte_at_offset(1),
                            Some(b';' | b'&')
                        )
                    {
                        break;
                    }
                    // Not at a stop condition — at ( or < or > etc.
                    // Don't push back None; let downstream re-scan.
                }
                other => self.push_back(other),
            }

            let line = self.line;
            let mut and_or = self.parse_and_or(alias_trie)?;
            let asynchronous = self.consume_amp();
            self.skip_separators()?;

            if !self.read_heredocs.is_empty() {
                fill_heredoc_bodies(&mut and_or, &mut self.read_heredocs);
            }

            items.push(ListItem {
                and_or,
                asynchronous,
                line,
            });
        }

        if !self.pending_heredocs.is_empty() {
            self.read_pending_heredocs()?;
        }

        Ok(Program {
            items: items.into_boxed_slice(),
        })
    }

    fn parse_and_or(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<AndOr<'src>, ParseError> {
        let first = self.parse_pipeline(alias_trie)?;
        let mut rest = Vec::new();
        loop {
            self.skip_blanks_and_comments();
            let op = if self.peek_byte() == Some(b'&')
                && self.peek_byte_at_offset(1) == Some(b'&')
            {
                self.advance_byte();
                self.advance_byte();
                LogicalOp::And
            } else if self.peek_byte() == Some(b'|')
                && self.peek_byte_at_offset(1) == Some(b'|')
            {
                self.advance_byte();
                self.advance_byte();
                LogicalOp::Or
            } else {
                break;
            };
            self.skip_linebreaks()?;
            let rhs = self.parse_pipeline(alias_trie)?;
            rest.push((op, rhs));
        }
        Ok(AndOr {
            first,
            rest: rest.into_boxed_slice(),
        })
    }

    fn parse_pipeline(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Pipeline<'src>, ParseError> {
        self.expand_alias_at_command_position(alias_trie)?;

        let timed = if self.consume_word_if("time", alias_trie)? {
            self.expand_alias_at_command_position(alias_trie)?;
            if self.consume_word_if("-p", alias_trie)? {
                self.expand_alias_at_command_position(alias_trie)?;
                TimedMode::Posix
            } else {
                TimedMode::Default
            }
        } else {
            TimedMode::Off
        };

        let negated = if self.consume_word_if("!", alias_trie)? {
            self.expand_alias_at_command_position(alias_trie)?;
            true
        } else {
            false
        };

        let mut commands = vec![self.parse_command(alias_trie)?];
        loop {
            self.skip_blanks_and_comments();
            if self.peek_byte() == Some(b'|') && self.peek_byte_at_offset(1) != Some(b'|') {
                self.advance_byte();
                self.skip_linebreaks()?;
                commands.push(self.parse_command(alias_trie)?);
            } else {
                break;
            }
        }

        Ok(Pipeline {
            negated,
            timed,
            commands: commands.into_boxed_slice(),
        })
    }

    fn parse_command(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        self.expand_alias_at_command_position(alias_trie)?;
        let command = self.parse_command_inner(alias_trie)?;
        self.parse_command_redirections(command, alias_trie)
    }

    /// Dispatch to the correct command parser based on a single scan.
    ///
    /// After alias expansion (handled by the caller), the first word is
    /// scanned with `keyword_ok = true`.  The `ScanResult` variant
    /// determines which parser takes over:
    ///
    ///   Keyword(If/While/Until/For/Case/Function) → compound command
    ///   Word("{")                                  → brace group
    ///   Word(name) followed by `(`                 → function definition
    ///   Word(...)                                  → simple command
    ///   None + `(`                                 → subshell
    ///   None + `<`/`>`                             → redirection-only command
    fn parse_command_inner(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        let line = self.line;
        match self.scan_word(true, false, alias_trie)? {
            ScanResult::None => {
                if self.peek_byte() == Some(b'(') {
                    self.advance_byte();
                    let body =
                        self.parse_program_until(|_| false, true, false, alias_trie)?;
                    self.skip_blanks_and_comments();
                    if self.peek_byte() != Some(b')') {
                        return Err(self.error("expected ')' to close subshell"));
                    }
                    self.advance_byte();
                    return Ok(Command::Subshell(body));
                }
                // Leading redirection without command word (e.g., ">file")
                if matches!(self.peek_byte(), Some(b'<' | b'>')) {
                    return self
                        .parse_simple_command_with_first_redir(alias_trie)
                        .map(Command::Simple);
                }
                Err(self.error("expected command"))
            }
            ScanResult::Keyword(Keyword::If, _) => self.parse_if_command(alias_trie),
            ScanResult::Keyword(Keyword::While, _) => {
                self.parse_loop_command(LoopKind::While, alias_trie)
            }
            ScanResult::Keyword(Keyword::Until, _) => {
                self.parse_loop_command(LoopKind::Until, alias_trie)
            }
            ScanResult::Keyword(Keyword::For, _) => self.parse_for_command(alias_trie),
            ScanResult::Keyword(Keyword::Case, _) => self.parse_case_command(alias_trie),
            ScanResult::Keyword(Keyword::Function, _) => {
                self.parse_function_keyword(alias_trie)
            }
            ScanResult::Word(raw) | ScanResult::Keyword(_, raw) => {
                // Standalone ! at command level (not pipeline level) is an error
                if raw == "!" {
                    return Err(self.error("expected command"));
                }
                // { brace group: scan_word returns Word("{") when { is
                // followed by a delimiter
                if raw == "{" {
                    self.skip_separators()?;
                    let body =
                        self.parse_program_until(|_| false, true, false, alias_trie)?;
                    self.skip_blanks_and_comments();
                    self.expect_word("}", alias_trie)?;
                    return Ok(Command::Group(body));
                }
                // Check for name() function definition
                if is_name(raw) {
                    self.skip_blanks();
                    if self.peek_byte() == Some(b'(') {
                        self.advance_byte();
                        self.skip_blanks();
                        if self.peek_byte() == Some(b')') {
                            self.advance_byte();
                            self.skip_linebreaks().ok();
                            let body = self.parse_command(alias_trie)?;
                            return Ok(Command::FunctionDef(FunctionDef {
                                name: raw,
                                body: Box::new(body),
                            }));
                        }
                        return Err(self.error("syntax error near unexpected token `('"));
                    }
                }
                self.parse_simple_command_with_first_word(raw, line, alias_trie)
                    .map(Command::Simple)
            }
            ScanResult::Alias { .. } => unreachable!("alias_ok=false"),
        }
    }

    /// Parse a simple command when the first word has already been scanned
    /// by `parse_command_inner` (which used it for keyword/function dispatch).
    ///
    /// The first word is classified in this order:
    ///   1. All-digit + followed by `<`/`>` → IO number for a redirection
    ///   2. Contains `=` with valid name left of it → variable assignment
    ///   3. Otherwise → command word
    ///
    /// Subsequent words are scanned in a loop.  Assignments are only
    /// recognized before the first command word (POSIX 2.9.1).
    fn parse_simple_command_with_first_word(
        &mut self,
        first_raw: &'src str,
        first_line: usize,
        alias_trie: &AliasTrie,
    ) -> Result<SimpleCommand<'src>, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word<'src>> = Vec::new();
        let mut redirections = Vec::new();

        if first_raw.bytes().all(|b| b.is_ascii_digit())
            && matches!(self.peek_byte(), Some(b'<' | b'>'))
        {
            let fd = first_raw.parse::<i32>().ok();
            if let Some(mut redir) = self.try_parse_redirection(alias_trie)? {
                redir.fd = redir.fd.or(fd);
                redirections.push(redir);
            }
        } else if let Some((name, value_raw)) = split_assignment(first_raw) {
            assignments.push(Assignment {
                name,
                value: Word {
                    raw: value_raw,
                    line: first_line,
                },
            });
        } else {
            words.push(Word {
                raw: first_raw,
                line: first_line,
            });
        }

        loop {
            self.skip_blanks_and_comments();

            if self.alias_trailing_blank_pending {
                self.alias_trailing_blank_pending = false;
                self.expand_alias_at_command_position(alias_trie)?;
            }

            if let Some(redir) = self.try_parse_redirection(alias_trie)? {
                redirections.push(redir);
                continue;
            }

            // Try assignment (only before any command words)
            if words.is_empty() {
                if !assignments.is_empty() || !redirections.is_empty() {
                    self.expand_alias_at_command_position(alias_trie)?;
                }
                let line = self.line;
                match self.scan_word(false, false, alias_trie)? {
                    ScanResult::Word(raw) if !raw.is_empty() => {
                        if let Some((name, value_raw)) = split_assignment(raw) {
                            assignments.push(Assignment {
                                name,
                                value: Word {
                                    raw: value_raw,
                                    line,
                                },
                            });
                            continue;
                        }
                        words.push(Word { raw, line });
                        continue;
                    }
                    ScanResult::None => {}
                    other => {
                        self.push_back(other);
                    }
                }
            }

            let line = self.line;
            match self.scan_word(false, false, alias_trie)? {
                ScanResult::Word(raw) if !raw.is_empty() => {
                    words.push(Word { raw, line });
                    continue;
                }
                _ => break,
            }
        }

        if words.is_empty() && assignments.is_empty() && redirections.is_empty() {
            return Err(self.error("expected command"));
        }

        if !words.is_empty() && self.peek_byte() == Some(b'(') {
            return Err(self.error("syntax error near unexpected token `('"));
        }

        Ok(SimpleCommand {
            assignments: assignments.into_boxed_slice(),
            words: words.into_boxed_slice(),
            redirections: redirections.into_boxed_slice(),
        })
    }

    /// Parse a simple command when the first token is a redirection (no leading word).
    fn parse_simple_command_with_first_redir(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<SimpleCommand<'src>, ParseError> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word<'src>> = Vec::new();
        let mut redirections = Vec::new();

        if let Some(redir) = self.try_parse_redirection(alias_trie)? {
            redirections.push(redir);
        }

        loop {
            self.skip_blanks_and_comments();
            if let Some(redir) = self.try_parse_redirection(alias_trie)? {
                redirections.push(redir);
                continue;
            }
            if words.is_empty() {
                if !assignments.is_empty() || !redirections.is_empty() {
                    self.expand_alias_at_command_position(alias_trie)?;
                }
                let line = self.line;
                match self.scan_word(false, false, alias_trie)? {
                    ScanResult::Word(raw) if !raw.is_empty() => {
                        if let Some((name, value_raw)) = split_assignment(raw) {
                            assignments.push(Assignment {
                                name,
                                value: Word {
                                    raw: value_raw,
                                    line,
                                },
                            });
                            continue;
                        }
                        words.push(Word { raw, line });
                        continue;
                    }
                    ScanResult::None => {}
                    other => {
                        self.push_back(other);
                    }
                }
            }
            let line = self.line;
            match self.scan_word(false, false, alias_trie)? {
                ScanResult::Word(raw) if !raw.is_empty() => {
                    words.push(Word { raw, line });
                    continue;
                }
                _ => break,
            }
        }

        if words.is_empty() && assignments.is_empty() && redirections.is_empty() {
            return Err(self.error("expected command"));
        }

        Ok(SimpleCommand {
            assignments: assignments.into_boxed_slice(),
            words: words.into_boxed_slice(),
            redirections: redirections.into_boxed_slice(),
        })
    }

    /// Try to parse a redirection at the current position.  Returns `None`
    /// if the current bytes aren't `<` or `>` (possibly preceded by digits).
    ///
    /// For heredocs (`<<`), the delimiter word is parsed but the body is
    /// deferred to `read_pending_heredocs` at the next newline.
    fn try_parse_redirection(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Option<Redirection<'src>>, ParseError> {
        // IO number: scan a bounded run of digits before `<` or `>`.
        // This is O(few digits), not full word rescanning.
        let mut fd: Option<i32> = None;
        let digit_start = self.pos;
        if !self.in_alias() {
            while self.pos < self.source.len()
                && self.source.as_bytes()[self.pos].is_ascii_digit()
            {
                self.pos += 1;
            }
            if self.pos > digit_start && matches!(self.peek_byte(), Some(b'<' | b'>')) {
                let num_text = &self.source[digit_start..self.pos];
                fd = num_text.parse::<i32>().ok();
                if fd.is_none() {
                    self.pos = digit_start;
                }
            } else {
                self.pos = digit_start;
            }
        }

        let (kind, strip_tabs) = match self.peek_byte() {
            Some(b'<') => {
                self.advance_byte();
                match self.peek_byte() {
                    Some(b'<') => {
                        self.advance_byte();
                        if self.peek_byte() == Some(b'-') {
                            self.advance_byte();
                            (RedirectionKind::HereDoc, true)
                        } else {
                            (RedirectionKind::HereDoc, false)
                        }
                    }
                    Some(b'&') => {
                        self.advance_byte();
                        (RedirectionKind::DupInput, false)
                    }
                    Some(b'>') => {
                        self.advance_byte();
                        (RedirectionKind::ReadWrite, false)
                    }
                    _ => (RedirectionKind::Read, false),
                }
            }
            Some(b'>') => {
                self.advance_byte();
                match self.peek_byte() {
                    Some(b'>') => {
                        self.advance_byte();
                        (RedirectionKind::Append, false)
                    }
                    Some(b'&') => {
                        self.advance_byte();
                        (RedirectionKind::DupOutput, false)
                    }
                    Some(b'|') => {
                        self.advance_byte();
                        (RedirectionKind::ClobberWrite, false)
                    }
                    _ => (RedirectionKind::Write, false),
                }
            }
            _ => {
                self.pos = digit_start;
                return Ok(None);
            }
        };

        self.skip_blanks();
        let line = self.line;
        let target_raw = match self.scan_word(false, false, alias_trie)? {
            ScanResult::Word(w) => w,
            _ => return Err(self.error("expected redirection target")),
        };

        let target = Word {
            raw: target_raw,
            line,
        };

        let here_doc = if kind == RedirectionKind::HereDoc {
            let (unquoted_delim, expand) = parse_here_doc_delimiter(target.raw);
            self.pending_heredocs.push(PendingHereDoc {
                delimiter: unquoted_delim,
                strip_tabs,
                expand,
            });
            None
        } else {
            None
        };

        Ok(Some(Redirection {
            fd,
            kind,
            target,
            here_doc,
        }))
    }

    /// Collect trailing redirections after a compound command and wrap it
    /// in `Command::Redirected`.  Simple commands handle their own
    /// redirections inline, so this is a no-op for them.
    fn parse_command_redirections(
        &mut self,
        command: Command<'src>,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        if matches!(command, Command::Simple(_)) {
            return Ok(command);
        }
        let mut redirections = Vec::new();
        while let Some(redir) = self.try_parse_redirection(alias_trie)? {
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

    // ---- Compound commands ----

    fn parse_if_command(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        let condition = self.parse_program_until(
            |kw| matches!(kw, Keyword::Then),
            false,
            false,
            alias_trie,
        )?;
        if condition.items.is_empty() {
            return Err(self.error("expected command list after 'if'"));
        }
        self.expect_keyword(Keyword::Then, alias_trie)?;

        fn at_elif_else_fi(kw: Keyword) -> bool {
            matches!(kw, Keyword::Elif | Keyword::Else | Keyword::Fi)
        }
        let then_branch =
            self.parse_program_until(at_elif_else_fi, false, false, alias_trie)?;
        let mut elif_branches = Vec::new();

        while self.check_keyword(Keyword::Elif, alias_trie)? {
            self.skip_separators()?;
            let cond = self.parse_program_until(
                |kw| matches!(kw, Keyword::Then),
                false,
                false,
                alias_trie,
            )?;
            if cond.items.is_empty() {
                return Err(self.error("expected command list after 'elif'"));
            }
            self.expect_keyword(Keyword::Then, alias_trie)?;
            let body =
                self.parse_program_until(at_elif_else_fi, false, false, alias_trie)?;
            elif_branches.push(ElifBranch {
                condition: cond,
                body,
            });
        }

        let else_branch = if self.check_keyword(Keyword::Else, alias_trie)? {
            self.skip_separators()?;
            Some(self.parse_program_until(
                |kw| matches!(kw, Keyword::Fi),
                false,
                false,
                alias_trie,
            )?)
        } else {
            None
        };

        self.expect_keyword(Keyword::Fi, alias_trie)?;
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
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        let keyword = match kind {
            LoopKind::While => "while",
            LoopKind::Until => "until",
        };
        let condition = self.parse_program_until(
            |kw| matches!(kw, Keyword::Do),
            false,
            false,
            alias_trie,
        )?;
        if condition.items.is_empty() {
            return Err(self.error(format!("expected command list after '{keyword}'")));
        }
        self.expect_keyword(Keyword::Do, alias_trie)?;
        let body = self.parse_program_until(
            |kw| matches!(kw, Keyword::Done),
            false,
            false,
            alias_trie,
        )?;
        self.expect_keyword(Keyword::Done, alias_trie)?;
        Ok(Command::Loop(LoopCommand {
            kind,
            condition,
            body,
        }))
    }

    fn parse_for_command(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        self.skip_blanks_and_comments();
        let name = match self.scan_word(false, false, alias_trie)? {
            ScanResult::Word(w) => w,
            _ => return Err(self.error("expected for loop variable name")),
        };
        if !is_name(name) {
            return Err(self.error("expected for loop variable name"));
        }

        self.skip_linebreaks()?;
        let items = if self.check_keyword(Keyword::In, alias_trie)? {
            let mut items = Vec::new();
            loop {
                self.skip_blanks_and_comments();
                if self.at_eof()
                    || matches!(self.peek_byte(), Some(b'\n' | b';'))
                {
                    break;
                }
                let line = self.line;
                match self.scan_word(false, false, alias_trie)? {
                    ScanResult::Word(w) => items.push(Word { raw: w, line }),
                    other => {
                        self.push_back(other);
                        break;
                    }
                }
            }
            Some(items.into_boxed_slice())
        } else {
            None
        };

        self.skip_separators()?;
        self.expect_keyword(Keyword::Do, alias_trie)?;
        let body = self.parse_program_until(
            |kw| matches!(kw, Keyword::Done),
            false,
            false,
            alias_trie,
        )?;
        self.expect_keyword(Keyword::Done, alias_trie)?;
        Ok(Command::For(ForCommand { name, items, body }))
    }

    fn parse_case_command(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        self.skip_blanks_and_comments();
        let line = self.line;
        let word_raw = self
            .consume_any_word(alias_trie)?
            .ok_or_else(|| self.error("expected case word"))?;
        let word = Word {
            raw: word_raw,
            line,
        };

        self.skip_linebreaks()?;
        if !self.check_keyword(Keyword::In, alias_trie)? {
            return Err(self.error("expected 'in'"));
        }
        self.skip_linebreaks()?;

        let mut arms = Vec::new();
        loop {
            if self.peek_next_keyword(alias_trie)? == Some(Keyword::Esac) || self.at_eof() {
                break;
            }
            self.skip_blanks_and_comments();
            if self.peek_byte() == Some(b'(') {
                self.advance_byte();
            }

            let mut patterns = Vec::new();
            loop {
                self.skip_blanks_and_comments();
                let pat_line = self.line;
                let pat = self
                    .consume_any_word(alias_trie)?
                    .ok_or_else(|| self.error("expected case pattern"))?;
                patterns.push(Word {
                    raw: pat,
                    line: pat_line,
                });

                self.skip_blanks_and_comments();
                if self.peek_byte() == Some(b'|')
                    && self.peek_byte_at_offset(1) != Some(b'|')
                {
                    self.advance_byte();
                    continue;
                }
                break;
            }

            self.skip_blanks_and_comments();
            if self.peek_byte() != Some(b')') {
                return Err(self.error("expected ')' after case pattern"));
            }
            self.advance_byte();
            self.skip_separators()?;

            let body = self.parse_program_until(
                |kw| matches!(kw, Keyword::Esac),
                false,
                true,
                alias_trie,
            )?;

            let fallthrough = self.peek_byte() == Some(b';')
                && self.peek_byte_at_offset(1) == Some(b'&');

            arms.push(CaseArm {
                patterns: patterns.into_boxed_slice(),
                body,
                fallthrough,
            });

            self.skip_blanks_and_comments();
            if self.peek_byte() == Some(b';') {
                if self.peek_byte_at_offset(1) == Some(b';') {
                    self.advance_byte();
                    self.advance_byte();
                    self.skip_separators()?;
                } else if self.peek_byte_at_offset(1) == Some(b'&') {
                    self.advance_byte();
                    self.advance_byte();
                    self.skip_separators()?;
                } else if self.peek_next_keyword(alias_trie)? != Some(Keyword::Esac) {
                    return Err(self.error("expected ';;', ';&', or 'esac'"));
                }
            } else if self.peek_next_keyword(alias_trie)? != Some(Keyword::Esac) {
                break;
            }
        }

        self.expect_keyword(Keyword::Esac, alias_trie)?;
        Ok(Command::Case(CaseCommand {
            word,
            arms: arms.into_boxed_slice(),
        }))
    }

    fn parse_function_keyword(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Command<'src>, ParseError> {
        self.skip_blanks_and_comments();
        let name = match self.scan_word(false, false, alias_trie)? {
            ScanResult::Word(w) => w,
            _ => return Err(self.error("expected function name")),
        };
        if !is_name(name) {
            return Err(self.error("expected function name"));
        }
        self.skip_blanks();
        if self.peek_byte() == Some(b'(') {
            self.advance_byte();
            self.skip_blanks();
            if self.peek_byte() == Some(b')') {
                self.advance_byte();
            }
        }
        self.skip_linebreaks().ok();
        let body = self.parse_command(alias_trie)?;
        Ok(Command::FunctionDef(FunctionDef {
            name,
            body: Box::new(body),
        }))
    }

    // ---- Public API (on Parser) ----
    //
    // Two granularities are exposed:
    //
    //   `next_list_item`        — yields one pipeline at a time (asynchronous
    //                             flag included).  This is the finer granularity
    //                             used when `complete_command_parsing` is off.
    //
    //   `next_complete_command` — yields an entire newline-terminated command
    //                             list at once.  This is what POSIX strictly
    //                             requires (one complete command per execution
    //                             cycle) and what most other shells do.

    /// Return the next pipeline (possibly asynchronous) from the input, or
    /// `None` at EOF.
    pub fn next_list_item(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Option<ListItem<'src>>, ParseError> {
        self.skip_separators()?;
        self.expand_alias_at_command_position(alias_trie)?;
        if self.at_eof() {
            return Ok(None);
        }
        let line = self.line;
        let mut and_or = self.parse_and_or(alias_trie)?;
        let asynchronous = self.consume_amp();
        self.skip_separators()?;
        if !self.read_heredocs.is_empty() {
            fill_heredoc_bodies(&mut and_or, &mut self.read_heredocs);
        }
        Ok(Some(ListItem {
            and_or,
            asynchronous,
            line,
        }))
    }

    /// Return the next complete command (everything up to the next unquoted
    /// newline), or `None` at EOF.
    pub fn next_complete_command(
        &mut self,
        alias_trie: &AliasTrie,
    ) -> Result<Option<Program<'src>>, ParseError> {
        self.skip_separators()?;
        if self.at_eof() {
            return Ok(None);
        }
        let mut items = Vec::new();
        loop {
            self.expand_alias_at_command_position(alias_trie)?;
            if self.at_eof() {
                break;
            }
            let line = self.line;
            let mut and_or = self.parse_and_or(alias_trie)?;
            let asynchronous = self.consume_amp();

            self.skip_blanks_and_comments();
            let at_newline = self.peek_byte() == Some(b'\n');
            if self.peek_byte() == Some(b';') {
                self.advance_byte();
            }
            if at_newline {
                self.advance_byte();
                self.read_pending_heredocs()?;
            }

            if !self.read_heredocs.is_empty() {
                fill_heredoc_bodies(&mut and_or, &mut self.read_heredocs);
            }

            items.push(ListItem {
                and_or,
                asynchronous,
                line,
            });

            if at_newline || self.at_eof() {
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

// ---- Heredoc body fill-in ----
//
// Heredoc bodies are read from the source at newline boundaries (after
// the command line that contained `<<`), and collected into `read_heredocs`.
// These functions walk the freshly-built AST in source order and pop
// bodies from the front of the queue, attaching each one to the matching
// `Redirection` node whose `here_doc` field is still `None`.

fn fill_heredoc_bodies<'src>(and_or: &mut AndOr<'src>, bodies: &mut VecDeque<HereDoc<'src>>) {
    fill_pipeline_hd(&mut and_or.first, bodies);
    for (_, pl) in and_or.rest.iter_mut() {
        fill_pipeline_hd(pl, bodies);
    }
}

fn fill_pipeline_hd<'src>(pipeline: &mut Pipeline<'src>, bodies: &mut VecDeque<HereDoc<'src>>) {
    for cmd in pipeline.commands.iter_mut() {
        fill_command_hd(cmd, bodies);
    }
}

fn fill_command_hd<'src>(cmd: &mut Command<'src>, bodies: &mut VecDeque<HereDoc<'src>>) {
    match cmd {
        Command::Simple(sc) => fill_redirs_hd(&mut sc.redirections, bodies),
        Command::Redirected(inner, redirs) => {
            fill_command_hd(inner, bodies);
            fill_redirs_hd(redirs, bodies);
        }
        Command::If(c) => {
            fill_program_hd(&mut c.condition, bodies);
            fill_program_hd(&mut c.then_branch, bodies);
            for b in c.elif_branches.iter_mut() {
                fill_program_hd(&mut b.condition, bodies);
                fill_program_hd(&mut b.body, bodies);
            }
            if let Some(ref mut e) = c.else_branch {
                fill_program_hd(e, bodies);
            }
        }
        Command::Loop(c) => {
            fill_program_hd(&mut c.condition, bodies);
            fill_program_hd(&mut c.body, bodies);
        }
        Command::For(c) => {
            fill_program_hd(&mut c.body, bodies);
        }
        Command::Case(c) => {
            for arm in c.arms.iter_mut() {
                fill_program_hd(&mut arm.body, bodies);
            }
        }
        Command::Subshell(p) | Command::Group(p) => fill_program_hd(p, bodies),
        Command::FunctionDef(f) => fill_command_hd(&mut f.body, bodies),
    }
}

fn fill_program_hd<'src>(program: &mut Program<'src>, bodies: &mut VecDeque<HereDoc<'src>>) {
    for item in program.items.iter_mut() {
        fill_heredoc_bodies(&mut item.and_or, bodies);
    }
}

fn fill_redirs_hd<'src>(
    redirs: &mut Box<[Redirection<'src>]>,
    bodies: &mut VecDeque<HereDoc<'src>>,
) {
    for r in redirs.iter_mut() {
        if r.kind == RedirectionKind::HereDoc && r.here_doc.is_none() {
            r.here_doc = bodies.pop_front();
        }
    }
}

// ============================================================
// Public API — entry points for callers
// ============================================================

/// Parse the entire source as a single program (batch mode).
pub fn parse(source: &str) -> Result<Program<'_>, ParseError> {
    parse_with_aliases(source, &HashMap::new())
}

/// Parse with alias expansion.  Builds an `AliasTrie` from the map and
/// runs a single `parse_program_until` over the entire source.
pub fn parse_with_aliases<'src>(
    source: &'src str,
    aliases: &HashMap<String, String>,
) -> Result<Program<'src>, ParseError> {
    let alias_trie = AliasTrie::build(aliases);
    let mut parser = Parser::new(source);
    parser.parse_program_until(|_| false, false, false, &alias_trie)
}

/// Incremental parsing session — wraps a `Parser` for callers that
/// want to pull one list-item or one complete-command at a time
/// (used by `Shell::execute_source`).
pub struct ParseSession<'src> {
    parser: Parser<'src>,
}

impl<'src> ParseSession<'src> {
    pub fn new(source: &'src str) -> Result<Self, ParseError> {
        Ok(Self {
            parser: Parser::new(source),
        })
    }

    pub fn next_item(
        &mut self,
        aliases: &HashMap<String, String>,
    ) -> Result<Option<ListItem<'src>>, ParseError> {
        let alias_trie = AliasTrie::build(aliases);
        self.parser.next_list_item(&alias_trie)
    }

    pub fn current_line(&self) -> usize {
        self.parser.current_line()
    }
}

// ============================================================
// Utility functions
// ============================================================

/// Split `NAME=VALUE` into `(NAME, VALUE)`.  Returns `None` if the
/// left side is not a valid shell identifier or there is no `=`.
fn split_assignment(input: &str) -> Option<(&str, &str)> {
    let (name, value) = input.split_once('=')?;
    if !is_name(name) {
        return None;
    }
    Some((name, value))
}

/// Strip quoting from a heredoc delimiter word and determine whether the
/// body should undergo parameter/command expansion.
/// Any quoting character (`'`, `"`, `\`) in the delimiter suppresses
/// expansion of the body (POSIX 2.7.4).
fn parse_here_doc_delimiter(raw: &str) -> (String, bool) {
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
                    if bytes[index] == b'"' {
                        index += 1;
                        break;
                    }
                    delimiter.push(bytes[index] as char);
                    index += 1;
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

    (delimiter, expand)
}

/// Check whether `name` is a valid POSIX shell identifier:
/// starts with `[A-Za-z_]`, followed by `[A-Za-z0-9_]*`.
pub fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    !chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test(source: &str) -> Result<Program<'_>, ParseError> {
        parse(source)
    }

    fn parse_with_aliases_test<'src>(
        source: &'src str,
        aliases: &HashMap<String, String>,
    ) -> Result<Program<'src>, ParseError> {
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
            Command::Simple(cmd) if cmd.assignments.len() == 1 && cmd.words[0].raw == "echo"
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
            Command::Simple(cmd) if cmd.words.len() == 2 && cmd.words[1].raw == "'ok'"
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
                    && cmd.redirections[0].target.raw == "EOF"
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body) == Some("hello $USER\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse_test("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.delimiter) == Some("EOF")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse_test("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| doc.body) == Some("\tone\n")
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
                    && redirections[0].target.raw == "out"
        ));

        let not_a_group = parse_test("{echo hi; }").expect("parse brace word");
        assert!(matches!(
            &not_a_group.items[0].and_or.first.commands[0],
            Command::Simple(simple) if simple.words[0].raw == "{echo"
        ));

        let closer_literal = parse_test("echo }").expect("parse literal closer");
        assert!(matches!(
            &closer_literal.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["echo", "}"]
        ));
    }

    #[test]
    fn parses_function_definition() {
        let program = parse_test("greet() { echo hi; }").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(function) if function.name == "greet"
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
                if for_command.name == "item" && for_command.items.as_ref().map(|s| s.len()) == Some(3)
        ));

        let positional = parse_test("for item; do echo $item; done").expect("parse");
        assert!(matches!(
            &positional.items[0].and_or.first.commands[0],
            Command::For(for_command) if for_command.name == "item" && for_command.items.is_none()
        ));

        let linebreak_before_in =
            parse_test("for item\nin a b; do echo $item; done").expect("parse linebreak before in");
        assert!(matches!(
            &linebreak_before_in.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.name == "item"
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw).collect::<Vec<_>>())
                        == Some(vec!["a", "b"])
        ));

        let reserved_words_as_items = parse_test("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| word.raw).collect::<Vec<_>>())
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
                if case_command.word.raw == "$name"
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

        let ft_static = fallthrough.items[0].and_or.first.commands[0]
            .clone()
            .into_static();
        assert!(matches!(ft_static, Command::Case(ref cc) if cc.arms[0].fallthrough));
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
    fn parse_session_uses_updated_aliases_between_items() {
        let mut session =
            ParseSession::new("alias setok='printf ok'; setok").expect("session");
        let first = session
            .next_item(&HashMap::new())
            .expect("first item")
            .expect("some item");
        assert!(matches!(first.and_or.first.commands[0], Command::Simple(_)));

        let second = session
            .next_item(&HashMap::from([(
                String::from("setok"),
                String::from("printf ok"),
            )]))
            .expect("second item")
            .expect("some item");
        assert!(matches!(
            &second.and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["printf", "ok"]
        ));

        assert!(session.next_item(&HashMap::new()).expect("eof").is_none());
    }

    #[test]
    fn alias_expansion_in_simple_commands() {
        let mut aliases = HashMap::new();
        aliases.insert("say".to_string(), "printf hi".to_string());
        let program = parse_with_aliases_test("say", &aliases).expect("parse alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["printf", "hi"]
        ));

        let mut aliases = HashMap::new();
        aliases.insert("cond".to_string(), "if".to_string());
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
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["echo", "!"]
        ));

        let program = parse_test("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["!true"]
        ));

        let program = parse_test("! true").expect("parse negation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn trailing_blank_aliases_expand_next_simple_command_word() {
        let mut aliases = HashMap::new();
        aliases.insert("say".to_string(), "printf %s ".to_string());
        aliases.insert("word".to_string(), "ok".to_string());
        let program = parse_with_aliases_test("say word", &aliases).expect("parse chained alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["printf", "%s", "ok"]
        ));
    }

    #[test]
    fn self_referential_aliases_do_not_loop_indefinitely() {
        let mut aliases = HashMap::new();
        aliases.insert("loop".to_string(), "loop ".to_string());
        let program = parse_with_aliases_test("loop ok", &aliases).expect("self alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| word.raw).collect::<Vec<_>>() == vec!["loop", "ok"]
        ));
        assert!(alias_has_trailing_blank("value "));
        assert!(!alias_has_trailing_blank("value"));
    }

    #[test]
    fn alias_expansion_after_assignment_and_redirection() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".to_string(), "echo aliased".to_string());
        let program =
            parse_with_aliases_test("VAR=value foo", &aliases).expect("alias after assignment");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.assignments.len() == 1
        ));

        let program =
            parse_with_aliases_test("</dev/null foo", &aliases).expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.redirections.len() == 1
        ));
    }

    #[test]
    fn lparen_after_simple_command_is_syntax_error() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".to_string(), "echo aliased".to_string());
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
                if cmd.words.len() == 2 && cmd.words[1].raw == "$(cmd arg)"
        ));
    }

    #[test]
    fn tokenizer_keeps_dollar_double_paren_as_single_word() {
        let program = parse_test("echo $((1 + 2))").expect("parse dollar arith");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "$((1 + 2))"
        ));

        let nested = parse_test("echo $((1 + (2 * 3)))").expect("parse nested arith");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words[1].raw == "$((1 + (2 * 3)))"
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
                if cmd.words.len() == 2 && cmd.words[1].raw == "${VAR:-default}"
        ));

        let nested = parse_test("echo ${VAR:-${INNER}}").expect("parse nested brace");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-${INNER}}"
        ));
    }

    #[test]
    fn tokenizer_keeps_backtick_as_single_word() {
        let program = parse_test("echo `cmd arg`").expect("parse backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && cmd.words[1].raw == "`cmd arg`"
        ));

        let error = parse_test("echo `unterminated").expect_err("unterminated backtick");
        assert_eq!(&*error.message, "unterminated backquote");
    }

    #[test]
    fn tokenizer_nested_constructs_in_brace_body() {
        let program = parse_test("echo ${VAR:-'a}b'}").expect("parse brace sq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-'a}b'}"
        ));

        let program = parse_test("echo ${VAR:-\"a}b\"}").expect("parse brace dq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-\"a}b\"}"
        ));

        let program = parse_test("echo ${VAR:-\\}}").expect("parse brace escaped");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "${VAR:-\\}}"
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
            Command::Simple(cmd) if cmd.words.len() == 2 && cmd.words[1].raw == "hello"
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
            matches!(&pipeline.commands[0], Command::Simple(cmd) if cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn time_posix_pipeline() {
        let program = parse_test("time -p echo hello").expect("parse time -p");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Posix);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn function_keyword_basic() {
        let program = parse_test("function foo { echo hi; }").expect("parse function keyword");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_with_parens() {
        let program =
            parse_test("function foo() { echo hi; }").expect("parse function keyword parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_invalid_name() {
        let error = parse_test("function 123").expect_err("bad function name");
        assert_eq!(&*error.message, "expected function name");
    }

    #[test]
    fn into_static_covers_all_command_variants() {
        let simple = Command::Simple(SimpleCommand {
            assignments: vec![Assignment {
                name: "X",
                value: Word { raw: "1", line: 0 },
            }]
            .into_boxed_slice(),
            words: vec![Word { raw: "echo", line: 0 }].into_boxed_slice(),
            redirections: vec![Redirection {
                fd: Some(2),
                kind: RedirectionKind::Write,
                target: Word { raw: "err", line: 0 },
                here_doc: None,
            }]
            .into_boxed_slice(),
        });
        let s: Command<'static> = simple.into_static();
        assert!(matches!(s, Command::Simple(ref sc) if sc.words[0].raw == "echo"));

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
        assert!(matches!(subshell.clone().into_static(), Command::Subshell(_)));

        let group = Command::Group(Program { items: vec![].into_boxed_slice() });
        assert!(matches!(group.into_static(), Command::Group(_)));

        let func = Command::FunctionDef(FunctionDef {
            name: "f",
            body: Box::new(s.clone()),
        });
        assert!(matches!(func.into_static(), Command::FunctionDef(fd) if fd.name == "f"));

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
        assert!(matches!(if_cmd.into_static(), Command::If(_)));

        let loop_cmd = Command::Loop(LoopCommand {
            kind: LoopKind::While,
            condition: Program { items: vec![].into_boxed_slice() },
            body: Program { items: vec![].into_boxed_slice() },
        });
        assert!(matches!(loop_cmd.into_static(), Command::Loop(_)));

        let for_cmd = Command::For(ForCommand {
            name: "i",
            items: Some(vec![Word { raw: "a", line: 0 }].into_boxed_slice()),
            body: Program { items: vec![].into_boxed_slice() },
        });
        assert!(matches!(&for_cmd.into_static(), Command::For(fc) if fc.name == "i"));

        let case_cmd = Command::Case(CaseCommand {
            word: Word { raw: "x", line: 0 },
            arms: vec![CaseArm {
                patterns: vec![Word { raw: "*", line: 0 }].into_boxed_slice(),
                body: Program { items: vec![].into_boxed_slice() },
                fallthrough: false,
            }]
            .into_boxed_slice(),
        });
        assert!(matches!(case_cmd.into_static(), Command::Case(_)));

        let redir = Command::Redirected(
            Box::new(s.clone()),
            vec![Redirection {
                fd: None,
                kind: RedirectionKind::Write,
                target: Word { raw: "out", line: 0 },
                here_doc: Some(HereDoc {
                    delimiter: "EOF",
                    body: "test\n",
                    expand: true,
                    strip_tabs: false,
                    body_line: 0,
                }),
            }]
            .into_boxed_slice(),
        );
        assert!(matches!(redir.into_static(), Command::Redirected(_, _)));
    }

    #[test]
    fn alias_expansion_produces_non_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert("both".to_string(), "echo a; echo b".to_string());
        let program =
            parse_with_aliases_test("both", &aliases).expect("parse alias with semicolon");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn alias_expansion_interns_reserved_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "myif".to_string(),
            "if true; then echo ok; elif false; then echo no; else echo fb; fi".to_string(),
        );
        let program =
            parse_with_aliases_test("myif", &aliases).expect("alias if/then/elif/else/fi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "mywhile".to_string(),
            "while false; do echo loop; done".to_string(),
        );
        let program = parse_with_aliases_test("mywhile", &aliases).expect("alias while/do/done");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myuntil".to_string(),
            "until true; do echo u; done".to_string(),
        );
        let program = parse_with_aliases_test("myuntil", &aliases).expect("alias until");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myfor".to_string(),
            "for x in a b; do echo $x; done".to_string(),
        );
        let program = parse_with_aliases_test("myfor", &aliases).expect("alias for/in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "mycase".to_string(),
            "case x in a) echo a;; esac".to_string(),
        );
        let program = parse_with_aliases_test("mycase", &aliases).expect("alias case/esac");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert(
            "myfn".to_string(),
            "function myfunc { echo hi; }".to_string(),
        );
        let program = parse_with_aliases_test("myfn", &aliases).expect("alias function/{/}");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myneg".to_string(), "! true".to_string());
        let program = parse_with_aliases_test("myneg", &aliases).expect("alias bang");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn backslash_newline_mid_word_produces_stripped_form() {
        let program = parse_test("ec\\\nho ok").expect("continuation in command");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "echo" && cmd.words[1].raw == "ok"
        ));

        let program = parse_test("echo a\\\nb\\\nc").expect("multiple continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[1].raw == "abc"
        ));
    }

    #[test]
    fn backslash_newline_before_comment_does_not_start_comment() {
        let program = parse_test("a\\\n#b").expect("continuation before hash");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words[0].raw == "a#b"
        ));
    }

    #[test]
    fn backslash_newline_before_operator_resets() {
        let program = parse_test("echo a\\\nb; echo c").expect("continuation before semi");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn heredoc_delimiter_helpers() {
        assert_eq!(parse_here_doc_delimiter("\"EOF\""), ("EOF".into(), false));
        assert_eq!(parse_here_doc_delimiter("\\EOF"), ("EOF".into(), false));
    }

    #[test]
    fn skip_scan_covers_dollar_single_quote_and_default_in_subshell() {
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
    fn alias_trie_basic() {
        let mut aliases = HashMap::new();
        aliases.insert("ls".to_string(), "ls --color".to_string());
        aliases.insert("ll".to_string(), "ls -la".to_string());
        let trie = AliasTrie::build(&aliases);

        let mut state = trie.root();
        for &b in b"ls" {
            state = trie.step(state, b);
        }
        assert!(trie.terminal(state).is_some());
        assert_eq!(trie.value(trie.terminal(state).unwrap()), "ls --color");

        let mut state = trie.root();
        for &b in b"xyz" {
            state = trie.step(state, b);
        }
        assert_eq!(state, AliasTrie::NONE);
    }
}
