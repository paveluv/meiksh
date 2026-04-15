#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::shell::options::ShellOptions;
use crate::shell::state::Shell;
use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};
use crate::sys;
use crate::sys::test_support::{ArgMatcher, TraceEntry, TraceResult, t};

pub(super) fn parse_test(
    source: &str,
) -> Result<crate::syntax::ast::Program, crate::syntax::ParseError> {
    crate::syntax::parse(source.as_bytes())
}

pub(super) fn test_shell() -> Shell {
    Shell {
        options: ShellOptions::default(),
        shell_name: b"meiksh".to_vec().into(),
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

pub(super) fn t_stderr(msg: &str) -> TraceEntry {
    t(
        "write",
        vec![
            ArgMatcher::Fd(sys::constants::STDERR_FILENO),
            ArgMatcher::Bytes(format!("{msg}\n").into_bytes()),
        ],
        TraceResult::Auto,
    )
}
