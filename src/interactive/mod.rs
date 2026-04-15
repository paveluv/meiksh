use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

mod env_file;
mod history;
mod mail;
mod prompt;
mod repl;
mod vi_editing;

fn remove_file_bytes(path: &[u8]) {
    let _ = sys::fs::unlink(path);
}

pub(crate) fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    sys::fd_io::ensure_blocking_read_fd(sys::constants::STDIN_FILENO)
        .map_err(|e| shell.diagnostic_syserr(1, &e))?;
    repl::run_loop(shell)
}

pub(crate) fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    env_file::load_env_file(shell)
}
#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
pub(super) mod test_support {
    use crate::shell::options::ShellOptions;
    use crate::shell::state::Shell;
    use crate::sys;
    use crate::sys::test_support::{ArgMatcher, TraceEntry, TraceResult, t};
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    pub(crate) fn read_line_trace(input: &[u8]) -> Vec<TraceEntry> {
        input
            .iter()
            .map(|&b| {
                t(
                    "read",
                    vec![
                        ArgMatcher::Fd(sys::constants::STDIN_FILENO),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Bytes(vec![b]),
                )
            })
            .collect()
    }

    pub(crate) fn test_shell() -> Shell {
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
            path_cache: std::collections::HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: std::collections::HashMap::new(),
        }
    }
}
