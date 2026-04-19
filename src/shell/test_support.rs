use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::hash::ShellMap;
use crate::sys;
use crate::trace_entries;

use super::options::ShellOptions;
use super::state::{SharedEnv, Shell};

pub fn fake_handle(pid: sys::types::Pid) -> sys::types::ChildHandle {
    sys::types::ChildHandle {
        pid,
        stdout_fd: None,
    }
}

pub fn t_stderr(msg: &str) -> crate::sys::test_support::TraceEntry {
    trace_entries![write(fd(sys::constants::STDERR_FILENO), bytes(format!("{msg}\n"))) -> auto]
        .pop()
        .expect("t_stderr trace")
}

pub fn test_shell() -> Shell {
    Shell {
        options: ShellOptions::default(),
        shell_name: b"meiksh"[..].into(),
        shared: Rc::new(SharedEnv {
            env: ShellMap::default(),
            exported: BTreeSet::new(),
            readonly: BTreeSet::new(),
            aliases: ShellMap::default(),
            functions: ShellMap::default(),
            path_cache: ShellMap::default(),
            history: Vec::new(),
            mail_sizes: ShellMap::default(),
        }),
        positional: Vec::new(),
        last_status: 0,
        last_background: None,
        running: true,
        jobs: Vec::new(),
        known_pid_statuses: HashMap::new(),
        known_job_statuses: HashMap::new(),
        trap_actions: BTreeMap::new(),
        ignored_on_entry: BTreeSet::new(),
        subshell_saved_traps: None,
        loop_depth: 0,
        function_depth: 0,
        source_depth: 0,
        pending_control: None,
        interactive: false,
        errexit_suppressed: false,
        owns_terminal: false,
        in_subshell: false,
        wait_was_interrupted: false,
        pid: 0,
        lineno: 0,
        mail_last_check: 0,
        expand_scratch: crate::expand::scratch::ExpandScratch::new(),
    }
}

pub fn capture_forked_trace(
    exit_status: i32,
    pid: i32,
) -> Vec<crate::sys::test_support::TraceEntry> {
    trace_entries![
        pipe() -> fds(200, 201),
        fork() -> pid(pid), child: [
            close(fd(200)) -> 0,
            dup2(fd(201), fd(sys::constants::STDOUT_FILENO)) -> 0,
            close(fd(201)) -> 0,
        ],
        close(fd(201)) -> 0,
        read(fd(200), _) -> 0,
        close(fd(200)) -> 0,
        waitpid(int(pid), _, int(0)) -> status(exit_status),
    ]
}
