use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

mod editor;
pub(crate) mod emacs_editing;
mod history;
pub(crate) mod inputrc;
mod mail;
pub(crate) mod prompt;
mod prompt_expand;
mod repl;
mod startup;
mod vi_editing;

fn remove_file_bytes(path: &[u8]) {
    let _ = sys::fs::unlink(path);
}

pub(crate) fn run(shell: &mut Shell) -> Result<i32, ShellError> {
    sys::fd_io::ensure_blocking_read_fd(sys::constants::STDIN_FILENO)
        .map_err(|e| shell.diagnostic_syserr(1, &e))?;
    repl::run_loop(shell)
}

pub(crate) fn load_startup_files(shell: &mut Shell) -> Result<(), ShellError> {
    startup::load_startup_files(shell)
}
#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
pub(super) mod test_support {
    use crate::shell::state::Shell;
    use crate::sys;
    use crate::sys::test_support::{ArgMatcher, TraceEntry, TraceResult, t};

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
        crate::shell::test_support::test_shell()
    }
}
