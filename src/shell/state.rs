use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::hash::ShellMap;
use crate::sys;

use super::jobs::Job;
use super::options::ShellOptions;
use super::traps::{TrapAction, TrapCondition};

pub(crate) enum FlowSignal {
    Continue(i32),
    UtilityError(i32),
    Exit(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingControl {
    Return(i32),
    Break(usize),
    Continue(usize),
}

#[derive(Clone)]
pub(crate) struct SharedEnv {
    pub(crate) env: ShellMap<Vec<u8>, Vec<u8>>,
    pub(crate) exported: BTreeSet<Vec<u8>>,
    pub(crate) readonly: BTreeSet<Vec<u8>>,
    pub(crate) aliases: ShellMap<Box<[u8]>, Box<[u8]>>,
    pub(crate) functions: ShellMap<Vec<u8>, Rc<crate::syntax::ast::Command>>,
    pub(crate) path_cache: ShellMap<Box<[u8]>, Vec<u8>>,
    pub(crate) history: Vec<Box<[u8]>>,
    pub(crate) mail_sizes: ShellMap<Box<[u8]>, u64>,
}

#[derive(Clone)]
pub(crate) struct Shell {
    pub(crate) options: ShellOptions,
    pub(crate) shell_name: Box<[u8]>,
    pub(crate) shared: Rc<SharedEnv>,
    pub(crate) positional: Vec<Vec<u8>>,
    pub(crate) last_status: i32,
    pub(crate) last_background: Option<sys::types::Pid>,
    pub(crate) running: bool,
    pub(crate) jobs: Vec<Job>,
    pub(crate) known_pid_statuses: HashMap<sys::types::Pid, i32>,
    pub(crate) known_job_statuses: HashMap<usize, i32>,
    pub(crate) trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub(crate) ignored_on_entry: BTreeSet<TrapCondition>,
    pub(crate) subshell_saved_traps: Option<BTreeMap<TrapCondition, TrapAction>>,
    pub(crate) loop_depth: usize,
    pub(crate) function_depth: usize,
    /// Nesting depth of dot (`source_path`) files being executed.
    pub(crate) source_depth: usize,
    pub(crate) pending_control: Option<PendingControl>,
    pub(crate) interactive: bool,
    pub(crate) errexit_suppressed: bool,
    pub(crate) owns_terminal: bool,
    pub(crate) in_subshell: bool,
    pub(crate) wait_was_interrupted: bool,
    pub(crate) pid: sys::types::Pid,
    pub(crate) lineno: usize,
    pub(crate) mail_last_check: u64,
}

impl Shell {
    pub(crate) fn env(&self) -> &ShellMap<Vec<u8>, Vec<u8>> {
        &self.shared.env
    }
    pub(crate) fn env_mut(&mut self) -> &mut ShellMap<Vec<u8>, Vec<u8>> {
        &mut Rc::make_mut(&mut self.shared).env
    }
    pub(crate) fn exported(&self) -> &BTreeSet<Vec<u8>> {
        &self.shared.exported
    }
    pub(crate) fn exported_mut(&mut self) -> &mut BTreeSet<Vec<u8>> {
        &mut Rc::make_mut(&mut self.shared).exported
    }
    pub(crate) fn readonly(&self) -> &BTreeSet<Vec<u8>> {
        &self.shared.readonly
    }
    pub(crate) fn readonly_mut(&mut self) -> &mut BTreeSet<Vec<u8>> {
        &mut Rc::make_mut(&mut self.shared).readonly
    }
    pub(crate) fn aliases(&self) -> &ShellMap<Box<[u8]>, Box<[u8]>> {
        &self.shared.aliases
    }
    pub(crate) fn aliases_mut(&mut self) -> &mut ShellMap<Box<[u8]>, Box<[u8]>> {
        &mut Rc::make_mut(&mut self.shared).aliases
    }
    pub(crate) fn functions(&self) -> &ShellMap<Vec<u8>, Rc<crate::syntax::ast::Command>> {
        &self.shared.functions
    }
    pub(crate) fn functions_mut(
        &mut self,
    ) -> &mut ShellMap<Vec<u8>, Rc<crate::syntax::ast::Command>> {
        &mut Rc::make_mut(&mut self.shared).functions
    }
    pub(crate) fn path_cache(&self) -> &ShellMap<Box<[u8]>, Vec<u8>> {
        &self.shared.path_cache
    }
    pub(crate) fn path_cache_mut(&mut self) -> &mut ShellMap<Box<[u8]>, Vec<u8>> {
        &mut Rc::make_mut(&mut self.shared).path_cache
    }
    pub(crate) fn history(&self) -> &Vec<Box<[u8]>> {
        &self.shared.history
    }
    pub(crate) fn history_mut(&mut self) -> &mut Vec<Box<[u8]>> {
        &mut Rc::make_mut(&mut self.shared).history
    }
    pub(crate) fn mail_sizes(&self) -> &ShellMap<Box<[u8]>, u64> {
        &self.shared.mail_sizes
    }
    pub(crate) fn mail_sizes_mut(&mut self) -> &mut ShellMap<Box<[u8]>, u64> {
        &mut Rc::make_mut(&mut self.shared).mail_sizes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shell::test_support::test_shell;
    use crate::trace_entries;

    #[test]
    fn run_builtin_returns_correct_flow_signals() {
        crate::sys::test_support::run_trace(trace_entries![], || {
            let mut shell = test_shell();

            let flow = shell
                .run_builtin(
                    &[b"export".to_vec(), b"FLOW=1".to_vec()],
                    &[(b"ASSIGN".to_vec(), b"2".to_vec())],
                )
                .expect("builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.get_var(b"ASSIGN"), Some(b"2".as_slice()));
            assert_eq!(shell.get_var(b"FLOW"), Some(b"1".as_slice()));

            let flow = shell
                .run_builtin(&[b"exit".to_vec(), b"9".to_vec()], &[])
                .expect("exit builtin");
            assert!(matches!(flow, FlowSignal::Exit(9)));

            shell.function_depth = 1;
            let flow = shell
                .run_builtin(&[b"return".to_vec(), b"4".to_vec()], &[])
                .expect("return builtin");
            assert!(matches!(flow, FlowSignal::Continue(4)));
            assert_eq!(shell.pending_control, Some(PendingControl::Return(4)));
            shell.pending_control = None;
            shell.function_depth = 0;

            shell.loop_depth = 2;
            let flow = shell
                .run_builtin(&[b"break".to_vec(), b"5".to_vec()], &[])
                .expect("break builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.pending_control, Some(PendingControl::Break(2)));
            shell.pending_control = None;
        });
    }

    #[test]
    fn return_in_dot_sourced_file_exits_source_with_status() {
        crate::sys::test_support::assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.source_depth = 1;
            let status = shell
                .execute_string(b":; return 5; :")
                .expect("return from source");
            assert_eq!(status, 5);
            assert!(shell.pending_control.is_none());
        });
    }
}
