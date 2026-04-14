use crate::shell::{Shell, ShellError};
use crate::sys;

mod env_file;
mod history;
mod mail;
mod prompt;
mod repl;
pub(crate) mod vi_editing;

pub(crate) use mail::{check_mail, command_is_fc};
pub(crate) use vi_editing as vi;

use history::append_history;
use prompt::{expand_prompt, read_line, write_prompt};
use repl::run_loop;

fn remove_file_bytes(path: &[u8]) {
    let _ = sys::unlink(path);
}

pub fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| shell.diagnostic_syserr(1, &e))?;
    run_loop(shell)
}

pub fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    env_file::load_env_file(shell)
}
#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
pub(super) mod test_support {
    use super::*;
    use crate::shell::ShellOptions;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    pub(crate) use crate::sys::test_support::{
        ArgMatcher, TraceEntry, TraceResult, assert_no_syscalls, run_trace, t,
    };

    pub(crate) fn read_line_trace(input: &[u8]) -> Vec<TraceEntry> {
        input
            .iter()
            .map(|&b| {
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
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
