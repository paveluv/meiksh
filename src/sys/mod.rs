mod constants;
mod env;
mod error;
mod fd_io;
mod fs;
mod interface;
mod locale;
mod process;
mod time;
mod tty;
mod types;

#[cfg(test)]
pub mod test_support;

pub use constants::*;
pub use env::{
    env_args_os, env_set_var, env_unset_var, env_var, env_vars, getenv, home_dir_for_user, setenv,
};
pub use error::{SysError, SysResult};
pub use fd_io::{
    close_fd, create_pipe, duplicate_fd, duplicate_fd_to_new, ensure_blocking_read_fd, read_fd,
    write_all_fd, write_fd,
};
pub use fs::{
    access_path, canonicalize, change_dir, file_exists, get_cwd, is_directory, is_regular_file,
    lstat_path, open_file, open_for_redirect, read_dir_entries, read_file, read_file_bytes,
    stat_path, unlink,
};
pub use locale::{classify_byte, setup_locale};
pub use process::{
    all_signal_names, current_pid, decode_wait_status, default_signal_action, exec_replace,
    exec_replace_with_env, exit_process, fork_process, format_signal_exit, getrlimit,
    has_pending_signal, has_same_real_and_effective_ids, ignore_signal,
    install_shell_signal_handler, interrupted, parent_pid, query_signal_disposition, send_signal,
    setrlimit, shell_name_from_args, signal_name, spawn_child, supported_trap_signals,
    take_pending_signals, wait_pid, wait_pid_job_status, wait_pid_untraced, wexitstatus,
    wifcontinued, wifsignaled, wifstopped, wstopsig, wtermsig,
};
pub use time::{
    clock_ticks_per_second, current_umask, monotonic_clock_ns, process_times, set_umask,
};
pub use tty::{
    current_foreground_pgrp, get_terminal_attrs, is_interactive_fd, isatty_fd, set_foreground_pgrp,
    set_process_group, set_terminal_attrs,
};
pub use types::{
    ChildExitStatus, ChildHandle, ChildOutput, FdReader, FileModeMask, FileStat, Pid, ProcessTimes,
    RawFd, WaitStatus,
};

#[allow(unused_imports)]
pub(crate) use interface::{
    SystemInterface, default_interface, last_error, record_signal, set_errno, sys_interface,
};
