use libc;

use crate::bstr::ByteWriter;
use crate::sys;

use super::error::ShellError;
use super::state::Shell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JobState {
    Running,
    Stopped(i32),
    Done(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReapedJobState {
    Stopped(i32, Box<[u8]>),
    Done(i32, Box<[u8]>),
    Signaled(i32, Box<[u8]>),
}

#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub(crate) id: usize,
    pub(crate) command: Box<[u8]>,
    pub(crate) pgid: Option<sys::types::Pid>,
    pub(crate) last_pid: Option<sys::types::Pid>,
    pub(crate) last_status: Option<i32>,
    pub(crate) children: Vec<sys::types::ChildHandle>,
    pub(crate) state: JobState,
    pub(crate) saved_termios: Option<libc::termios>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WaitOutcome {
    Exited(i32),
    Signaled(i32),
    Stopped(i32),
    Continued,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockingWaitOutcome {
    Exited(i32),
    Signaled(i32),
    Stopped(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChildWaitResult {
    Exited(i32),
    Stopped(i32),
    Interrupted(i32),
}

pub(super) fn try_wait_child(pid: sys::types::Pid) -> sys::error::SysResult<Option<WaitOutcome>> {
    match sys::process::wait_pid_job_status(pid) {
        Ok(Some(waited)) => {
            if sys::process::wifcontinued(waited.status) {
                Ok(Some(WaitOutcome::Continued))
            } else if sys::process::wifstopped(waited.status) {
                Ok(Some(WaitOutcome::Stopped(sys::process::wstopsig(
                    waited.status,
                ))))
            } else if sys::process::wifsignaled(waited.status) {
                Ok(Some(WaitOutcome::Signaled(sys::process::wtermsig(
                    waited.status,
                ))))
            } else {
                Ok(Some(WaitOutcome::Exited(sys::process::wexitstatus(
                    waited.status,
                ))))
            }
        }
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

impl Shell {
    pub(crate) fn register_background_job(
        &mut self,
        command: Box<[u8]>,
        pgid: Option<sys::types::Pid>,
        children: Vec<sys::types::ChildHandle>,
    ) -> usize {
        let id = self.jobs.last().map(|job| job.id + 1).unwrap_or(1);
        if let Some(last) = children.last() {
            self.last_background = Some(last.pid);
        }
        self.jobs.push(Job {
            id,
            command,
            pgid,
            last_pid: children.last().map(|c| c.pid),
            last_status: None,
            children,
            state: JobState::Running,
            saved_termios: None,
        });
        id
    }

    pub(crate) fn reap_jobs(&mut self) -> Vec<(usize, ReapedJobState)> {
        let mut finished = Vec::new();
        let mut remaining = Vec::new();

        for mut job in self.jobs.drain(..) {
            let mut running = Vec::new();
            let mut any_stopped = false;
            let mut stop_signal = 0i32;
            let mut last_signal: Option<i32> = None;
            for child in job.children.drain(..) {
                match try_wait_child(child.pid) {
                    Ok(Some(WaitOutcome::Exited(code))) => {
                        self.known_pid_statuses.insert(child.pid, code);
                        if job.last_pid == Some(child.pid) {
                            job.last_status = Some(code);
                        }
                    }
                    Ok(Some(WaitOutcome::Signaled(sig))) => {
                        let code = 128 + sig;
                        self.known_pid_statuses.insert(child.pid, code);
                        if job.last_pid == Some(child.pid) {
                            job.last_status = Some(code);
                            last_signal = Some(sig);
                        }
                    }
                    Ok(Some(WaitOutcome::Stopped(sig))) => {
                        if let Ok(Some(WaitOutcome::Continued)) = try_wait_child(child.pid) {
                            running.push(child);
                        } else {
                            any_stopped = true;
                            stop_signal = sig;
                            running.push(child);
                        }
                    }
                    Ok(Some(WaitOutcome::Continued)) => {
                        job.state = JobState::Running;
                        running.push(child);
                    }
                    Ok(None) => running.push(child),
                    Err(_) => {
                        self.known_pid_statuses.insert(child.pid, 1);
                        if job.last_pid == Some(child.pid) {
                            job.last_status = Some(1);
                        }
                    }
                }
            }
            job.children = running;
            if job.children.is_empty() && !matches!(job.state, JobState::Stopped(_)) {
                let final_status = job.last_status.unwrap_or(0);
                self.known_job_statuses.insert(job.id, final_status);
                job.state = JobState::Done(final_status);
                let cmd = job.command.clone();
                if let Some(sig) = last_signal {
                    finished.push((job.id, ReapedJobState::Signaled(sig, cmd)));
                } else {
                    finished.push((job.id, ReapedJobState::Done(final_status, cmd)));
                }
            } else if any_stopped {
                job.state = JobState::Stopped(stop_signal);
                let cmd = job.command.clone();
                finished.push((job.id, ReapedJobState::Stopped(stop_signal, cmd)));
                remaining.push(job);
            } else {
                remaining.push(job);
            }
        }

        self.jobs = remaining;
        finished
    }

    pub(crate) fn wait_for_job(&mut self, id: usize) -> Result<i32, ShellError> {
        if let Some(status) = self.known_job_statuses.remove(&id) {
            self.last_status = status;
            return Ok(status);
        }
        let index = self
            .jobs
            .iter()
            .position(|job| job.id == id)
            .ok_or_else(|| {
                let msg = ByteWriter::new()
                    .bytes(b"job ")
                    .usize_val(id)
                    .bytes(b": not found")
                    .finish();
                self.diagnostic(1, &msg)
            })?;
        let pgid = self.jobs[index].pgid;
        if let Some(ref termios) = self.jobs[index].saved_termios {
            let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, termios);
        }
        let saved_foreground = if self.owns_terminal {
            if let Some(pg) = pgid {
                let _ = sys::tty::set_foreground_pgrp(sys::constants::STDIN_FILENO, pg);
            }
            Some(self.pid)
        } else {
            None
        };
        self.jobs[index].state = JobState::Running;
        self.jobs[index].saved_termios = None;
        let mut status = self.jobs[index].last_status.unwrap_or(0);
        let children: Vec<sys::types::ChildHandle> = self.jobs[index].children.clone();
        for child in &children {
            match self.wait_for_child_blocking(child.pid, true)? {
                BlockingWaitOutcome::Exited(code) => {
                    status = code;
                    let idx = self
                        .jobs
                        .iter()
                        .position(|j| j.id == id)
                        .expect("job vanished");
                    self.known_pid_statuses.insert(child.pid, code);
                    if self.jobs[idx].last_pid == Some(child.pid) {
                        self.jobs[idx].last_status = Some(code);
                    }
                    if let Some(ci) = self.jobs[idx]
                        .children
                        .iter()
                        .position(|c| c.pid == child.pid)
                    {
                        self.jobs[idx].children.remove(ci);
                    }
                }
                BlockingWaitOutcome::Signaled(sig) => {
                    let code = 128 + sig;
                    status = code;
                    let idx = self
                        .jobs
                        .iter()
                        .position(|j| j.id == id)
                        .expect("job vanished");
                    self.known_pid_statuses.insert(child.pid, code);
                    if self.jobs[idx].last_pid == Some(child.pid) {
                        self.jobs[idx].last_status = Some(code);
                    }
                    if let Some(ci) = self.jobs[idx]
                        .children
                        .iter()
                        .position(|c| c.pid == child.pid)
                    {
                        self.jobs[idx].children.remove(ci);
                    }
                }
                BlockingWaitOutcome::Stopped(sig) => {
                    self.restore_foreground(saved_foreground);
                    let idx = self
                        .jobs
                        .iter()
                        .position(|j| j.id == id)
                        .expect("job vanished");
                    self.jobs[idx].state = JobState::Stopped(sig);
                    if self.interactive {
                        self.jobs[idx].saved_termios =
                            sys::tty::get_terminal_attrs(sys::constants::STDIN_FILENO).ok();
                        let msg = ByteWriter::new()
                            .bytes(b"\n[")
                            .usize_val(id)
                            .bytes(b"] Stopped (")
                            .bytes(sys::process::signal_name(sig))
                            .bytes(b")\t")
                            .bytes(&self.jobs[idx].command)
                            .byte(b'\n')
                            .finish();
                        let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
                    }
                    self.last_status = 128 + sig;
                    return Ok(128 + sig);
                }
            }
        }
        self.restore_foreground(saved_foreground);
        let idx = self
            .jobs
            .iter()
            .position(|j| j.id == id)
            .expect("job vanished during wait");
        let removed = self.jobs.remove(idx);
        if let Some(pid) = removed.last_pid {
            self.known_pid_statuses.remove(&pid);
        }
        for child in &children {
            self.known_pid_statuses.remove(&child.pid);
        }
        self.last_status = status;
        Ok(status)
    }

    pub(crate) fn continue_job(&mut self, id: usize, foreground: bool) -> Result<(), ShellError> {
        let idx = self
            .jobs
            .iter()
            .position(|job| job.id == id)
            .ok_or_else(|| {
                let msg = ByteWriter::new()
                    .bytes(b"job ")
                    .usize_val(id)
                    .bytes(b": not found")
                    .finish();
                self.diagnostic(1, &msg)
            })?;
        self.jobs[idx].state = JobState::Running;
        if let Some(pgid) = self.jobs[idx].pgid {
            if foreground && self.owns_terminal {
                let _ = sys::tty::set_foreground_pgrp(sys::constants::STDIN_FILENO, pgid);
            }
            sys::process::send_signal(-pgid, sys::constants::SIGCONT)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
        } else {
            let pids: Vec<sys::types::Pid> =
                self.jobs[idx].children.iter().map(|c| c.pid).collect();
            for pid in pids {
                sys::process::send_signal(pid, sys::constants::SIGCONT)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?;
            }
        }
        Ok(())
    }

    pub(crate) fn source_path(&mut self, path: &[u8]) -> Result<i32, ShellError> {
        let contents = sys::fs::read_file(path).map_err(|e| self.diagnostic_syserr(1, &e))?;
        self.source_depth += 1;
        let result = self.execute_string(&contents);
        self.source_depth -= 1;
        result
    }

    #[cfg(test)]
    pub(crate) fn print_jobs(&mut self) {
        let finished = self.reap_jobs();
        for (id, state) in finished {
            match state {
                ReapedJobState::Done(status, cmd) => {
                    if status == 0 {
                        let msg = ByteWriter::new()
                            .bytes(b"[")
                            .usize_val(id)
                            .bytes(b"] Done\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish();
                        let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &msg);
                    } else {
                        let msg = ByteWriter::new()
                            .bytes(b"[")
                            .usize_val(id)
                            .bytes(b"] Done(")
                            .i32_val(status)
                            .bytes(b")\t")
                            .bytes(&cmd)
                            .byte(b'\n')
                            .finish();
                        let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &msg);
                    }
                }
                ReapedJobState::Signaled(sig, cmd) => {
                    let msg = ByteWriter::new()
                        .bytes(b"[")
                        .usize_val(id)
                        .bytes(b"] Terminated (")
                        .bytes(sys::process::signal_name(sig))
                        .bytes(b")\t")
                        .bytes(&cmd)
                        .byte(b'\n')
                        .finish();
                    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &msg);
                }
                ReapedJobState::Stopped(..) => {}
            }
        }
        for job in &self.jobs {
            if let JobState::Stopped(sig) = job.state {
                let msg = ByteWriter::new()
                    .bytes(b"[")
                    .usize_val(job.id)
                    .bytes(b"] Stopped (")
                    .bytes(sys::process::signal_name(sig))
                    .bytes(b") ")
                    .bytes(&job.command)
                    .byte(b'\n')
                    .finish();
                let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &msg);
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"[")
                    .usize_val(job.id)
                    .bytes(b"] Running ")
                    .bytes(&job.command)
                    .byte(b'\n')
                    .finish();
                let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &msg);
            }
        }
    }
}

impl Shell {
    pub(crate) fn wait_for_job_operand(&mut self, id: usize) -> Result<i32, ShellError> {
        if let Some(status) = self.known_job_statuses.remove(&id) {
            self.remove_known_pids_for_job(id);
            return Ok(status);
        }
        let index = match self.jobs.iter().position(|job| job.id == id) {
            Some(index) => index,
            None => return Ok(127),
        };
        self.wait_on_job_index(index, true)
    }

    pub(crate) fn wait_for_pid_operand(&mut self, pid: sys::types::Pid) -> Result<i32, ShellError> {
        if let Some(status) = self.known_pid_statuses.remove(&pid) {
            return Ok(status);
        }
        let (job_index, child_index) = match self.find_job_child(pid) {
            Some(position) => position,
            None => {
                let msg = ByteWriter::new()
                    .bytes(b"wait: pid ")
                    .i64_val(pid as i64)
                    .bytes(b" is not a child of this shell")
                    .finish();
                self.diagnostic(1, &msg);
                return Ok(127);
            }
        };
        match self.wait_for_child_interruptible(pid) {
            Ok(ChildWaitResult::Exited(status)) => {
                self.record_completed_child(job_index, child_index, pid, status);
                self.known_pid_statuses.remove(&pid);
                Ok(status)
            }
            Ok(ChildWaitResult::Stopped(sig)) => Ok(128 + sig),
            Ok(ChildWaitResult::Interrupted(status)) => Ok(status),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn wait_for_all_jobs(&mut self) -> Result<i32, ShellError> {
        self.wait_was_interrupted = false;
        let ids: Vec<usize> = self.jobs.iter().map(|job| job.id).collect();
        for id in ids {
            let status = self.wait_for_job_operand(id)?;
            if self.wait_was_interrupted {
                return Ok(status);
            }
        }
        self.known_pid_statuses.clear();
        self.known_job_statuses.clear();
        Ok(0)
    }
}

impl Shell {
    pub(super) fn wait_on_job_index(
        &mut self,
        index: usize,
        interruptible: bool,
    ) -> Result<i32, ShellError> {
        let pgid = self.jobs[index].pgid;
        let saved_foreground = self.foreground_handoff(pgid);
        let mut status = self.jobs[index].last_status.unwrap_or(0);
        while !self.jobs[index].children.is_empty() {
            let pid = self.jobs[index].children[0].pid;
            let child_index = 0;
            if interruptible {
                match self.wait_for_child_interruptible(pid) {
                    Ok(ChildWaitResult::Exited(code)) => {
                        status = code;
                        self.record_completed_child(index, child_index, pid, code);
                    }
                    Ok(ChildWaitResult::Stopped(sig)) => {
                        self.restore_foreground(saved_foreground);
                        return Ok(128 + sig);
                    }
                    Ok(ChildWaitResult::Interrupted(int_status)) => {
                        self.restore_foreground(saved_foreground);
                        self.last_status = int_status;
                        self.wait_was_interrupted = true;
                        self.run_pending_traps()?;
                        self.last_status = int_status;
                        return Ok(int_status);
                    }
                    Err(error) => {
                        self.restore_foreground(saved_foreground);
                        return Err(error);
                    }
                }
            } else {
                match self.wait_for_child_blocking(pid, true) {
                    Ok(BlockingWaitOutcome::Exited(code)) => {
                        status = code;
                        self.record_completed_child(index, child_index, pid, code);
                    }
                    Ok(BlockingWaitOutcome::Signaled(sig)) => {
                        status = 128 + sig;
                        self.record_completed_child(index, child_index, pid, 128 + sig);
                    }
                    Ok(BlockingWaitOutcome::Stopped(sig)) => {
                        self.restore_foreground(saved_foreground);
                        return Ok(128 + sig);
                    }
                    Err(error) => {
                        self.restore_foreground(saved_foreground);
                        return Err(error);
                    }
                }
            }
        }
        let job = self.jobs.remove(index);
        let final_status = job.last_status.unwrap_or(status);
        self.restore_foreground(saved_foreground);
        self.last_status = final_status;
        Ok(final_status)
    }

    pub(crate) fn wait_for_child_blocking(
        &mut self,
        pid: sys::types::Pid,
        report_stopped: bool,
    ) -> Result<BlockingWaitOutcome, ShellError> {
        loop {
            match sys::process::wait_pid_untraced(pid, false) {
                Ok(Some(waited)) => {
                    self.run_pending_traps()?;
                    if sys::process::wifstopped(waited.status) {
                        if report_stopped {
                            return Ok(BlockingWaitOutcome::Stopped(sys::process::wstopsig(
                                waited.status,
                            )));
                        }
                        continue;
                    } else if sys::process::wifsignaled(waited.status) {
                        return Ok(BlockingWaitOutcome::Signaled(sys::process::wtermsig(
                            waited.status,
                        )));
                    } else {
                        return Ok(BlockingWaitOutcome::Exited(sys::process::wexitstatus(
                            waited.status,
                        )));
                    }
                }
                Ok(None) => continue,
                Err(error) if sys::process::interrupted(&error) => {
                    self.run_pending_traps()?;
                    continue;
                }
                Err(error) => return Err(self.diagnostic_syserr(1, &error)),
            }
        }
    }

    pub(crate) fn wait_for_child_interruptible(
        &mut self,
        pid: sys::types::Pid,
    ) -> Result<ChildWaitResult, ShellError> {
        loop {
            match sys::process::wait_pid_untraced(pid, false) {
                Ok(Some(waited)) => {
                    return if sys::process::wifstopped(waited.status) {
                        Ok(ChildWaitResult::Stopped(sys::process::wstopsig(
                            waited.status,
                        )))
                    } else if sys::process::wifsignaled(waited.status) {
                        Ok(ChildWaitResult::Exited(
                            128 + sys::process::wtermsig(waited.status),
                        ))
                    } else {
                        Ok(ChildWaitResult::Exited(sys::process::wexitstatus(
                            waited.status,
                        )))
                    };
                }
                Ok(None) => continue,
                Err(error)
                    if sys::process::interrupted(&error)
                        && sys::process::has_pending_signal().is_some() =>
                {
                    let signal =
                        sys::process::has_pending_signal().unwrap_or(sys::constants::SIGINT);
                    return Ok(ChildWaitResult::Interrupted(128 + signal));
                }
                Err(error) if sys::process::interrupted(&error) => continue,
                Err(error) => return Err(self.diagnostic_syserr(1, &error)),
            }
        }
    }

    fn find_job_child(&self, pid: sys::types::Pid) -> Option<(usize, usize)> {
        self.jobs.iter().enumerate().find_map(|(job_index, job)| {
            job.children
                .iter()
                .position(|child| child.pid == pid)
                .map(|child_index| (job_index, child_index))
        })
    }

    fn record_completed_child(
        &mut self,
        job_index: usize,
        child_index: usize,
        pid: sys::types::Pid,
        status: i32,
    ) {
        self.known_pid_statuses.insert(pid, status);
        if self.jobs[job_index].last_pid == Some(pid) {
            self.jobs[job_index].last_status = Some(status);
        }
        self.jobs[job_index].children.remove(child_index);
    }

    fn remove_known_pids_for_job(&mut self, id: usize) {
        let Some(job) = self.jobs.iter().find(|job| job.id == id) else {
            return;
        };
        for child in &job.children {
            self.known_pid_statuses.remove(&child.pid);
        }
    }

    pub(crate) fn current_job_id(&self) -> Option<usize> {
        self.jobs
            .iter()
            .rev()
            .find(|j| matches!(j.state, JobState::Stopped(_)))
            .or_else(|| self.jobs.last())
            .map(|j| j.id)
    }

    pub(crate) fn previous_job_id(&self) -> Option<usize> {
        let current = self.current_job_id();
        let stopped: Vec<&Job> = self
            .jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Stopped(_)))
            .collect();
        if stopped.len() >= 2 {
            return Some(stopped[stopped.len() - 2].id);
        }
        self.jobs
            .iter()
            .rev()
            .find(|j| Some(j.id) != current)
            .map(|j| j.id)
    }

    pub(crate) fn find_job_by_prefix(&self, prefix: &[u8]) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.starts_with(prefix))
            .map(|j| j.id)
    }

    pub(crate) fn find_job_by_substring(&self, substring: &[u8]) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.windows(substring.len()).any(|w| w == substring))
            .map(|j| j.id)
    }

    pub(super) fn foreground_handoff(
        &self,
        pgid: Option<sys::types::Pid>,
    ) -> Option<sys::types::Pid> {
        let Some(pgid) = pgid else {
            return None;
        };
        if !self.owns_terminal {
            return None;
        }
        if !(sys::tty::is_interactive_fd(sys::constants::STDIN_FILENO)
            && sys::tty::is_interactive_fd(sys::constants::STDERR_FILENO))
        {
            return None;
        }
        let Ok(saved) = sys::tty::current_foreground_pgrp(sys::constants::STDIN_FILENO) else {
            return None;
        };
        let _ = sys::tty::set_foreground_pgrp(sys::constants::STDIN_FILENO, pgid);
        Some(saved)
    }

    pub(super) fn restore_foreground(&self, saved_foreground: Option<sys::types::Pid>) {
        if let Some(pgid) = saved_foreground {
            let _ = sys::tty::set_foreground_pgrp(sys::constants::STDIN_FILENO, pgid);
        }
    }
}

#[cfg(test)]
mod tests {

    #![allow(
        clippy::disallowed_types,
        clippy::disallowed_macros,
        clippy::disallowed_methods
    )]

    use super::*;

    use libc;

    use crate::sys;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use crate::trace_entries;

    use crate::shell::test_support::{fake_handle, t_stderr, test_shell};
    use crate::shell::traps::{TrapAction, TrapCondition};
    #[test]
    fn launch_and_wait_for_background_job_updates_state() {
        run_trace(
            trace_entries![
                waitpid(int(1001), _, int(sys::constants::WUNTRACED)) -> status(7),
            ],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"exit 7"[..].into(),
                    None,
                    vec![fake_handle(1001)],
                );
                let status = shell.wait_for_job(id).expect("wait");
                assert_eq!(status, 7);
                assert_eq!(shell.last_status, 7);
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn source_path_runs_script() {
        run_trace(
            trace_entries![
                open("/tmp/source-test.sh", _, _) -> fd(10),
                read(fd(10), _) -> bytes(b"VALUE=42\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let status = shell.source_path(b"/tmp/source-test.sh").expect("source");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), Some(b"42".as_slice()));
            },
        );
    }

    #[test]
    fn reap_jobs_collects_finished_background_jobs() {
        run_trace(trace_entries![waitpid(1001, _) -> status(0),], || {
            let mut shell = test_shell();
            shell.register_background_job(b"exit 0"[..].into(), None, vec![fake_handle(1001)]);
            let finished = shell.reap_jobs();
            assert_eq!(
                finished,
                vec![(1, ReapedJobState::Done(0, b"exit 0"[..].into()))]
            );
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn reap_jobs_handles_try_wait_errors() {
        run_trace(
            trace_entries![waitpid(1001, _) -> err(sys::constants::ECHILD),],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"exit 0"[..].into(),
                    None,
                    vec![fake_handle(1001)],
                );
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(id, ReapedJobState::Done(1, b"exit 0"[..].into()))]
                );
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn continue_job_errors_when_job_missing() {
        run_trace(
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: job 99: not found\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: job 99: not found\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let error = shell.continue_job(99, false).expect_err("missing job");
                assert_eq!(error.exit_status(), 1);

                let error = shell.wait_for_job(99).expect_err("missing job");
                assert_eq!(error.exit_status(), 1);
            },
        );
    }

    #[test]
    fn source_path_errors_when_file_missing() {
        run_trace(
            trace_entries![
                open("/definitely/missing-meiksh-script", _, _) -> err(sys::constants::ENOENT),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: No such file or directory\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let error = shell
                    .source_path(b"/definitely/missing-meiksh-script")
                    .expect_err("missing source");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn print_jobs_shows_done_for_finished_job() {
        run_trace(
            trace_entries![
                waitpid(1001, _) -> status(0),
                waitpid(1002, _) -> status(0),
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[1] Done\tsleep\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"done"[..].into(), None, vec![fake_handle(1001)]);
                shell.reap_jobs();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(1002)]);
                shell.print_jobs();
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn print_jobs_shows_running_for_active_job() {
        run_trace(
            trace_entries![
                waitpid(1003, _) -> pid(0),
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[1] Running sleep\n")) -> auto,
                waitpid(int(1003), _, int(sys::constants::WUNTRACED)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(1003)]);
                shell.print_jobs();
                if let Some(id) = shell.jobs.first().map(|job| job.id) {
                    let _ = shell.wait_for_job(id);
                }
            },
        );
    }

    #[test]
    fn print_jobs_emits_finished_branch_when_job_is_done() {
        run_trace(
            trace_entries![
                waitpid(1001, _) -> status(0),
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[1] Done\tdone\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"done"[..].into(), None, vec![fake_handle(1001)]);
                shell.print_jobs();
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn wait_operands_return_known_statuses_or_127() {
        run_trace(
            trace_entries![write(
                fd(sys::constants::STDERR_FILENO),
                bytes(b"meiksh: wait: pid 999999 is not a child of this shell\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                shell.known_job_statuses.insert(9, 44);
                assert_eq!(shell.wait_for_job_operand(9).expect("known job"), 44);
                shell.known_pid_statuses.insert(55, 12);
                assert_eq!(shell.wait_for_pid_operand(55).expect("known pid"), 12);
                assert_eq!(shell.wait_for_job_operand(999).expect("unknown job"), 127);
                assert_eq!(
                    shell.wait_for_pid_operand(999_999).expect("unknown pid"),
                    127
                );
            },
        );
    }

    #[test]
    fn foreground_handoff_switches_terminal_process_group() {
        run_trace(
            trace_entries![
                isatty(fd(sys::constants::STDIN_FILENO)) -> 1,
                isatty(fd(sys::constants::STDERR_FILENO)) -> 1,
                tcgetpgrp(fd(sys::constants::STDIN_FILENO)) -> pid(77),
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(88)) -> 0,
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(77)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                assert_eq!(shell.foreground_handoff(Some(88)), Some(77));
                shell.restore_foreground(Some(77));
            },
        );
    }

    #[test]
    fn foreground_handoff_returns_none_when_tcgetpgrp_fails() {
        run_trace(
            trace_entries![
                isatty(fd(sys::constants::STDIN_FILENO)) -> 1,
                isatty(fd(sys::constants::STDERR_FILENO)) -> 1,
                tcgetpgrp(fd(sys::constants::STDIN_FILENO)) -> pid(-1),
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                assert_eq!(shell.foreground_handoff(Some(88)), None);
            },
        );
    }

    #[test]
    fn continue_job_sends_sigcont_to_process_group() {
        run_trace(
            trace_entries![kill(int(-11), int(sys::constants::SIGCONT)) -> 0,],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"sleep"[..].into(),
                    Some(11),
                    vec![fake_handle(1001)],
                );
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = JobState::Stopped(sys::constants::SIGTSTP);
                shell.continue_job(id, false).expect("continue pgid job");
                shell.jobs.clear();
            },
        );
    }

    #[test]
    fn wait_for_job_operand_returns_130_on_eintr_with_pending_signal() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGINT), _) -> 0,
                waitpid(int(2001), _, int(sys::constants::WUNTRACED)) -> interrupt(sys::constants::SIGINT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::Command(b":"[..].into())),
                    )
                    .expect("trap");
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2001)]);
                assert_eq!(
                    shell.wait_for_job_operand(1).expect("interrupted wait"),
                    130
                );
                assert_eq!(shell.last_status, 130);
            },
        );
    }

    #[test]
    fn wait_for_child_blocking_retries_on_eintr_and_pid_zero() {
        run_trace(
            trace_entries![
                waitpid(int(99), _, int(sys::constants::WUNTRACED)) -> err(sys::constants::EINTR),
                waitpid(int(99), _, int(sys::constants::WUNTRACED)) -> pid(0),
                waitpid(int(99), _, int(sys::constants::WUNTRACED)) -> status(7),
            ],
            || {
                let mut shell = test_shell();
                assert_eq!(
                    shell
                        .wait_for_child_blocking(99, true)
                        .expect("retry after none"),
                    BlockingWaitOutcome::Exited(7)
                );
            },
        );
    }

    #[test]
    fn wait_operations_fail_on_echild() {
        run_trace(
            trace_entries![
                waitpid(int(2002), _, int(sys::constants::WUNTRACED)) -> err(sys::constants::ECHILD),
                ..vec![t_stderr("meiksh: No child processes")],
                ..trace_entries![waitpid(int(99), _, int(sys::constants::WUNTRACED)) -> err(sys::constants::ECHILD),],
                ..vec![t_stderr("meiksh: No child processes")],
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2002)]);
                assert!(shell.wait_for_job_operand(1).is_err());
                assert!(shell.wait_for_child_blocking(99, true).is_err());
            },
        );
    }

    #[test]
    fn wait_for_pid_operand_handles_interrupt_and_echild() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGINT), _) -> 0,
                waitpid(int(2003), _, int(sys::constants::WUNTRACED)) -> interrupt(sys::constants::SIGINT),
                waitpid(int(2004), _, int(sys::constants::WUNTRACED)) -> err(sys::constants::ECHILD),
                ..vec![t_stderr("meiksh: No child processes")],
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::Command(b":"[..].into())),
                    )
                    .expect("trap");

                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2003)]);
                assert_eq!(
                    shell.wait_for_pid_operand(2003).expect("pid interrupt"),
                    130
                );

                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2004)]);
                assert!(shell.wait_for_pid_operand(2004).is_err());
            },
        );
    }

    #[test]
    fn wait_for_all_jobs_returns_130_on_interrupt() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGINT), _) -> 0,
                waitpid(int(2002), _, int(sys::constants::WUNTRACED)) -> interrupt(sys::constants::SIGINT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::constants::SIGINT),
                        Some(TrapAction::Command(b":"[..].into())),
                    )
                    .expect("trap");
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2002)]);
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2005)]);
                assert_eq!(shell.wait_for_all_jobs().expect("wait all status"), 130);
            },
        );
    }

    #[test]
    fn wait_for_job_operand_consumes_status_second_wait_returns_127() {
        run_trace(
            trace_entries![waitpid(int(3001), _, int(sys::constants::WUNTRACED)) -> status(42),],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"sleep"[..].into(),
                    None,
                    vec![fake_handle(3001)],
                );
                assert_eq!(shell.wait_for_job_operand(id).expect("first wait"), 42);
                assert_eq!(shell.wait_for_job_operand(id).expect("second wait"), 127);
            },
        );
    }

    #[test]
    fn known_job_status_fast_path_avoids_syscalls() {
        let mut shell = test_shell();
        let id = shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2006)]);
        if let Some(job) = shell.jobs.iter().find(|job| job.id == id) {
            if let Some(pid) = job.last_pid {
                shell.known_pid_statuses.insert(pid, 1);
            }
        }
        shell.known_job_statuses.insert(id, 5);
        assert_no_syscalls(|| {
            assert_eq!(shell.wait_for_job_operand(id).expect("known job path"), 5);
        });
    }

    #[test]
    fn try_wait_child_returns_stopped_for_stopped_process() {
        run_trace(
            trace_entries![waitpid(2222, _) -> stopped_sig(sys::constants::SIGTSTP),],
            || {
                let result = try_wait_child(2222).expect("try_wait_child");
                assert_eq!(result, Some(WaitOutcome::Stopped(sys::constants::SIGTSTP)));
            },
        );
    }

    #[test]
    fn known_job_statuses_shortcut_in_wait_for_job() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.known_job_statuses.insert(42, 7);
            let status = shell.wait_for_job(42).expect("wait");
            assert_eq!(status, 7);
            assert_eq!(shell.last_status, 7);
        });
    }

    #[test]
    fn wait_for_job_stopped_handling() {
        run_trace(
            trace_entries![
                waitpid(int(2001), _, int(sys::constants::WUNTRACED)) -> stopped_sig(20),
                tcgetattr(fd(sys::constants::STDIN_FILENO), _) -> 0,
                write(
                    fd(sys::constants::STDERR_FILENO),
                    bytes(b"\n[1] Stopped (SIGTSTP)\tsleep 99\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let id = shell.register_background_job(
                    b"sleep 99"[..].into(),
                    None,
                    vec![fake_handle(2001)],
                );
                let status = shell.wait_for_job(id).expect("wait stopped");
                assert_eq!(status, 128 + 20);
                let job = shell.jobs.iter().find(|j| j.id == id).expect("job exists");
                assert!(matches!(job.state, JobState::Stopped(20)));
                assert!(job.saved_termios.is_some());
            },
        );
    }

    #[test]
    fn wait_for_job_restores_saved_termios() {
        let termios = unsafe { std::mem::zeroed::<libc::termios>() };
        run_trace(
            trace_entries![
                tcsetattr(fd(sys::constants::STDIN_FILENO), _, _) -> 0,
                waitpid(int(2002), _, int(sys::constants::WUNTRACED)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"exit 0"[..].into(),
                    None,
                    vec![fake_handle(2002)],
                );
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].saved_termios = Some(termios);
                let status = shell.wait_for_job(id).expect("wait with termios");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn wait_for_pid_operand_stopped() {
        run_trace(
            trace_entries![waitpid(int(3001), _, int(sys::constants::WUNTRACED)) -> stopped_sig(20),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(3001)]);
                let status = shell.wait_for_pid_operand(3001).expect("wait stopped pid");
                assert_eq!(status, 128 + 20);
            },
        );
    }

    #[test]
    fn wait_on_job_index_stopped() {
        run_trace(
            trace_entries![waitpid(int(4001), _, int(sys::constants::WUNTRACED)) -> stopped_sig(20),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(4001)]);
                let status = shell
                    .wait_on_job_index(0, false)
                    .expect("wait stopped index");
                assert_eq!(status, 128 + 20);
            },
        );
    }

    #[test]
    fn print_jobs_shows_stopped_running_and_done() {
        run_trace(
            trace_entries![
                waitpid(
                    int(3001),
                    _,
                    int((sys::constants::WUNTRACED | sys::constants::WCONTINUED | sys::constants::WNOHANG) as i64),
                ) -> 0,
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[2] Done\texit 0\n")) -> auto,
                write(
                    fd(sys::constants::STDOUT_FILENO),
                    bytes(b"[1] Stopped (SIGTSTP) sleep 99\n"),
                ) -> auto,
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[3] Running sleep 300\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(Job {
                    id: 1,
                    command: b"sleep 99"[..].into(),
                    children: vec![],
                    last_pid: None,
                    last_status: None,
                    pgid: None,
                    state: JobState::Stopped(sys::constants::SIGTSTP),
                    saved_termios: None,
                });
                shell.jobs.push(Job {
                    id: 2,
                    command: b"exit 0"[..].into(),
                    children: vec![],
                    last_pid: None,
                    last_status: None,
                    pgid: None,
                    state: JobState::Done(0),
                    saved_termios: None,
                });
                shell.jobs.push(Job {
                    id: 3,
                    command: b"sleep 300"[..].into(),
                    children: vec![fake_handle(3001)],
                    last_pid: Some(3001),
                    last_status: None,
                    pgid: None,
                    state: JobState::Running,
                    saved_termios: None,
                });
                shell.print_jobs();
            },
        );
    }

    #[test]
    fn wait_on_job_index_blocking_exited() {
        run_trace(
            trace_entries![waitpid(int(5001), _, int(sys::constants::WUNTRACED)) -> status(0),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(5001)]);
                let status = shell
                    .wait_on_job_index(0, false)
                    .expect("wait blocking exited");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn wait_on_job_index_blocking_error() {
        run_trace(
            trace_entries![
                waitpid(int(5002), _, int(sys::constants::WUNTRACED)) -> err(sys::constants::ECHILD),
                ..vec![t_stderr("meiksh: No child processes")],
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(5002)]);
                let result = shell.wait_on_job_index(0, false);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn wait_on_job_index_interruptible_stopped() {
        run_trace(
            trace_entries![
                waitpid(int(5003), _, int(sys::constants::WUNTRACED)) -> stopped_sig(sys::constants::SIGTSTP),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(5003)]);
                let status = shell
                    .wait_on_job_index(0, true)
                    .expect("wait interruptible stopped");
                assert_eq!(status, 128 + sys::constants::SIGTSTP);
            },
        );
    }

    #[test]
    fn wait_for_child_interruptible_retries_on_pid_zero() {
        run_trace(
            trace_entries![
                waitpid(int(5004), _, int(sys::constants::WUNTRACED)) -> pid(0),
                waitpid(int(5004), _, int(sys::constants::WUNTRACED)) -> status(42),
            ],
            || {
                let mut shell = test_shell();
                let result = shell
                    .wait_for_child_interruptible(5004)
                    .expect("retry after none");
                assert_eq!(result, ChildWaitResult::Exited(42));
            },
        );
    }

    #[test]
    fn try_wait_child_returns_continued() {
        run_trace(trace_entries![waitpid(3333, _) -> continued,], || {
            let result = try_wait_child(3333).expect("try_wait_child");
            assert_eq!(result, Some(WaitOutcome::Continued));
        });
    }

    #[test]
    fn try_wait_child_returns_signaled() {
        run_trace(trace_entries![waitpid(3334, _) -> signaled_sig(9),], || {
            let result = try_wait_child(3334).expect("try_wait_child");
            assert_eq!(result, Some(WaitOutcome::Signaled(9)));
        });
    }

    #[test]
    fn reap_jobs_signaled_child() {
        run_trace(trace_entries![waitpid(4001, _) -> signaled_sig(9),], || {
            let mut shell = test_shell();
            shell.register_background_job(b"killed"[..].into(), None, vec![fake_handle(4001)]);
            let finished = shell.reap_jobs();
            assert_eq!(
                finished,
                vec![(1, ReapedJobState::Signaled(9, b"killed"[..].into()))]
            );
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn reap_jobs_continued_child_transitions_to_running() {
        run_trace(trace_entries![waitpid(4002, _) -> continued,], || {
            let mut shell = test_shell();
            let id =
                shell.register_background_job(b"cont"[..].into(), None, vec![fake_handle(4002)]);
            shell.jobs[0].state = JobState::Stopped(sys::constants::SIGTSTP);
            let finished = shell.reap_jobs();
            assert!(finished.is_empty());
            let job = shell.jobs.iter().find(|j| j.id == id).expect("job");
            assert!(matches!(job.state, JobState::Running));
        });
    }

    #[test]
    fn reap_jobs_stopped_then_continued() {
        run_trace(
            trace_entries![
                waitpid(4003, _) -> stopped_sig(sys::constants::SIGTSTP),
                waitpid(4003, _) -> continued,
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    b"stopcont"[..].into(),
                    None,
                    vec![fake_handle(4003)],
                );
                let finished = shell.reap_jobs();
                assert!(finished.is_empty());
                assert!(matches!(shell.jobs[0].state, JobState::Running));
            },
        );
    }

    #[test]
    fn reap_jobs_reports_stopped_when_child_remains_stopped() {
        run_trace(
            trace_entries![
                waitpid(4005, _) -> stopped_sig(sys::constants::SIGTSTP),
                waitpid(4005, _) -> pid(0),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"stopped"[..].into(), None, vec![fake_handle(4005)]);
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(
                        1,
                        ReapedJobState::Stopped(sys::constants::SIGTSTP, b"stopped"[..].into())
                    )]
                );
                assert!(matches!(
                    shell.jobs[0].state,
                    JobState::Stopped(sys::constants::SIGTSTP)
                ));
            },
        );
    }

    #[test]
    fn reap_jobs_signaled_produces_finished_entry() {
        run_trace(
            trace_entries![waitpid(4004, _) -> signaled_sig(15),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"termed"[..].into(), None, vec![fake_handle(4004)]);
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(1, ReapedJobState::Signaled(15, b"termed"[..].into()))]
                );
                assert_eq!(*shell.known_pid_statuses.get(&4004).unwrap(), 128 + 15);
            },
        );
    }

    #[test]
    fn wait_for_job_signaled_child() {
        run_trace(
            trace_entries![waitpid(int(5001), _, int(sys::constants::WUNTRACED)) -> signaled_sig(9),],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"killed"[..].into(),
                    None,
                    vec![fake_handle(5001)],
                );
                let status = shell.wait_for_job(id).expect("wait signaled");
                assert_eq!(status, 128 + 9);
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn wait_for_job_cleanup_removes_known_pids() {
        run_trace(
            trace_entries![waitpid(int(5003), _, int(sys::constants::WUNTRACED)) -> status(42),],
            || {
                let mut shell = test_shell();
                shell.known_pid_statuses.insert(5003, 0);
                let id = shell.register_background_job(
                    b"clean"[..].into(),
                    None,
                    vec![fake_handle(5003)],
                );
                let status = shell.wait_for_job(id).expect("wait");
                assert_eq!(status, 42);
                assert!(!shell.known_pid_statuses.contains_key(&5003));
            },
        );
    }

    #[test]
    fn continue_job_foreground_with_owns_terminal() {
        run_trace(
            trace_entries![
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(6001)) -> 0,
                kill(int(-6001), int(sys::constants::SIGCONT)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                let id = shell.register_background_job(
                    b"fg"[..].into(),
                    Some(6001),
                    vec![fake_handle(6001)],
                );
                shell.jobs[0].state = JobState::Stopped(sys::constants::SIGTSTP);
                shell.continue_job(id, true).expect("continue");
                assert!(matches!(shell.jobs[0].state, JobState::Running));
            },
        );
    }

    #[test]
    fn print_jobs_signaled_and_done_nonzero() {
        run_trace(
            trace_entries![
                waitpid(7001, _) -> signaled_sig(15),
                waitpid(7002, _) -> status(3),
                write(
                    fd(sys::constants::STDOUT_FILENO),
                    bytes(b"[1] Terminated (SIGTERM)\tsig-job\n"),
                ) -> auto,
                write(fd(sys::constants::STDOUT_FILENO), bytes(b"[2] Done(3)\tfail-job\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sig-job"[..].into(), None, vec![fake_handle(7001)]);
                shell.register_background_job(
                    b"fail-job"[..].into(),
                    None,
                    vec![fake_handle(7002)],
                );
                shell.print_jobs();
            },
        );
    }

    #[test]
    fn wait_on_job_index_signaled() {
        run_trace(
            trace_entries![waitpid(int(8001), _, int(sys::constants::WUNTRACED)) -> signaled_sig(11),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"segv"[..].into(), None, vec![fake_handle(8001)]);
                let status = shell.wait_on_job_index(0, false).expect("wait signaled");
                assert_eq!(status, 128 + 11);
            },
        );
    }

    #[test]
    fn wait_for_child_blocking_signaled() {
        run_trace(
            trace_entries![waitpid(int(9002), _, int(sys::constants::WUNTRACED)) -> signaled_sig(6),],
            || {
                let mut shell = test_shell();
                let outcome = shell.wait_for_child_blocking(9002, true).expect("wait");
                assert_eq!(outcome, BlockingWaitOutcome::Signaled(6));
            },
        );
    }

    #[test]
    fn foreground_handoff_with_owns_terminal() {
        run_trace(
            trace_entries![
                isatty(fd(sys::constants::STDIN_FILENO)) -> 1,
                isatty(fd(sys::constants::STDERR_FILENO)) -> 1,
                tcgetpgrp(fd(sys::constants::STDIN_FILENO)) -> pid(1000),
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(2000)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                let saved = shell.foreground_handoff(Some(2000));
                assert_eq!(saved, Some(1000));
            },
        );
    }

    #[test]
    fn foreground_handoff_not_interactive_returns_none() {
        run_trace(
            trace_entries![isatty(fd(sys::constants::STDIN_FILENO)) -> 0,],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                let saved = shell.foreground_handoff(Some(2000));
                assert_eq!(saved, None);
            },
        );
    }

    #[test]
    fn wait_for_job_with_owns_terminal_and_signaled_cleanup() {
        run_trace(
            trace_entries![
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(5010)) -> 0,
                waitpid(int(5010), _, int(sys::constants::WUNTRACED)) -> signaled_sig(9),
                tcsetpgrp(fd(sys::constants::STDIN_FILENO), int(100)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                shell.pid = 100;
                let id = shell.register_background_job(
                    b"killed"[..].into(),
                    Some(5010),
                    vec![fake_handle(5010)],
                );
                shell.known_pid_statuses.insert(5010, 0);
                let status = shell.wait_for_job(id).expect("wait signaled");
                assert_eq!(status, 128 + 9);
                assert!(shell.jobs.is_empty());
                assert!(!shell.known_pid_statuses.contains_key(&5010));
            },
        );
    }

    #[test]
    fn print_jobs_stopped_notification_is_noop() {
        run_trace(
            trace_entries![
                waitpid(7010, _) -> stopped_sig(sys::constants::SIGTSTP),
                waitpid(7010, _) -> pid(0),
                ..vec![t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::constants::STDOUT_FILENO),
                        ArgMatcher::Bytes({
                            let mut v = b"[1] Stopped (".to_vec();
                            v.extend_from_slice(sys::process::signal_name(sys::constants::SIGTSTP));
                            v.extend_from_slice(b") stopped-job\n");
                            v
                        }),
                    ],
                    TraceResult::Auto,
                )],
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(
                    b"stopped-job"[..].into(),
                    None,
                    vec![fake_handle(7010)],
                );
                shell.print_jobs();
            },
        );
    }

    #[test]
    fn wait_for_job_cleanup_iterates_remaining_children() {
        run_trace(
            trace_entries![
                waitpid(int(5020), _, int(sys::constants::WUNTRACED)) -> status(0),
                waitpid(int(5021), _, int(sys::constants::WUNTRACED)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"multi"[..].into(),
                    None,
                    vec![fake_handle(5020), fake_handle(5021)],
                );
                shell.known_pid_statuses.insert(5020, 0);
                shell.known_pid_statuses.insert(5021, 0);
                let status = shell.wait_for_job(id).expect("wait");
                assert_eq!(status, 0);
                assert!(!shell.known_pid_statuses.contains_key(&5020));
                assert!(!shell.known_pid_statuses.contains_key(&5021));
            },
        );
    }

    #[test]
    fn wait_on_job_index_signaled_with_cleanup() {
        run_trace(
            trace_entries![waitpid(int(8010), _, int(sys::constants::WUNTRACED)) -> signaled_sig(11),],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"segv"[..].into(), None, vec![fake_handle(8010)]);
                shell.known_pid_statuses.insert(8010, 0);
                let status = shell.wait_on_job_index(0, false).expect("wait signaled");
                assert_eq!(status, 128 + 11);
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn wait_for_child_blocking_skips_stop_when_not_reporting() {
        run_trace(
            trace_entries![
                waitpid(int(7070), _, int(sys::constants::WUNTRACED)) -> stopped_sig(19),
                waitpid(int(7070), _, int(sys::constants::WUNTRACED)) -> status(42),
            ],
            || {
                let mut shell = test_shell();
                let outcome = shell.wait_for_child_blocking(7070, false).expect("wait");
                assert_eq!(outcome, BlockingWaitOutcome::Exited(42));
            },
        );
    }

    #[test]
    fn continue_job_no_pgid_sends_sigcont_to_each_child() {
        run_trace(
            trace_entries![
                kill(int(2001), int(sys::constants::SIGCONT)) -> 0,
                kill(int(2002), int(sys::constants::SIGCONT)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"multi"[..].into(),
                    None,
                    vec![fake_handle(2001), fake_handle(2002)],
                );
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = JobState::Stopped(sys::constants::SIGTSTP);
                shell.continue_job(id, false).expect("continue no pgid");
                assert!(matches!(shell.jobs[0].state, JobState::Running));
            },
        );
    }

    #[test]
    fn previous_job_id_with_two_stopped_jobs() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(Job {
                id: 1,
                command: b"sleep 10"[..].into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Stopped(sys::constants::SIGTSTP),
                saved_termios: None,
            });
            shell.jobs.push(Job {
                id: 2,
                command: b"sleep 20"[..].into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Stopped(sys::constants::SIGTSTP),
                saved_termios: None,
            });
            assert_eq!(shell.current_job_id(), Some(2));
            assert_eq!(shell.previous_job_id(), Some(1));
        });
    }

    #[test]
    fn find_job_by_prefix_and_substring() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.jobs.push(Job {
                id: 1,
                command: b"sleep 10"[..].into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Running,
                saved_termios: None,
            });
            shell.jobs.push(Job {
                id: 2,
                command: b"echo hello world"[..].into(),
                pgid: None,
                last_pid: None,
                last_status: None,
                children: Vec::new(),
                state: JobState::Running,
                saved_termios: None,
            });

            assert_eq!(shell.find_job_by_prefix(b"sleep"), Some(1));
            assert_eq!(shell.find_job_by_prefix(b"echo"), Some(2));
            assert_eq!(shell.find_job_by_prefix(b"nonexistent"), None);

            assert_eq!(shell.find_job_by_substring(b"hello"), Some(2));
            assert_eq!(shell.find_job_by_substring(b"10"), Some(1));
            assert_eq!(shell.find_job_by_substring(b"xyz"), None);
        });
    }
}
