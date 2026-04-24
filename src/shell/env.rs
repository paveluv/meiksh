use crate::bstr;
use crate::sys;

use super::error::{ShellError, VarError, var_error_message};
use super::run::stdin_parse_error_requires_more_input;
use super::state::Shell;
use super::vars::EnvEntry;

const LOCALE_VARS: &[&[u8]] = &[
    b"LC_ALL",
    b"LC_CTYPE",
    b"LC_COLLATE",
    b"LC_NUMERIC",
    b"LC_MESSAGES",
    b"LC_TIME",
    b"LANG",
];

fn is_locale_var(name: &[u8]) -> bool {
    LOCALE_VARS.iter().any(|v| *v == name)
}

impl Shell {
    pub(crate) fn env_for_child(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.vars()
            .iter_exported()
            .filter_map(|(name, entry)| entry.value.as_ref().map(|v| (name.to_vec(), v.clone())))
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
        self.var_value(name)
    }

    pub(crate) fn input_is_incomplete(&self, error: &crate::syntax::ParseError) -> bool {
        stdin_parse_error_requires_more_input(error)
    }

    pub(crate) fn history_number(&self) -> usize {
        self.history().len() + 1
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
        if self.history().len() >= histsize && histsize > 0 {
            self.history_mut().remove(0);
        }
        self.history_mut().push(trimmed.into());
    }

    pub(crate) fn set_var(&mut self, name: &[u8], value: &[u8]) -> Result<(), VarError> {
        let vars = self.vars_mut();
        let slot = vars.ensure_slot(name) as u32;
        self.set_var_by_slot(slot, name, value)
    }

    /// Slot-indexed variant of [`Shell::set_var`]. Callers that have
    /// already resolved the variable's dense-slot index (typically
    /// via an AST-level [`crate::shell::vars::CachedVarBinding`]) use
    /// this to skip the `ShellMap<Vec<u8>, u32>` name lookup and go
    /// straight to the slot vector.
    ///
    /// `name` is still required because it is used for:
    /// * the readonly-error message,
    /// * the side-effect channels that depend on the name (`PATH`
    ///   cache invalidation, `IFS` cache invalidation,
    ///   locale-variable environment propagation).
    ///
    /// The caller must pass the same `name` that `slot` was
    /// resolved from; passing a mismatching pair writes to the slot
    /// indicated by `slot`, which would desync cached bindings.
    pub(crate) fn set_var_by_slot(
        &mut self,
        slot: u32,
        name: &[u8],
        value: &[u8],
    ) -> Result<(), VarError> {
        if self.vars().get_slot(slot).is_some_and(|e| e.readonly) {
            return Err(VarError::Readonly(name.into()));
        }
        let path_var = name == b"PATH";
        let ifs_var = name == b"IFS";
        let allexport = self.options.allexport;
        {
            let vars = self.vars_mut();
            // Grow the slot vector if necessary. This should not
            // normally happen since the caller resolved the slot via
            // ensure_slot, but a copy-on-write SharedEnv clone may
            // have shrunk us here.
            while (slot as usize) >= vars.slots.len() {
                vars.slots.push(None);
            }
            let slot_idx = slot as usize;
            match &mut vars.slots[slot_idx] {
                Some(entry) => {
                    match &mut entry.value {
                        Some(buf) => {
                            buf.clear();
                            buf.extend_from_slice(value);
                        }
                        val @ None => *val = Some(value.to_vec()),
                    }
                    if allexport {
                        entry.exported = true;
                    }
                }
                None => {
                    vars.slots[slot_idx] = Some(EnvEntry {
                        value: Some(value.to_vec()),
                        exported: allexport,
                        readonly: false,
                    });
                }
            }
        }
        if path_var {
            self.path_cache_mut().clear();
        }
        if ifs_var && let Some(s) = self.expand_scratch.as_mut() {
            s.invalidate_ifs();
        }
        if is_locale_var(name) {
            #[cfg(not(test))]
            {
                let _ = sys::env::env_set_var(name, value);
            }
            sys::locale::reinit_locale();
        }
        Ok(())
    }

    pub(crate) fn export_var(
        &mut self,
        name: &[u8],
        value: Option<&[u8]>,
    ) -> Result<(), ShellError> {
        if let Some(value) = value {
            self.set_var(name, value).map_err(|e| {
                let msg = var_error_message(&e);
                self.diagnostic(1, &msg)
            })?;
        }
        self.mark_exported(name);
        Ok(())
    }

    pub(crate) fn mark_readonly(&mut self, name: &[u8]) {
        let vars = self.vars_mut();
        let slot = vars.ensure_slot(name) as usize;
        match &mut vars.slots[slot] {
            Some(entry) => entry.readonly = true,
            None => {
                vars.slots[slot] = Some(EnvEntry {
                    value: None,
                    exported: false,
                    readonly: true,
                });
            }
        }
    }

    pub(crate) fn unset_var(&mut self, name: &[u8]) -> Result<(), VarError> {
        if self.is_readonly(name) {
            return Err(VarError::Readonly(name.into()));
        }
        let vars = self.vars_mut();
        if let Some(slot) = vars.slot_of(name) {
            vars.slots[slot as usize] = None;
        }
        if name == b"PATH" {
            self.path_cache_mut().clear();
        }
        if name == b"IFS"
            && let Some(s) = self.expand_scratch.as_mut()
        {
            s.invalidate_ifs();
        }
        if is_locale_var(name) {
            #[cfg(not(test))]
            {
                let _ = sys::env::env_unset_var(name);
            }
            sys::locale::reinit_locale();
        }
        Ok(())
    }

    pub(crate) fn set_positional(&mut self, values: Vec<Vec<u8>>) {
        self.positional = values;
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::test_support::t_stderr;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    use crate::shell::error::var_error_message;
    use crate::shell::test_support::test_shell;

    #[test]
    fn env_for_child_filters_exported_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"A".to_vec(), b"1".to_vec());
            shell.env_set_raw(b"B".to_vec(), b"2".to_vec());
            shell.mark_exported(b"A");
            let env = shell.env_for_child();
            assert_eq!(
                env.iter()
                    .find(|(k, _)| k == b"A")
                    .map(|(_, v)| v.as_slice()),
                Some(b"1".as_slice())
            );
            assert!(!env.iter().any(|(k, _)| k == b"B"));

            shell.options.allexport = true;
            shell.set_var(b"B", b"3").expect("allexport set");
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
            shell.set_var(b"NAME", b"value").expect("set");
            shell.mark_readonly(b"NAME");
            let set_error = shell.set_var(b"NAME", b"new").expect_err("readonly");
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
            shell.env_set_raw(b"NAME".to_vec(), b"value".to_vec());
            shell.export_var(b"NAME", None).expect("export");
            assert!(shell.is_exported(b"NAME"));
        });
    }

    #[test]
    fn env_for_exec_utility_overlays_and_appends() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env_set_raw(b"A".to_vec(), b"1".to_vec());
            shell.mark_exported(b"A");
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
        assert!(shell.history().is_empty());

        shell.add_history(b"first");
        assert_eq!(shell.history().len(), 1);

        shell.env_set_raw(b"HISTSIZE".to_vec(), b"2".to_vec());
        shell.add_history(b"second");
        shell.add_history(b"third");
        assert_eq!(shell.history().len(), 2);
        assert_eq!(&*shell.history()[0], b"second".as_slice());
        assert_eq!(&*shell.history()[1], b"third".as_slice());
    }

    #[test]
    fn set_var_fills_previously_marked_empty_slot() {
        // `mark_exported` creates `Some(EnvEntry { value: None, .. })`
        // when the slot had not yet been used.  A subsequent
        // `set_var` must then take the `val @ None` arm rather than
        // the `Some(buf)` reuse arm.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.mark_exported(b"LAZY");
            assert_eq!(shell.get_var(b"LAZY"), None);
            shell.set_var(b"LAZY", b"now").expect("set");
            assert_eq!(shell.get_var(b"LAZY"), Some(b"now".as_slice()));
            assert!(shell.is_exported(b"LAZY"));
        });
    }

    #[test]
    fn set_var_grows_slot_vector_when_shrunk() {
        // Hits the `while (slot as usize) >= vars.slots.len()` fix-up
        // guard that keeps `set_var_by_slot` safe when a copy-on-write
        // clone has left the slot vector shorter than the caller's
        // pre-computed index.
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let slot = shell.vars_mut().ensure_slot(b"GROW");
            shell.vars_mut().slots.clear();
            shell
                .set_var_by_slot(slot, b"GROW", b"v")
                .expect("grow + set");
            assert_eq!(shell.get_var(b"GROW"), Some(b"v".as_slice()));
        });
    }

    #[test]
    fn export_var_error_on_readonly() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: RO: readonly variable")]],
            || {
                let mut shell = test_shell();
                shell.set_var(b"RO", b"orig").expect("set");
                shell.mark_readonly(b"RO");
                let error = shell
                    .export_var(b"RO", Some(b"new"))
                    .expect_err("readonly export");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }
}
