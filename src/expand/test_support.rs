use std::borrow::Cow;
use std::rc::Rc;

use crate::bstr;
use crate::expand::core::{Context, ExpandError};
use crate::expand::model::Expansion;
use crate::hash::ShellMap;
use crate::syntax::ast::Program;

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

    fn set_lineno(&mut self, _line: usize) {}
    fn inc_lineno(&mut self) {}
    fn lineno(&self) -> usize {
        0
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

    fn pathname_expansion_enabled(&self) -> bool {
        true
    }

    fn set_lineno(&mut self, _line: usize) {}
    fn inc_lineno(&mut self) {}
    fn lineno(&self) -> usize {
        0
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
