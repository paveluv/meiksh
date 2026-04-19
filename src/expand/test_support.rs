use std::borrow::Cow;
use std::rc::Rc;

use crate::bstr;
use crate::expand::core::{Context, ExpandError};
use crate::expand::expand_parts::ExpandOutput;
use crate::expand::model::Expansion;
use crate::expand::word::{
    ensure_ifs_cached, expand_word_into, expand_word_with_scratch, expand_words_into, with_scratch,
};
use crate::hash::ShellMap;
use crate::syntax::ast::{Command, Program, Word};

pub(super) struct FakeContext {
    pub(super) env: ShellMap<Vec<u8>, Vec<u8>>,
    pub(super) positional: Vec<Vec<u8>>,
    pub(super) pathname_expansion_enabled: bool,
    pub(super) nounset_enabled: bool,
    pub(super) scratch: crate::expand::scratch::ExpandScratch,
}

impl FakeContext {
    pub(super) fn new() -> Self {
        let mut env = ShellMap::default();
        env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        env.insert(b"USER".to_vec(), b"meiksh".to_vec());
        env.insert(b"IFS".to_vec(), b" \t\n,".to_vec());
        env.insert(b"WORDS".to_vec(), b"one,two three".to_vec());
        env.insert(b"DELIMS".to_vec(), b",,,".to_vec());
        env.insert(b"EMPTY".to_vec(), Vec::new());
        env.insert(b"X".to_vec(), b"fallback".to_vec());
        Self {
            env,
            positional: vec![b"alpha".to_vec(), b"beta".to_vec()],
            pathname_expansion_enabled: true,
            nounset_enabled: false,
            scratch: crate::expand::scratch::ExpandScratch::new(),
        }
    }
}

impl Context for FakeContext {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.env.get(name).map(|v| Cow::Borrowed(v.as_slice()))
    }

    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>> {
        match name {
            b'?' => Some(Cow::Owned(b"0".to_vec())),
            b'#' => Some(Cow::Owned(bstr::u64_to_bytes(self.positional.len() as u64))),
            b'-' => Some(Cow::Owned(b"aC".to_vec())),
            b'*' | b'@' => Some(Cow::Owned(bstr::join_bstrings(&self.positional, b" "))),
            _ => None,
        }
    }

    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
        if index == 0 {
            Some(Cow::Owned(b"meiksh".to_vec()))
        } else {
            self.positional
                .get(index - 1)
                .map(|v| Cow::Borrowed(v.as_slice()))
        }
    }

    fn positional_params(&self) -> &[Vec<u8>] {
        &self.positional
    }

    fn set_var(&mut self, name: &[u8], value: &[u8]) -> Result<(), ExpandError> {
        if name == b"IFS" {
            self.scratch.invalidate_ifs();
        }
        self.env.insert(name.to_vec(), value.to_vec());
        Ok(())
    }

    fn pathname_expansion_enabled(&self) -> bool {
        self.pathname_expansion_enabled
    }

    fn nounset_enabled(&self) -> bool {
        self.nounset_enabled
    }

    fn shell_name(&self) -> &[u8] {
        b"meiksh"
    }

    fn command_substitute(&mut self, program: &Rc<Program>) -> Result<Vec<u8>, ExpandError> {
        // Echo the command back, matching `command_substitute_raw`'s shape so
        // tests driven through either the production (parser -> Program) or
        // the pattern/redirect paths (raw bytes) observe the same byte stream.
        let mut out = Vec::new();
        crate::exec::render::render_program_into(program, &mut out);
        out.push(b'\n');
        Ok(out)
    }

    fn command_substitute_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let mut out = command.to_vec();
        out.push(b'\n');
        Ok(out)
    }

    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        if name == b"testuser" {
            Some(Cow::Owned(b"/home/testuser".to_vec()))
        } else if name == b"slashuser" {
            Some(Cow::Owned(b"/home/slashuser/".to_vec()))
        } else {
            None
        }
    }

    fn expand_scratch_mut(&mut self) -> &mut crate::expand::scratch::ExpandScratch {
        // Test-only: many unit tests mutate `ctx.env` directly (bypassing
        // `set_var`) and expect the very next expansion to see the new IFS.
        // Production uses proper `set_var`, which calls `invalidate_ifs` for
        // us. Here we conservatively invalidate on every access so the
        // behavioural tests stay independent of cache state.
        self.scratch.invalidate_ifs();
        &mut self.scratch
    }
}

pub(super) struct DefaultPathContext {
    pub(super) env: ShellMap<Vec<u8>, Vec<u8>>,
    pub(super) nounset_enabled: bool,
    pub(super) scratch: crate::expand::scratch::ExpandScratch,
}

impl DefaultPathContext {
    pub(super) fn new() -> Self {
        let mut env = ShellMap::default();
        env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        Self {
            env,
            nounset_enabled: false,
            scratch: crate::expand::scratch::ExpandScratch::new(),
        }
    }
}

impl Context for DefaultPathContext {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.env.get(name).map(|v| Cow::Borrowed(v.as_slice()))
    }

    fn special_param(&self, _name: u8) -> Option<Cow<'_, [u8]>> {
        None
    }

    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
        if index == 0 {
            Some(Cow::Owned(b"meiksh".to_vec()))
        } else {
            None
        }
    }

    fn positional_params(&self) -> &[Vec<u8>] {
        &[]
    }

    fn set_var(&mut self, name: &[u8], value: &[u8]) -> Result<(), ExpandError> {
        self.env.insert(name.to_vec(), value.to_vec());
        Ok(())
    }

    fn nounset_enabled(&self) -> bool {
        self.nounset_enabled
    }

    fn shell_name(&self) -> &[u8] {
        b"meiksh"
    }

    fn command_substitute(&mut self, program: &Rc<Program>) -> Result<Vec<u8>, ExpandError> {
        let mut out = Vec::new();
        crate::exec::render::render_program_into(program, &mut out);
        out.push(b'\n');
        Ok(out)
    }

    fn command_substitute_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let mut out = command.to_vec();
        out.push(b'\n');
        Ok(out)
    }

    fn home_dir_for_user(&self, _name: &[u8]) -> Option<Cow<'_, [u8]>> {
        None
    }

    fn expand_scratch_mut(&mut self) -> &mut crate::expand::scratch::ExpandScratch {
        &mut self.scratch
    }
}

pub(super) fn expect_one(result: Result<(Expansion, usize), ExpandError>) -> (Vec<u8>, usize) {
    let (expansion, consumed) = result.expect("expansion");
    match expansion {
        Expansion::One(s) => (s, consumed),
        Expansion::Static(s) => (s.to_vec(), consumed),
        Expansion::AtFields(_) => panic!("expected One/Static, got AtFields"),
    }
}

/// Test-only entry point that expands a single `Word` end-to-end through
/// the production pipeline. Unit tests throughout `src/expand/*` build
/// `Word`s by hand, frequently with `parts: Box::new([])`, and expect the
/// expander to tokenise the raw source. We reparse those inputs via
/// [`reparse_test_word`] so the hot path in `expand_word_into` stays free
/// of a dedicated "empty parts" branch.
pub(crate) fn expand_word<C: Context>(
    ctx: &mut C,
    word: &Word,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let reparsed = reparse_test_word(word)?;
    let mut argv = Vec::new();
    with_scratch(ctx, |ctx, scratch| {
        ensure_ifs_cached(ctx, scratch);
        expand_word_with_scratch(ctx, &reparsed, scratch, &mut argv)
    })?;
    Ok(argv)
}

/// Test-only batched entry point mirroring [`expand_word`]. Each input
/// word is reparsed if it was hand-built with empty `parts`.
pub(crate) fn expand_words<C: Context>(
    ctx: &mut C,
    words: &[Word],
) -> Result<Vec<Vec<u8>>, ExpandError> {
    let mut argv = Vec::with_capacity(words.len());
    let mut reparsed: Vec<Word> = Vec::with_capacity(words.len());
    for w in words {
        reparsed.push(reparse_test_word(w)?);
    }
    expand_words_into(ctx, &reparsed, &mut argv)?;
    Ok(argv)
}

/// Bridge that `expand_word_into` calls (in test builds only) when it
/// encounters the test-only "empty parts" shape. Reparses the word's raw
/// bytes through the real `syntax::parse` pipeline and re-enters
/// `expand_word_into` once per produced sub-word. Production code never
/// reaches this function: the parser guarantees non-empty `parts` for any
/// non-empty `raw`.
pub(super) fn expand_empty_parts_word<C: Context>(
    ctx: &mut C,
    word: &Word,
    ifs: &[u8],
    scratch: &mut ExpandOutput,
    argv: &mut Vec<Vec<u8>>,
) -> Result<(), ExpandError> {
    if word.raw.is_empty() {
        return Ok(());
    }
    let prog = crate::syntax::parse(&word.raw).map_err(|e| ExpandError { message: e.message })?;
    for item in prog.items.iter() {
        if let Some(first_cmd) = item.and_or.first.commands.first() {
            if let Command::Simple(simple) = first_cmd {
                for reparsed in simple.words.iter() {
                    // Preserve the caller-supplied line number so
                    // diagnostics don't leak the synthetic line-1 from
                    // our on-the-fly reparse.
                    let with_line = Word {
                        raw: reparsed.raw.clone(),
                        parts: reparsed.parts.clone(),
                        line: word.line,
                    };
                    expand_word_into(ctx, &with_line, ifs, scratch, argv)?;
                }
            }
        }
    }
    Ok(())
}

/// Rewrite a hand-crafted `Word { raw, parts: [] }` into a real
/// parser-produced `Word` with populated `parts`, so unit tests drive the
/// same pipeline as production code. If `word.parts` is already populated,
/// returns a clone unchanged.
///
/// Parse errors surface as `ExpandError` (with the parser's message) so
/// existing tests that expect an expand error for malformed input such as
/// `'oops` or `$(echo` continue to work.
fn reparse_test_word(word: &Word) -> Result<Word, ExpandError> {
    if !word.parts.is_empty() || word.raw.is_empty() {
        return Ok(Word {
            raw: word.raw.clone(),
            parts: word.parts.clone(),
            line: word.line,
        });
    }

    let program =
        crate::syntax::parse(&word.raw).map_err(|e| ExpandError { message: e.message })?;
    let first_item = program
        .items
        .into_vec()
        .into_iter()
        .next()
        .ok_or_else(|| ExpandError {
            message: b"test helper: parse produced no items".as_ref().into(),
        })?;
    let first_command = first_item
        .and_or
        .first
        .commands
        .into_vec()
        .into_iter()
        .next()
        .ok_or_else(|| ExpandError {
            message: b"test helper: empty pipeline".as_ref().into(),
        })?;
    let simple = match first_command {
        Command::Simple(s) => s,
        _ => {
            return Err(ExpandError {
                message: b"test helper: raw did not parse as a simple command"
                    .as_ref()
                    .into(),
            });
        }
    };
    let first_word = simple
        .words
        .into_vec()
        .into_iter()
        .next()
        .ok_or_else(|| ExpandError {
            message: b"test helper: simple command had no words".as_ref().into(),
        })?;
    Ok(Word {
        raw: first_word.raw,
        parts: first_word.parts,
        line: word.line,
    })
}
