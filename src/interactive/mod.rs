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
#[cfg(test)]
use prompt::expand_prompt_exclamation;
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
mod tests;
