use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

use crate::bstr;
use crate::expand::core::{Context, ExpandError};
use crate::expand::model::Expansion;
use crate::syntax::ast::Program;

pub(super) struct FakeContext {
    pub(super) env: HashMap<Vec<u8>, Vec<u8>>,
    pub(super) positional: Vec<Vec<u8>>,
    pub(super) pathname_expansion_enabled: bool,
    pub(super) nounset_enabled: bool,
}

impl FakeContext {
    pub(super) fn new() -> Self {
        let mut env = HashMap::new();
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

    fn command_substitute(&mut self, _program: &Rc<Program>) -> Result<Vec<u8>, ExpandError> {
        Ok(b"fake_command_output\n".to_vec())
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
}

pub(super) struct DefaultPathContext {
    pub(super) env: HashMap<Vec<u8>, Vec<u8>>,
    pub(super) nounset_enabled: bool,
}

impl DefaultPathContext {
    pub(super) fn new() -> Self {
        let mut env = HashMap::new();
        env.insert(b"HOME".to_vec(), b"/tmp/home".to_vec());
        Self {
            env,
            nounset_enabled: false,
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

    fn command_substitute(&mut self, _program: &Rc<Program>) -> Result<Vec<u8>, ExpandError> {
        Ok(b"fake_command_output\n".to_vec())
    }

    fn command_substitute_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let mut out = command.to_vec();
        out.push(b'\n');
        Ok(out)
    }

    fn home_dir_for_user(&self, _name: &[u8]) -> Option<Cow<'_, [u8]>> {
        None
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
