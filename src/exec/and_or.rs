use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::syntax::ast::{AndOr, LogicalOp};
use crate::sys;

use super::pipeline::{execute_pipeline, spawn_pipeline};
use super::program::check_errexit;

#[derive(Clone, Copy, Debug)]
pub(super) enum ProcessGroupPlan {
    #[cfg_attr(not(test), allow(dead_code))]
    None,
    NewGroup,
    Join(sys::types::Pid),
}

pub(super) struct SpawnedProcesses {
    pub(super) children: Vec<sys::types::ChildHandle>,
    pub(super) pgid: Option<sys::types::Pid>,
}

pub(super) fn execute_and_or(shell: &mut Shell, node: &AndOr) -> Result<i32, ShellError> {
    if node.rest.is_empty() {
        let saved_suppressed = shell.errexit_suppressed;
        if node.first.negated {
            shell.errexit_suppressed = true;
        }
        let status = execute_pipeline(shell, &node.first, false)?;
        shell.errexit_suppressed = saved_suppressed;
        if !node.first.negated {
            check_errexit(shell, status);
        }
        return Ok(status);
    }
    let saved_suppressed = shell.errexit_suppressed;
    shell.errexit_suppressed = true;
    let mut status = execute_pipeline(shell, &node.first, false)?;
    for (i, (op, pipeline)) in node.rest.iter().enumerate() {
        let is_last = i == node.rest.len() - 1;
        if is_last {
            shell.errexit_suppressed = saved_suppressed;
            if pipeline.negated {
                shell.errexit_suppressed = true;
            }
        }
        match op {
            LogicalOp::And if status == 0 => status = execute_pipeline(shell, pipeline, false)?,
            LogicalOp::Or if status != 0 => status = execute_pipeline(shell, pipeline, false)?,
            _ => {}
        }
    }
    shell.errexit_suppressed = saved_suppressed;
    let last_pipeline = node.rest.last().map(|(_, p)| p).unwrap_or(&node.first);
    if !last_pipeline.negated {
        check_errexit(shell, status);
    }
    Ok(status)
}

pub(super) fn spawn_and_or(
    shell: &mut Shell,
    node: &AndOr,
    stdin_override: Option<i32>,
) -> Result<SpawnedProcesses, ShellError> {
    let ignore_int_quit = !shell.options.monitor;
    if node.rest.is_empty() && !ignore_int_quit {
        return spawn_pipeline(shell, &node.first, stdin_override);
    }
    let pid = sys::process::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
    if pid == 0 {
        if let Some(fd) = stdin_override {
            let _ = sys::fd_io::duplicate_fd(fd, sys::constants::STDIN_FILENO);
            let _ = sys::fd_io::close_fd(fd);
        }
        let _ = sys::tty::set_process_group(0, 0);
        if ignore_int_quit {
            let _ = sys::process::ignore_signal(sys::constants::SIGINT);
            let _ = sys::process::ignore_signal(sys::constants::SIGQUIT);
        }
        // Fork already gave us a COW-isolated address space; avoid the
        // userspace `shell.clone()` so `Rc<SharedEnv>` strong-count stays
        // at 1 and later mutations skip `Rc::make_mut`'s deep clone.
        shell.owns_terminal = false;
        shell.in_subshell = true;
        shell.restore_signals_for_child();
        let _ = shell.reset_traps_for_subshell();
        let status = if node.rest.is_empty() {
            execute_pipeline(shell, &node.first, false).unwrap_or(1)
        } else {
            execute_and_or(shell, node).unwrap_or(1)
        };
        let status = shell.run_exit_trap(status).unwrap_or(status);
        sys::process::exit_process(status as sys::types::RawFd);
    }
    if let Some(fd) = stdin_override {
        let _ = sys::fd_io::close_fd(fd);
    }
    let _ = sys::tty::set_process_group(pid, pid);
    Ok(SpawnedProcesses {
        children: vec![sys::types::ChildHandle {
            pid,
            stdout_fd: None,
        }],
        pgid: Some(pid),
    })
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::program::execute_program;
    use crate::exec::test_support::{parse_test, test_shell};
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, Word};

    #[test]
    fn spawn_and_or_monitor_mode_single_pipeline() {
        crate::sys::test_support::run_trace(
            crate::trace_entries![
                fork() -> pid(123), child: [
                    setpgid(int(0), int(0)) -> 0,
                    signal(int(sys::constants::SIGTSTP), _) -> 0,
                    signal(int(sys::constants::SIGTTIN), _) -> 0,
                    signal(int(sys::constants::SIGTTOU), _) -> 0,
                ],
                setpgid(int(123), int(123)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.options.monitor = true;
                let prog = parse_test("true &").expect("parse");
                let status = execute_program(&mut shell, &prog).unwrap();
                assert_eq!(status, 0);
            },
        );
    }
}
