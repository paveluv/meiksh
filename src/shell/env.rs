use crate::bstr;

use super::error::{ShellError, VarError, var_error_message};
use super::run::stdin_parse_error_requires_more_input;
use super::state::Shell;

impl Shell {
    pub(crate) fn env_for_child(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.exported
            .iter()
            .filter_map(|name| {
                self.env
                    .get(name)
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect()
    }

    pub(crate) fn env_for_exec_utility(
        &self,
        cmd_assignments: &[(Vec<u8>, Vec<u8>)],
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut env = self.env_for_child();
        for (k, v) in cmd_assignments {
            if let Some(pos) = env.iter().position(|(name, _)| name == k) {
                env[pos] = (k.clone(), v.clone());
            } else {
                env.push((k.clone(), v.clone()));
            }
        }
        env
    }

    pub(crate) fn get_var(&self, name: &[u8]) -> Option<&[u8]> {
        self.env.get(name).map(Vec::as_slice)
    }

    pub(crate) fn input_is_incomplete(&self, error: &crate::syntax::ParseError) -> bool {
        stdin_parse_error_requires_more_input(error)
    }

    pub(crate) fn history_number(&self) -> usize {
        self.history.len() + 1
    }

    pub(crate) fn add_history(&mut self, line: &[u8]) {
        let mut end = line.len();
        while end > 0
            && (line[end - 1] == b' '
                || line[end - 1] == b'\t'
                || line[end - 1] == b'\n'
                || line[end - 1] == b'\r')
        {
            end -= 1;
        }
        let trimmed = &line[..end];
        if trimmed.is_empty() {
            return;
        }
        let histsize = self
            .get_var(b"HISTSIZE")
            .and_then(bstr::parse_i64)
            .and_then(|v| if v >= 0 { Some(v as usize) } else { None })
            .unwrap_or(128);
        if self.history.len() >= histsize && histsize > 0 {
            self.history.remove(0);
        }
        self.history.push(trimmed.into());
    }

    pub(crate) fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        if name == b"PATH" {
            self.path_cache.clear();
        }
        if let Some(existing) = self.env.get_mut(name) {
            *existing = value;
        } else {
            self.env.insert(name.to_vec(), value);
        }
        if self.options.allexport && !self.exported.contains(name) {
            self.exported.insert(name.to_vec());
        }
        Ok(())
    }

    pub(crate) fn export_var(
        &mut self,
        name: &[u8],
        value: Option<Vec<u8>>,
    ) -> Result<(), ShellError> {
        if let Some(value) = value {
            self.set_var(name, value).map_err(|e| {
                let msg = var_error_message(&e);
                self.diagnostic(1, &msg)
            })?;
        }
        if !self.exported.contains(name) {
            self.exported.insert(name.to_vec());
        }
        Ok(())
    }

    pub(crate) fn mark_readonly(&mut self, name: &[u8]) {
        self.readonly.insert(name.to_vec());
    }

    pub(crate) fn unset_var(&mut self, name: &[u8]) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        self.env.remove(name);
        self.exported.remove(name);
        Ok(())
    }

    pub(crate) fn set_positional(&mut self, values: Vec<Vec<u8>>) {
        self.positional = values;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shell::test_support::t_stderr;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    use crate::shell::error::var_error_message;
    use crate::shell::test_support::test_shell;

    #[test]
    fn env_for_child_filters_exported_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"A".to_vec(), b"1".to_vec());
            shell.env.insert(b"B".to_vec(), b"2".to_vec());
            shell.exported.insert(b"A".to_vec());
            let env = shell.env_for_child();
            assert_eq!(
                env.iter()
                    .find(|(k, _)| k == b"A")
                    .map(|(_, v)| v.as_slice()),
                Some(b"1".as_slice())
            );
            assert!(!env.iter().any(|(k, _)| k == b"B"));

            shell.options.allexport = true;
            shell.set_var(b"B", b"3".to_vec()).expect("allexport set");
            let env = shell.env_for_child();
            assert_eq!(
                env.iter()
                    .find(|(k, _)| k == b"B")
                    .map(|(_, v)| v.as_slice()),
                Some(b"3".as_slice())
            );
        });
    }

    #[test]
    fn readonly_variables_reject_mutation_and_unset() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.set_var(b"NAME", b"value".to_vec()).expect("set");
            shell.mark_readonly(b"NAME");
            let set_error = shell
                .set_var(b"NAME", b"new".to_vec())
                .expect_err("readonly");
            let msg = var_error_message(&set_error);
            assert_eq!(msg, b"NAME: readonly variable");
            let unset_error = shell.unset_var(b"NAME").expect_err("readonly");
            let msg = var_error_message(&unset_error);
            assert_eq!(msg, b"NAME: readonly variable");
        });
    }

    #[test]
    fn export_without_value_marks_variable_exported() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"NAME".to_vec(), b"value".to_vec());
            shell.export_var(b"NAME", None).expect("export");
            assert!(shell.exported.contains(b"NAME".as_slice()));
        });
    }

    #[test]
    fn env_for_exec_utility_overlays_and_appends() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"A".to_vec(), b"1".to_vec());
            shell.exported.insert(b"A".to_vec());
            let env = shell.env_for_exec_utility(&[
                (b"A".to_vec(), b"2".to_vec()),
                (b"B".to_vec(), b"3".to_vec()),
            ]);
            assert!(env.iter().any(|(k, v)| k == b"A" && v == b"2"));
            assert!(env.iter().any(|(k, v)| k == b"B" && v == b"3"));
        });
    }

    #[test]
    fn add_history_skips_empty_and_respects_histsize() {
        let mut shell = test_shell();
        shell.add_history(b"");
        shell.add_history(b"   ");
        assert!(shell.history.is_empty());

        shell.add_history(b"first");
        assert_eq!(shell.history.len(), 1);

        shell.env.insert(b"HISTSIZE".to_vec(), b"2".to_vec());
        shell.add_history(b"second");
        shell.add_history(b"third");
        assert_eq!(shell.history.len(), 2);
        assert_eq!(&*shell.history[0], b"second".as_slice());
        assert_eq!(&*shell.history[1], b"third".as_slice());
    }

    #[test]
    fn export_var_error_on_readonly() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: RO: readonly variable")]],
            || {
                let mut shell = test_shell();
                shell.set_var(b"RO", b"orig".to_vec()).expect("set");
                shell.mark_readonly(b"RO");
                let error = shell
                    .export_var(b"RO", Some(b"new".to_vec()))
                    .expect_err("readonly export");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }
}
