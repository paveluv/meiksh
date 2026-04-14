#![allow(unused_imports)]

use std::collections::HashMap;

use crate::arena::ByteArena;
use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand;
use crate::shell::{
    BlockingWaitOutcome, FlowSignal, JobState, PendingControl, Shell, ShellError, VarError,
};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};
use crate::sys;

use super::program::check_errexit;
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum ProcessGroupPlan {
    #[cfg_attr(not(test), allow(dead_code))]
    None,
    NewGroup,
    Join(sys::Pid),
}

pub(super) struct SpawnedProcesses {
    pub(super) children: Vec<sys::ChildHandle>,
    pub(super) pgid: Option<sys::Pid>,
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
    let pid = sys::fork_process().map_err(|e| shell.diagnostic_syserr(1, &e))?;
    if pid == 0 {
        if let Some(fd) = stdin_override {
            let _ = sys::duplicate_fd(fd, sys::STDIN_FILENO);
            let _ = sys::close_fd(fd);
        }
        let _ = sys::set_process_group(0, 0);
        if ignore_int_quit {
            let _ = sys::ignore_signal(sys::SIGINT);
            let _ = sys::ignore_signal(sys::SIGQUIT);
        }
        let mut child_shell = shell.clone();
        child_shell.owns_terminal = false;
        child_shell.in_subshell = true;
        child_shell.restore_signals_for_child();
        let _ = child_shell.reset_traps_for_subshell();
        let status = if node.rest.is_empty() {
            execute_pipeline(&mut child_shell, &node.first, false).unwrap_or(1)
        } else {
            execute_and_or(&mut child_shell, node).unwrap_or(1)
        };
        let status = child_shell.run_exit_trap(status).unwrap_or(status);
        sys::exit_process(status as sys::RawFd);
    }
    if let Some(fd) = stdin_override {
        let _ = sys::close_fd(fd);
    }
    let _ = sys::set_process_group(pid, pid);
    Ok(SpawnedProcesses {
        children: vec![sys::ChildHandle {
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
    use crate::exec::test_support::*;
    use crate::shell::Shell;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
}
