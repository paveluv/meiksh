use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::sys;
use crate::sys::test_support::{ArgMatcher, TraceResult, t, t_fork};

use super::options::ShellOptions;
use super::state::Shell;

pub fn fake_handle(pid: sys::Pid) -> sys::ChildHandle {
    sys::ChildHandle {
        pid,
        stdout_fd: None,
    }
}

pub fn t_stderr(msg: &str) -> crate::sys::test_support::TraceEntry {
    t(
        "write",
        vec![
            ArgMatcher::Fd(sys::STDERR_FILENO),
            ArgMatcher::Bytes(format!("{msg}\n").into_bytes()),
        ],
        TraceResult::Auto,
    )
}

pub fn test_shell() -> Shell {
    Shell {
        options: ShellOptions::default(),
        shell_name: b"meiksh"[..].into(),
        env: HashMap::new(),
        exported: BTreeSet::new(),
        readonly: BTreeSet::new(),
        aliases: HashMap::new(),
        functions: HashMap::new(),
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
        path_cache: HashMap::new(),
        history: Vec::new(),
        mail_last_check: 0,
        mail_sizes: HashMap::new(),
    }
}

pub fn capture_forked_trace(
    exit_status: i32,
    pid: i32,
) -> Vec<crate::sys::test_support::TraceEntry> {
    let child = vec![
        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
        t(
            "dup2",
            vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
            TraceResult::Int(0),
        ),
        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
    ];
    vec![
        t("pipe", vec![], TraceResult::Fds(200, 201)),
        t_fork(TraceResult::Pid(pid), child),
        t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
        t(
            "read",
            vec![ArgMatcher::Fd(200), ArgMatcher::Any],
            TraceResult::Int(0),
        ),
        t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
        t(
            "waitpid",
            vec![
                ArgMatcher::Int(pid as i64),
                ArgMatcher::Any,
                ArgMatcher::Int(0),
            ],
            TraceResult::Status(exit_status),
        ),
    ]
}
