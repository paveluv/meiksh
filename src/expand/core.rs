use std::borrow::Cow;
use std::rc::Rc;

use crate::syntax::ast::Program;

#[derive(Debug)]
pub(crate) struct ExpandError {
    pub(crate) message: Box<[u8]>,
}

pub(crate) trait Context {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>>;
    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>>;
    fn positional_params(&self) -> &[Vec<u8>];
    fn set_var(&mut self, name: &[u8], value: &[u8]) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &[u8];
    fn command_substitute(&mut self, program: &Rc<Program>) -> Result<Vec<u8>, ExpandError>;
    fn command_substitute_raw(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        let program = crate::syntax::parse(command).unwrap_or_default();
        self.command_substitute(&Rc::new(program))
    }
    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn set_lineno(&mut self, _line: usize) {}
    fn inc_lineno(&mut self) {}
    fn lineno(&self) -> usize {
        0
    }
}
