use std::borrow::Cow;
use std::rc::Rc;

use crate::bstr;
use crate::expand::core::{Context, ExpandError};
use crate::syntax::ast::Program;
use crate::sys;

use super::error::var_error_message;
use super::state::Shell;

impl Context for Shell {
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.env().get(name).map(|v| Cow::Borrowed(v.as_slice()))
    }

    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>> {
        match name {
            b'?' => Some(Cow::Owned(bstr::i64_to_bytes(self.last_status as i64))),
            b'$' => Some(Cow::Owned(bstr::i64_to_bytes(self.pid as i64))),
            b'!' => self
                .last_background
                .map(|pid| Cow::Owned(bstr::i64_to_bytes(pid as i64))),
            b'#' => Some(Cow::Owned(bstr::u64_to_bytes(self.positional.len() as u64))),
            b'-' => Some(Cow::Owned(self.active_option_flags())),
            b'*' | b'@' => Some(Cow::Owned(bstr::join_bstrings(&self.positional, b" "))),
            b'0' => Some(Cow::Borrowed(&self.shell_name)),
            digit if digit.is_ascii_digit() => {
                let index = (digit - b'0') as usize;
                self.positional
                    .get(index.saturating_sub(1))
                    .map(|v| Cow::Borrowed(v.as_slice()))
            }
            _ => None,
        }
    }

    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
        if index == 0 {
            Some(Cow::Borrowed(&self.shell_name))
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
        self.set_var(name, value).map_err(|e| {
            let msg = var_error_message(&e);
            ExpandError {
                message: msg.into(),
            }
        })
    }

    fn pathname_expansion_enabled(&self) -> bool {
        !self.options.noglob
    }

    fn nounset_enabled(&self) -> bool {
        self.options.nounset
    }

    fn shell_name(&self) -> &[u8] {
        &self.shell_name
    }

    fn command_substitute(&mut self, program: &Rc<Program>) -> Result<Vec<u8>, ExpandError> {
        self.capture_output_program(program)
            .map_err(|_| ExpandError {
                message: Box::default(),
            })
    }

    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        sys::env::home_dir_for_user(name).map(Cow::Owned)
    }

    fn set_lineno(&mut self, line: usize) {
        self.lineno = line;
    }
    fn inc_lineno(&mut self) {
        self.lineno += 1;
    }
    fn lineno(&self) -> usize {
        self.lineno
    }
}

#[cfg(test)]
mod tests {
    use crate::expand::core::Context;
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    use crate::shell::test_support::{capture_forked_trace, t_stderr, test_shell};

    #[test]
    fn special_parameters_reflect_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.pid = 12345;
            shell.positional = vec![b"first".to_vec(), b"second".to_vec()];
            shell.last_status = 17;
            shell.last_background = Some(42);
            shell.options.allexport = true;
            shell.options.noclobber = true;
            shell.options.command_string = Some(b"printf ok"[..].into());
            assert_eq!(
                Context::special_param(&shell, b'?').as_deref(),
                Some(b"17".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'$').as_deref(),
                Some(b"12345".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'#').as_deref(),
                Some(b"2".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'!').as_deref(),
                Some(b"42".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'-').as_deref(),
                Some(b"aCc".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'*').as_deref(),
                Some(b"first second".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'@').as_deref(),
                Some(b"first second".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'1').as_deref(),
                Some(b"first".as_slice())
            );
            assert_eq!(
                Context::special_param(&shell, b'0').as_deref(),
                Some(b"meiksh".as_slice())
            );
            assert_eq!(Context::special_param(&shell, b'9'), None);
            assert_eq!(Context::special_param(&shell, b'x'), None);
        });
    }

    #[test]
    fn dollar_hyphen_includes_i_when_interactive() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.interactive = true;
            assert!(shell.active_option_flags().contains(&b'i'));
        });
    }

    #[test]
    fn dollar_hyphen_excludes_i_when_not_interactive() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert!(!shell.active_option_flags().contains(&b'i'));
        });
    }

    #[test]
    fn context_trait_methods_work() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(Context::shell_name(&shell), b"meiksh");
            assert_eq!(
                Context::positional_param(&shell, 0).as_deref(),
                Some(b"meiksh".as_slice())
            );
            Context::set_var(&mut shell, b"CTX_SET", b"7").expect("ctx set");
            assert_eq!(shell.get_var(b"CTX_SET"), Some(b"7".as_slice()));
            shell.mark_readonly(b"CTX_SET");
            let error =
                Context::set_var(&mut shell, b"CTX_SET", b"8").expect_err("readonly ctx set");
            assert_eq!(&*error.message, b"CTX_SET: readonly variable".as_slice());
        });
    }

    #[test]
    fn command_substitute_success() {
        run_trace(trace_entries![..capture_forked_trace(0, 1000)], || {
            let mut shell = test_shell();
            let substituted = Context::command_substitute_raw(&mut shell, b"true").expect("subst");
            assert_eq!(substituted, b"");
            assert_eq!(shell.last_status, 0);
        });
    }

    #[test]
    fn command_substitute_sets_last_status_on_nonzero_exit() {
        run_trace(trace_entries![..capture_forked_trace(1, 1000)], || {
            let mut shell = test_shell();
            let output = Context::command_substitute_raw(&mut shell, b"false").expect("subst ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn command_substitute_maps_error() {
        run_trace(
            trace_entries![
                pipe() -> err(sys::constants::EIO),
                ..vec![t_stderr("meiksh: Input/output error")],
            ],
            || {
                let mut shell = test_shell();
                let result = Context::command_substitute_raw(&mut shell, b"true");
                assert!(result.is_err());
            },
        );
    }
}
