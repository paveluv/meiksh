use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::sys;

use super::jobs::Job;
use super::options::ShellOptions;
use super::traps::{TrapAction, TrapCondition};

pub enum FlowSignal {
    Continue(i32),
    UtilityError(i32),
    Exit(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingControl {
    Return(i32),
    Break(usize),
    Continue(usize),
}

#[derive(Clone)]
pub struct Shell {
    pub options: ShellOptions,
    pub shell_name: Box<[u8]>,
    pub env: HashMap<Vec<u8>, Vec<u8>>,
    pub exported: BTreeSet<Vec<u8>>,
    pub readonly: BTreeSet<Vec<u8>>,
    pub aliases: HashMap<Box<[u8]>, Box<[u8]>>,
    pub functions: HashMap<Vec<u8>, crate::syntax::Command>,
    pub positional: Vec<Vec<u8>>,
    pub last_status: i32,
    pub last_background: Option<sys::Pid>,
    pub running: bool,
    pub jobs: Vec<Job>,
    pub known_pid_statuses: HashMap<sys::Pid, i32>,
    pub known_job_statuses: HashMap<usize, i32>,
    pub trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub ignored_on_entry: BTreeSet<TrapCondition>,
    pub(crate) subshell_saved_traps: Option<BTreeMap<TrapCondition, TrapAction>>,
    pub loop_depth: usize,
    pub function_depth: usize,
    /// Nesting depth of dot (`source_path`) files being executed.
    pub source_depth: usize,
    pub pending_control: Option<PendingControl>,
    pub(crate) interactive: bool,
    pub(crate) errexit_suppressed: bool,
    pub(crate) owns_terminal: bool,
    pub(crate) in_subshell: bool,
    pub(crate) wait_was_interrupted: bool,
    pub(crate) pid: sys::Pid,
    pub(crate) lineno: usize,
    pub path_cache: HashMap<Box<[u8]>, Vec<u8>>,
    pub history: Vec<Box<[u8]>>,
    pub(crate) mail_last_check: u64,
    pub(crate) mail_sizes: HashMap<Box<[u8]>, u64>,
}

#[cfg(test)]
mod tests {
    use super::{FlowSignal, PendingControl};
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
