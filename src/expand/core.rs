use std::borrow::Cow;

#[derive(Debug)]
pub(crate) struct ExpandError {
    pub(crate) message: Box<[u8]>,
}

pub(crate) trait Context {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>>;
    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>>;
    fn positional_params(&self) -> &[Vec<u8>];
    fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool {
        true
    }
    fn shell_name(&self) -> &[u8];
    fn command_substitute(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError>;
    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>>;
    fn set_lineno(&mut self, _line: usize) {}
    fn inc_lineno(&mut self) {}
    fn lineno(&self) -> usize {
        0
    }
}
