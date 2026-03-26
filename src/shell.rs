use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::builtin::{self, BuiltinOutcome};
use crate::exec;
use crate::expand::{self, ExpandError};
use crate::interactive;
use crate::syntax::{self, Program};
use crate::sys;

pub fn run_from_env() -> i32 {
    match Shell::from_env().and_then(|mut shell| shell.run()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("meiksh: {}", err.display_message());
            err.exit_status()
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShellOptions {
    pub allexport: bool,
    pub command_string: Option<String>,
    pub syntax_check_only: bool,
    pub force_interactive: bool,
    pub noclobber: bool,
    pub noglob: bool,
    pub nounset: bool,
    pub verbose: bool,
    pub script_path: Option<PathBuf>,
    pub shell_name_override: Option<String>,
    pub positional: Vec<String>,
}

const REPORTABLE_OPTION_NAMES: [(&str, char); 6] = [
    ("allexport", 'a'),
    ("noclobber", 'C'),
    ("noglob", 'f'),
    ("noexec", 'n'),
    ("nounset", 'u'),
    ("verbose", 'v'),
];

impl ShellOptions {
    pub fn set_short_option(&mut self, ch: char, enabled: bool) -> Result<(), ShellError> {
        match ch {
            'a' => self.allexport = enabled,
            'C' => self.noclobber = enabled,
            'f' => self.noglob = enabled,
            'i' => self.force_interactive = enabled,
            'n' => self.syntax_check_only = enabled,
            'u' => self.nounset = enabled,
            'v' => self.verbose = enabled,
            _ => return Err(ShellError::with_status(2, format!("invalid option: {ch}"))),
        }
        Ok(())
    }

    pub fn set_named_option(&mut self, name: &str, enabled: bool) -> Result<(), ShellError> {
        let Some((_, letter)) = REPORTABLE_OPTION_NAMES
            .iter()
            .find(|(option_name, _)| *option_name == name)
        else {
            return Err(ShellError::with_status(2, format!("invalid option name: {name}")));
        };
        self.set_short_option(*letter, enabled)
    }

    pub fn reportable_options(&self) -> [(&'static str, bool); 6] {
        [
            ("allexport", self.allexport),
            ("noclobber", self.noclobber),
            ("noglob", self.noglob),
            ("noexec", self.syntax_check_only),
            ("nounset", self.nounset),
            ("verbose", self.verbose),
        ]
    }
}

#[derive(Debug)]
pub struct ShellError {
    pub message: String,
}

const STATUS_PREFIX: &str = "__MEIKSH_STATUS__:";

impl ShellError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn with_status(status: i32, message: impl Into<String>) -> Self {
        Self {
            message: format!("{STATUS_PREFIX}{status}:{}", message.into()),
        }
    }

    fn split_status_metadata(&self) -> Option<(i32, &str)> {
        let encoded = self.message.strip_prefix(STATUS_PREFIX)?;
        let (status, message) = encoded.split_once(':')?;
        status.parse::<i32>().ok().map(|status| (status, message))
    }

    pub fn display_message(&self) -> &str {
        self.split_status_metadata()
            .map(|(_, message)| message)
            .unwrap_or(&self.message)
    }

    pub fn exit_status(&self) -> i32 {
        self.split_status_metadata().map(|(status, _)| status).unwrap_or(1)
    }
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_message())
    }
}

impl std::error::Error for ShellError {}

impl From<sys::SysError> for ShellError {
    fn from(value: sys::SysError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<syntax::ParseError> for ShellError {
    fn from(value: syntax::ParseError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<ExpandError> for ShellError {
    fn from(value: ExpandError) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Clone)]
pub struct Shell {
    pub options: ShellOptions,
    pub shell_name: String,
    pub env: HashMap<String, String>,
    pub exported: BTreeSet<String>,
    pub readonly: BTreeSet<String>,
    pub aliases: HashMap<String, String>,
    pub functions: HashMap<String, crate::syntax::Command>,
    pub positional: Vec<String>,
    pub last_status: i32,
    pub last_background: Option<sys::Pid>,
    pub running: bool,
    pub jobs: Vec<Job>,
    pub known_pid_statuses: HashMap<sys::Pid, i32>,
    pub known_job_statuses: HashMap<usize, i32>,
    pub trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub loop_depth: usize,
    pub function_depth: usize,
    pub pending_control: Option<PendingControl>,
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: usize,
    pub command: String,
    pub pgid: Option<sys::Pid>,
    pub last_pid: Option<sys::Pid>,
    pub last_status: Option<i32>,
    pub children: Vec<sys::ChildHandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrapCondition {
    Exit,
    Signal(sys::Pid),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrapAction {
    Ignore,
    Command(String),
}

pub enum FlowSignal {
    Continue(i32),
    Exit(i32),
}

fn try_wait_child(pid: sys::Pid) -> sys::SysResult<Option<i32>> {
    match sys::wait_pid(pid, true) {
        Ok(Some(waited)) => Ok(Some(sys::decode_wait_status(waited.status))),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingControl {
    Return(i32),
    Break(usize),
    Continue(usize),
}

impl Shell {
    pub fn from_env() -> Result<Self, ShellError> {
        let raw_args: Vec<OsString> = std::env::args_os().collect();
        let args: Vec<String> = raw_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let options = parse_options(&args)?;
        let shell_name = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| sys::shell_name_from_args(&args).to_string());
        let env: HashMap<String, String> = std::env::vars().collect();
        let exported: BTreeSet<String> = env.keys().cloned().collect();
        Ok(Self {
            positional: options.positional.clone(),
            options,
            shell_name,
            env,
            exported,
            readonly: BTreeSet::new(),
            aliases: HashMap::new(),
            functions: HashMap::new(),
            last_status: 0,
            last_background: None,
            running: true,
            jobs: Vec::new(),
            known_pid_statuses: HashMap::new(),
            known_job_statuses: HashMap::new(),
            trap_actions: BTreeMap::new(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn from_args(args: &[&str]) -> Result<Self, ShellError> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let options = parse_options(&args)?;
        let shell_name = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| sys::shell_name_from_args(&args).to_string());
        Ok(Self {
            positional: options.positional.clone(),
            options,
            shell_name,
            env: HashMap::new(),
            exported: BTreeSet::new(),
            readonly: BTreeSet::new(),
            aliases: HashMap::new(),
            functions: HashMap::new(),
            last_status: 0,
            last_background: None,
            running: true,
            jobs: Vec::new(),
            known_pid_statuses: HashMap::new(),
            known_job_statuses: HashMap::new(),
            trap_actions: BTreeMap::new(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        })
    }

    pub fn run(&mut self) -> Result<i32, ShellError> {
        let status = if let Some(command) = self.options.command_string.clone() {
            self.run_source("<command>", &command)?
        } else if let Some(script) = self.options.script_path.clone() {
            let (resolved, contents) = self.load_script_source(&script)?;
            self.run_source(resolved.to_string_lossy().as_ref(), &contents)?
        } else {
            let interactive = self.options.force_interactive
                || (sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO));
            if interactive {
                interactive::run(self)?
            } else {
                self.run_standard_input()?
            }
        };
        self.run_exit_trap(status)
    }

    pub fn is_interactive(&self) -> bool {
        self.options.force_interactive
            || (sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO))
    }

    pub fn run_source(&mut self, _name: &str, source: &str) -> Result<i32, ShellError> {
        self.echo_verbose_input(source);
        self.run_source_buffer(source)
    }

    fn run_source_buffer(&mut self, source: &str) -> Result<i32, ShellError> {
        if self.options.syntax_check_only {
            let _ = syntax::parse_with_aliases(source, &self.aliases)?;
            return Ok(0);
        }
        self.execute_source_incrementally(source)
    }

    pub fn execute_program(&mut self, program: &Program) -> Result<i32, ShellError> {
        let status = exec::execute_program(self, program)?;
        self.last_status = status;
        Ok(status)
    }

    pub fn execute_string(&mut self, source: &str) -> Result<i32, ShellError> {
        self.echo_verbose_input(source);
        self.execute_source_incrementally(source)
    }

    fn run_standard_input(&mut self) -> Result<i32, ShellError> {
        sys::ensure_blocking_read_fd(sys::STDIN_FILENO)?;
        let mut status = 0;
        let mut source = String::new();
        let mut line_bytes = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let count = sys::read_fd(sys::STDIN_FILENO, &mut byte)?;
            if count == 0 {
                if !line_bytes.is_empty() {
                    let chunk = decode_stdin_chunk(std::mem::take(&mut line_bytes))?;
                    self.echo_verbose_input(&chunk);
                    source.push_str(&chunk);
                }
                break;
            }
            line_bytes.push(byte[0]);
            if byte[0] == b'\n' {
                let chunk = decode_stdin_chunk(std::mem::take(&mut line_bytes))?;
                self.echo_verbose_input(&chunk);
                source.push_str(&chunk);
                if let Some(executed_status) = self.maybe_run_stdin_source(&mut source, false)? {
                    status = executed_status;
                    if !self.running || self.has_pending_control() {
                        return Ok(status);
                    }
                }
            }
        }

        if let Some(executed_status) = self.maybe_run_stdin_source(&mut source, true)? {
            status = executed_status;
        }
        Ok(status)
    }

    fn execute_source_incrementally(&mut self, source: &str) -> Result<i32, ShellError> {
        let mut session = syntax::ParseSession::new(source)?;
        let mut status = 0;
        self.run_pending_traps()?;
        while let Some(item) = session.next_item(&self.aliases)? {
            status = self.execute_program(&Program { items: vec![item] })?;
            self.run_pending_traps()?;
            if !self.running || self.has_pending_control() {
                break;
            }
        }
        Ok(status)
    }

    fn maybe_run_stdin_source(&mut self, source: &mut String, eof: bool) -> Result<Option<i32>, ShellError> {
        if source.is_empty() {
            return Ok(None);
        }
        match syntax::parse_with_aliases(source, &self.aliases) {
            Ok(_) => {
                let buffered = std::mem::take(source);
                self.run_source_buffer(&buffered).map(Some)
            }
            Err(error) if !eof && stdin_parse_error_requires_more_input(&error) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn echo_verbose_input(&self, source: &str) {
        if self.options.verbose && !source.is_empty() {
            eprint!("{source}");
        }
    }

    pub fn capture_output(&mut self, source: &str) -> Result<String, ShellError> {
        let (read_fd, write_fd) = sys::create_pipe()?;
        let pid = sys::fork_process()?;
        if pid == 0 {
            let _ = sys::close_fd(read_fd);
            let _ = sys::duplicate_fd(write_fd, sys::STDOUT_FILENO);
            let _ = sys::close_fd(write_fd);
            let mut child_shell = self.clone();
            let status = child_shell.execute_string(source).unwrap_or(1);
            sys::exit_process(status as libc::c_int);
        }
        sys::close_fd(write_fd)?;
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = sys::read_fd(read_fd, &mut buf)?;
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }
        sys::close_fd(read_fd)?;
        let ws = sys::wait_pid(pid, false)?.expect("child status");
        let status = sys::decode_wait_status(ws.status);
        let text = String::from_utf8_lossy(&output).into_owned();
        if status == 0 {
            Ok(text)
        } else {
            let trimmed = text.trim().to_string();
            Err(ShellError {
                message: if trimmed.is_empty() {
                    format!("command substitution failed with status {status}")
                } else {
                    trimmed
                },
            })
        }
    }

    pub fn env_for_child(&self) -> HashMap<String, String> {
        self.exported
            .iter()
            .filter_map(|name| self.env.get(name).map(|value| (name.clone(), value.clone())))
            .collect()
    }

    pub fn get_var(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned()
    }

    pub fn set_var(&mut self, name: &str, value: String) -> Result<(), ShellError> {
        if self.readonly.contains(name) {
            return Err(ShellError {
                message: format!("{name}: readonly variable"),
            });
        }
        self.env.insert(name.to_string(), value);
        if self.options.allexport {
            self.exported.insert(name.to_string());
        }
        Ok(())
    }

    pub fn export_var(&mut self, name: &str, value: Option<String>) -> Result<(), ShellError> {
        if let Some(value) = value {
            self.set_var(name, value)?;
        }
        self.exported.insert(name.to_string());
        Ok(())
    }

    pub fn mark_readonly(&mut self, name: &str) {
        self.readonly.insert(name.to_string());
    }

    pub fn unset_var(&mut self, name: &str) -> Result<(), ShellError> {
        if self.readonly.contains(name) {
            return Err(ShellError {
                message: format!("{name}: readonly variable"),
            });
        }
        self.env.remove(name);
        self.exported.remove(name);
        Ok(())
    }

    pub fn set_positional(&mut self, values: Vec<String>) {
        self.positional = values;
    }

    pub fn launch_background_job(
        &mut self,
        command: String,
        pgid: Option<sys::Pid>,
        children: Vec<sys::ChildHandle>,
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
        });
        id
    }

    pub fn reap_jobs(&mut self) -> Vec<(usize, i32)> {
        let mut finished = Vec::new();
        let mut remaining = Vec::new();

        for mut job in self.jobs.drain(..) {
            let mut running = Vec::new();
            for child in job.children.drain(..) {
                match try_wait_child(child.pid) {
                    Ok(Some(code)) => {
                        self.known_pid_statuses.insert(child.pid, code);
                        if job.last_pid == Some(child.pid) {
                            job.last_status = Some(code);
                        }
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
            if job.children.is_empty() {
                let final_status = job.last_status.unwrap_or(0);
                self.known_job_statuses.insert(job.id, final_status);
                finished.push((job.id, final_status));
            } else {
                remaining.push(job);
            }
        }

        self.jobs = remaining;
        finished
    }

    pub fn wait_for_job(&mut self, id: usize) -> Result<i32, ShellError> {
        if let Some(status) = self.known_job_statuses.remove(&id) {
            self.last_status = status;
            return Ok(status);
        }
        let index = self
            .jobs
            .iter()
            .position(|job| job.id == id)
            .ok_or_else(|| ShellError {
                message: format!("job {id}: not found"),
            })?;
        let pgid = self.jobs[index].pgid;
        let saved_foreground = self.foreground_handoff(pgid);
        let mut job = self.jobs.remove(index);
        let mut status = job.last_status.unwrap_or(0);
        for child in &job.children {
            let waited = self.wait_for_child_pid(child.pid, false)?;
            status = waited;
            self.known_pid_statuses.insert(child.pid, waited);
            if job.last_pid == Some(child.pid) {
                job.last_status = Some(waited);
            }
        }
        self.restore_foreground(saved_foreground);
        self.last_status = status;
        Ok(status)
    }

    pub fn continue_job(&mut self, id: usize) -> Result<(), ShellError> {
        let job = self.jobs.iter().find(|job| job.id == id).ok_or_else(|| ShellError {
            message: format!("job {id}: not found"),
        })?;
        if let Some(pgid) = job.pgid {
            sys::send_signal(-pgid, sys::SIGCONT)?;
        } else {
            for child in &job.children {
                sys::send_signal(child.pid, sys::SIGCONT)?;
            }
        }
        Ok(())
    }

    pub fn source_path(&mut self, path: &Path) -> Result<i32, ShellError> {
        let contents = sys::read_file(&path.display().to_string())?;
        self.execute_string(&contents)
    }

    fn load_script_source(&self, script: &Path) -> Result<(PathBuf, String), ShellError> {
        let resolved = resolve_script_path(self, script).ok_or_else(|| {
            ShellError::with_status(127, format!("{}: not found", script.display()))
        })?;
        let contents = sys::read_file(&resolved.display().to_string())
            .map_err(|error| classify_script_read_error(&resolved, error))?;
        Ok((resolved, contents))
    }

    pub fn print_jobs(&mut self) {
        let finished = self.reap_jobs();
        for (id, status) in finished {
            println!("[{id}] Done {status}");
        }
        for job in &self.jobs {
            println!("[{}] Running {}", job.id, job.command);
        }
    }

    pub fn run_builtin(
        &mut self,
        argv: &[String],
        assignments: &[(String, String)],
    ) -> Result<FlowSignal, ShellError> {
        for (name, value) in assignments {
            self.set_var(name, value.clone())?;
        }
        match builtin::run(self, argv)? {
            BuiltinOutcome::Status(status) => Ok(FlowSignal::Continue(status)),
            BuiltinOutcome::Exit(status) => Ok(FlowSignal::Exit(status)),
            BuiltinOutcome::Return(status) => {
                self.pending_control = Some(PendingControl::Return(status));
                Ok(FlowSignal::Continue(status))
            }
            BuiltinOutcome::Break(levels) => {
                self.pending_control = Some(PendingControl::Break(levels));
                Ok(FlowSignal::Continue(0))
            }
            BuiltinOutcome::Continue(levels) => {
                self.pending_control = Some(PendingControl::Continue(levels));
                Ok(FlowSignal::Continue(0))
            }
        }
    }

    pub fn has_pending_control(&self) -> bool {
        self.pending_control.is_some()
    }

    pub fn trap_action(&self, condition: TrapCondition) -> Option<&TrapAction> {
        self.trap_actions.get(&condition)
    }

    pub fn set_trap(&mut self, condition: TrapCondition, action: Option<TrapAction>) -> Result<(), ShellError> {
        if let TrapCondition::Signal(signal) = condition {
            match action.as_ref() {
                Some(TrapAction::Ignore) => sys::ignore_signal(signal)?,
                Some(TrapAction::Command(_)) => sys::install_shell_signal_handler(signal)?,
                None => sys::default_signal_action(signal)?,
            }
        }
        match action {
            Some(action) => {
                self.trap_actions.insert(condition, action);
            }
            None => {
                self.trap_actions.remove(&condition);
            }
        }
        Ok(())
    }

    pub fn wait_for_job_operand(&mut self, id: usize) -> Result<i32, ShellError> {
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

    pub fn wait_for_pid_operand(&mut self, pid: sys::Pid) -> Result<i32, ShellError> {
        if let Some(status) = self.known_pid_statuses.remove(&pid) {
            return Ok(status);
        }
        let (job_index, child_index) = match self.find_job_child(pid) {
            Some(position) => position,
            None => return Ok(127),
        };
        match self.wait_for_child_pid(pid, true) {
            Ok(status) => {
                self.record_completed_child(job_index, child_index, pid, status);
                Ok(status)
            }
            Err(error) if error.message.starts_with("wait interrupted:") => {
                let status = error
                    .message
                    .split(':')
                    .nth(1)
                    .and_then(|value| value.parse::<i32>().ok())
                    .unwrap_or(130);
                Ok(status)
            }
            Err(error) => Err(error),
        }
    }

    pub fn wait_for_all_jobs(&mut self) -> Result<i32, ShellError> {
        let ids: Vec<usize> = self.jobs.iter().map(|job| job.id).collect();
        for id in ids {
            let status = self.wait_for_job_operand(id)?;
            if status > 128 && sys::has_pending_signal().is_none() {
                return Ok(status);
            }
        }
        self.known_pid_statuses.clear();
        self.known_job_statuses.clear();
        Ok(0)
    }

    pub fn run_pending_traps(&mut self) -> Result<(), ShellError> {
        for signal in sys::take_pending_signals() {
            let Some(TrapAction::Command(action)) = self.trap_actions.get(&TrapCondition::Signal(signal)).cloned() else {
                continue;
            };
            self.execute_trap_action(&action, self.last_status)?;
            if !self.running {
                break;
            }
        }
        Ok(())
    }

    fn run_exit_trap(&mut self, status: i32) -> Result<i32, ShellError> {
        let Some(TrapAction::Command(action)) = self.trap_actions.get(&TrapCondition::Exit).cloned() else {
            self.last_status = status;
            return Ok(status);
        };
        self.execute_trap_action(&action, status)
    }

    fn execute_trap_action(&mut self, action: &str, preserved_status: i32) -> Result<i32, ShellError> {
        let was_running = self.running;
        self.running = true;
        self.last_status = preserved_status;
        let status = self.execute_string(action)?;
        if self.running {
            self.running = was_running;
            self.last_status = preserved_status;
            Ok(preserved_status)
        } else {
            self.last_status = status;
            Ok(status)
        }
    }

    fn wait_on_job_index(&mut self, index: usize, interruptible: bool) -> Result<i32, ShellError> {
        let pgid = self.jobs[index].pgid;
        let saved_foreground = self.foreground_handoff(pgid);
        let mut status = self.jobs[index].last_status.unwrap_or(0);
        while !self.jobs[index].children.is_empty() {
            let pid = self.jobs[index].children[0].pid;
            let child_index = 0;
            match self.wait_for_child_pid(pid, interruptible) {
                Ok(code) => {
                    status = code;
                    self.record_completed_child(index, child_index, pid, code);
                }
                Err(error) => {
                    self.restore_foreground(saved_foreground);
                    if let Some(interrupted_status) = self.consume_wait_interrupt(&error)? {
                        return Ok(interrupted_status);
                    }
                    return Err(error);
                }
            }
        }
        let job = self.jobs.remove(index);
        let final_status = job.last_status.unwrap_or(status);
        self.known_job_statuses.insert(job.id, final_status);
        self.known_job_statuses.remove(&job.id);
        self.restore_foreground(saved_foreground);
        self.last_status = final_status;
        Ok(final_status)
    }

    fn consume_wait_interrupt(&mut self, error: &ShellError) -> Result<Option<i32>, ShellError> {
        if !error.message.starts_with("wait interrupted:") {
            return Ok(None);
        }
        let status = error
            .message
            .split(':')
            .nth(1)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(130);
        self.last_status = status;
        self.run_pending_traps()?;
        self.last_status = status;
        Ok(Some(status))
    }

    pub fn wait_for_child_pid(&mut self, pid: sys::Pid, interruptible: bool) -> Result<i32, ShellError> {
        loop {
            match sys::wait_pid(pid, false) {
                Ok(Some(waited)) => return Ok(sys::decode_wait_status(waited.status)),
                Ok(None) => continue,
                Err(error) if interruptible && sys::interrupted(&error) && sys::has_pending_signal().is_some() => {
                    let signal = sys::has_pending_signal().unwrap_or(sys::SIGINT);
                    return Err(ShellError {
                        message: format!("wait interrupted:{}", 128 + signal),
                    });
                }
                Err(error) if sys::interrupted(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn find_job_child(&self, pid: sys::Pid) -> Option<(usize, usize)> {
        self.jobs.iter().enumerate().find_map(|(job_index, job)| {
            job.children
                .iter()
                .position(|child| child.pid == pid)
                .map(|child_index| (job_index, child_index))
        })
    }

    fn record_completed_child(&mut self, job_index: usize, child_index: usize, pid: sys::Pid, status: i32) {
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

    fn foreground_handoff(&self, pgid: Option<sys::Pid>) -> Option<sys::Pid> {
        let Some(pgid) = pgid else {
            return None;
        };
        if !(sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO)) {
            return None;
        }
        let Ok(saved) = sys::current_foreground_pgrp(sys::STDIN_FILENO) else {
            return None;
        };
        let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
        Some(saved)
    }

    fn restore_foreground(&self, saved_foreground: Option<sys::Pid>) {
        if let Some(pgid) = saved_foreground {
            let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
        }
    }
}

impl expand::Context for Shell {
    fn env_var(&self, name: &str) -> Option<String> {
        self.get_var(name)
    }

    fn special_param(&self, name: char) -> Option<String> {
        match name {
            '?' => Some(self.last_status.to_string()),
            '$' => Some(sys::current_pid().to_string()),
            '!' => self.last_background.map(|pid| pid.to_string()),
            '#' => Some(self.positional.len().to_string()),
            '-' => Some(self.active_option_flags()),
            '*' => Some(self.positional.join(
                &self
                    .get_var("IFS")
                    .unwrap_or_else(|| " \t\n".to_string())
                    .chars()
                    .next()
                    .map(|ch| ch.to_string())
                    .unwrap_or_default(),
            )),
            '@' => Some(self.positional.join(" ")),
            '0' => Some(self.shell_name.clone()),
            digit if digit.is_ascii_digit() => {
                let index = digit.to_digit(10)? as usize;
                self.positional.get(index.saturating_sub(1)).cloned()
            }
            _ => None,
        }
    }

    fn positional_param(&self, index: usize) -> Option<String> {
        if index == 0 {
            Some(self.shell_name.clone())
        } else {
            self.positional.get(index - 1).cloned()
        }
    }

    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
        self.set_var(name, value).map_err(|err| ExpandError {
            message: err.message,
        })
    }

    fn pathname_expansion_enabled(&self) -> bool {
        !self.options.noglob
    }

    fn nounset_enabled(&self) -> bool {
        self.options.nounset
    }

    fn shell_name(&self) -> &str {
        &self.shell_name
    }

    fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError> {
        self.capture_output(command).map_err(|err| ExpandError {
            message: err.message,
        })
    }
}

fn parse_options(args: &[String]) -> Result<ShellOptions, ShellError> {
    let mut options = ShellOptions::default();
    let mut index = 1usize;

    while let Some(arg) = args.get(index) {
        if arg == "-c" {
            let command = args.get(index + 1).ok_or_else(|| ShellError {
                message: ShellError::with_status(2, "-c requires an argument").message,
            })?;
            options.command_string = Some(command.clone());
            options.shell_name_override = args.get(index + 2).cloned();
            options.positional = args.iter().skip(index + 3).cloned().collect();
            return Ok(options);
        }
        if arg == "-n" {
            options.syntax_check_only = true;
            index += 1;
            continue;
        }
        if arg == "-o" || arg == "+o" {
            let enabled = arg == "-o";
            let name = args
                .get(index + 1)
                .ok_or_else(|| ShellError::with_status(2, format!("{arg} requires an argument")))?;
            options.set_named_option(name, enabled)?;
            index += 2;
            continue;
        }
        if arg == "-i" {
            options.force_interactive = true;
            index += 1;
            continue;
        }
        if arg == "-s" {
            options.positional = args.iter().skip(index + 1).cloned().collect();
            return Ok(options);
        }
        if arg == "-" {
            index += 1;
            continue;
        }
        if arg == "--" {
            index += 1;
            break;
        }
        if (arg.starts_with('-') || arg.starts_with('+')) && arg != "-" && arg != "+" {
            let enabled = arg.starts_with('-');
            let mut read_stdin = false;
            for ch in arg[1..].chars() {
                match ch {
                    's' if enabled => read_stdin = true,
                    _ => options.set_short_option(ch, enabled)?,
                }
            }
            if read_stdin {
                options.positional = args.iter().skip(index + 1).cloned().collect();
                return Ok(options);
            }
            index += 1;
            continue;
        }
        options.script_path = Some(PathBuf::from(arg));
        options.shell_name_override = Some(arg.clone());
        options.positional = args.iter().skip(index + 1).cloned().collect();
        return Ok(options);
    }

    if index < args.len() {
        options.positional = args.iter().skip(index).cloned().collect();
    }

    Ok(options)
}

impl Shell {
    fn active_option_flags(&self) -> String {
        let mut flags = String::new();
        if self.options.allexport {
            flags.push('a');
        }
        if self.options.noclobber {
            flags.push('C');
        }
        if self.options.noglob {
            flags.push('f');
        }
        if self.is_interactive() {
            flags.push('i');
        }
        if self.options.syntax_check_only {
            flags.push('n');
        }
        if self.options.nounset {
            flags.push('u');
        }
        if self.options.verbose {
            flags.push('v');
        }
        if self.options.command_string.is_some() {
            flags.push('c');
        } else if self.options.script_path.is_none() {
            flags.push('s');
        }
        flags
    }
}

fn resolve_script_path(shell: &Shell, script: &Path) -> Option<PathBuf> {
    if script.is_absolute() || script.to_string_lossy().contains('/') {
        return Some(script.to_path_buf());
    }

    let cwd_path = PathBuf::from(script);
    if sys::file_exists(&cwd_path.display().to_string()) {
        return Some(cwd_path);
    }

    search_script_path(shell, script.to_str()?)
}

fn search_script_path(shell: &Shell, name: &str) -> Option<PathBuf> {
    let path_env = shell
        .get_var("PATH")
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    for dir in path_env.split(':') {
        let base = if dir.is_empty() { PathBuf::from(".") } else { PathBuf::from(dir) };
        let candidate = base.join(name);
        if executable_regular_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn executable_regular_file(path: &Path) -> bool {
    sys::stat_path(&path.display().to_string())
        .map(|stat| stat.is_regular_file() && stat.is_executable())
        .unwrap_or(false)
}

fn decode_stdin_chunk(bytes: Vec<u8>) -> sys::SysResult<String> {
    String::from_utf8(bytes).map_err(|_| sys::SysError::Errno(libc::EILSEQ))
}

fn stdin_parse_error_requires_more_input(error: &syntax::ParseError) -> bool {
    matches!(
        error.message.as_str(),
        "unterminated single quote"
            | "unterminated double quote"
            | "unterminated here-document"
            | "expected command"
            | "expected for loop variable name"
            | "expected for loop word list"
            | "expected case word"
            | "expected 'in'"
            | "expected case pattern"
            | "expected ';;' or 'esac'"
            | "expected redirection target"
            | "missing here-document body"
            | "unexpected end of tokens"
            | "expected 'then'"
            | "expected 'fi'"
            | "expected 'do'"
            | "expected 'done'"
            | "expected 'esac'"
            | "expected ')' to close subshell"
            | "expected '}'"
    )
}

fn classify_script_read_error(path: &Path, error: sys::SysError) -> ShellError {
    if error.is_enoent() {
        ShellError::with_status(127, format!("{}: not found", path.display()))
    } else {
        ShellError::with_status(128, format!("{}: {}", path.display(), error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::sys::test_support::{self, TraceResult, ArgMatcher, run_trace, t, assert_no_syscalls};

    fn fake_handle(pid: sys::Pid) -> sys::ChildHandle {
        sys::ChildHandle { pid, stdout_fd: None }
    }

    fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: "meiksh".to_string(),
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
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        }
    }

    #[test]
    fn parse_options_handles_command_script_and_errors() {
        assert_no_syscalls(|| {
            let options = parse_options(&["meiksh".into(), "-c".into(), "echo ok".into(), "name".into(), "arg".into()])
                .expect("parse");
            assert_eq!(options.command_string.as_deref(), Some("echo ok"));
            assert_eq!(options.shell_name_override.as_deref(), Some("name"));
            assert_eq!(options.positional, vec!["arg".to_string()]);

            let options =
                parse_options(&["meiksh".into(), "-n".into(), "-i".into(), "-f".into(), "script.sh".into(), "a".into()])
                .expect("parse");
            assert!(options.syntax_check_only);
            assert!(options.force_interactive);
            assert!(options.noglob);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));
            assert_eq!(options.positional, vec!["a".to_string()]);

            let options = parse_options(&["meiksh".into(), "-s".into(), "arg1".into(), "arg2".into()]).expect("parse -s");
            assert_eq!(options.script_path, None);
            assert_eq!(options.positional, vec!["arg1".to_string(), "arg2".to_string()]);

            let options = parse_options(&["meiksh".into(), "-is".into(), "arg".into()]).expect("parse -is");
            assert!(options.force_interactive);
            assert_eq!(options.positional, vec!["arg".to_string()]);

            let options = parse_options(
                &[
                    "meiksh".into(),
                    "-a".into(),
                    "-u".into(),
                    "-o".into(),
                    "noglob".into(),
                    "-v".into(),
                    "script.sh".into(),
                ],
            )
            .expect("parse -a -u -o noglob -v");
            assert!(options.allexport);
            assert!(options.nounset);
            assert!(options.noglob);
            assert!(options.verbose);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

            let error = parse_options(&["meiksh".into(), "-c".into()]).expect_err("missing arg");
            assert_eq!(error.display_message(), "-c requires an argument");
            assert_eq!(error.exit_status(), 2);

            let error = parse_options(&["meiksh".into(), "-o".into()]).expect_err("missing -o arg");
            assert_eq!(error.display_message(), "-o requires an argument");
            assert_eq!(error.exit_status(), 2);

            let error = parse_options(&["meiksh".into(), "-o".into(), "pipefail".into()]).expect_err("bad -o name");
            assert_eq!(error.display_message(), "invalid option name: pipefail");
            assert_eq!(error.exit_status(), 2);
        });
    }

    #[test]
    fn env_for_child_filters_exported_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("A".into(), "1".into());
            shell.env.insert("B".into(), "2".into());
            shell.exported.insert("A".into());
            let env = shell.env_for_child();
            assert_eq!(env.get("A").map(String::as_str), Some("1"));
            assert!(!env.contains_key("B"));

            shell.options.allexport = true;
            shell.set_var("B", "3".into()).expect("allexport set");
            let env = shell.env_for_child();
            assert_eq!(env.get("B").map(String::as_str), Some("3"));
        });
    }

    #[test]
    fn readonly_variables_reject_mutation_and_unset() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.set_var("NAME", "value".into()).expect("set");
            shell.mark_readonly("NAME");
            let set_error = shell.set_var("NAME", "new".into()).expect_err("readonly");
            assert_eq!(set_error.message, "NAME: readonly variable");
            let unset_error = shell.unset_var("NAME").expect_err("readonly");
            assert_eq!(unset_error.message, "NAME: readonly variable");
        });
    }

    #[test]
    fn special_parameters_reflect_shell_state() {
        run_trace(vec![
            t("getpid", vec![], TraceResult::Pid(12345)),
            // is_interactive() check from active_option_flags() via special_param('-')
            t("isatty", vec![ArgMatcher::Fd(sys::STDIN_FILENO)], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            shell.positional = vec!["first".into(), "second".into()];
            shell.last_status = 17;
            shell.last_background = Some(42);
            shell.options.allexport = true;
            shell.options.noclobber = true;
            shell.options.command_string = Some("printf ok".into());
            assert_eq!(expand::Context::special_param(&shell, '?').as_deref(), Some("17"));
            assert_eq!(expand::Context::special_param(&shell, '$').as_deref(), Some("12345"));
            assert_eq!(expand::Context::special_param(&shell, '#').as_deref(), Some("2"));
            assert_eq!(expand::Context::special_param(&shell, '!').as_deref(), Some("42"));
            assert_eq!(expand::Context::special_param(&shell, '-').as_deref(), Some("aCc"));
            assert_eq!(expand::Context::special_param(&shell, '*').as_deref(), Some("first second"));
            assert_eq!(expand::Context::special_param(&shell, '@').as_deref(), Some("first second"));
            assert_eq!(expand::Context::special_param(&shell, '1').as_deref(), Some("first"));
            assert_eq!(expand::Context::special_param(&shell, '0').as_deref(), Some("meiksh"));
            assert_eq!(expand::Context::special_param(&shell, '9'), None);
            assert_eq!(expand::Context::special_param(&shell, 'x'), None);
        });
    }

    #[test]
    fn launch_and_wait_for_background_job_updates_state() {
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Int(0)],
                TraceResult::Status(7)),
        ], || {
            let mut shell = test_shell();
            let id = shell.launch_background_job("exit 7".into(), None, vec![fake_handle(1001)]);
            let status = shell.wait_for_job(id).expect("wait");
            assert_eq!(status, 7);
            assert_eq!(shell.last_status, 7);
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn source_path_runs_script() {
        run_trace(vec![
            t("open", vec![ArgMatcher::Str("/tmp/source-test.sh".into()), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Fd(10)),
            t("read", vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                TraceResult::Bytes(b"VALUE=42\n".to_vec())),
            t("read", vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                TraceResult::Int(0)),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            let status = shell.source_path(Path::new("/tmp/source-test.sh")).expect("source");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("42"));
        });
    }

    #[test]
    fn export_without_value_marks_variable_exported() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("NAME".into(), "value".into());
            shell.export_var("NAME", None).expect("export");
            assert!(shell.exported.contains("NAME"));
        });
    }

    #[test]
    fn run_source_syntax_only_parses_without_executing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.syntax_check_only = true;
            let status = shell.run_source("<test>", "echo ok").expect("syntax only");
            assert_eq!(status, 0);
            assert_eq!(shell.last_status, 0);
        });
    }

    #[test]
    fn reap_jobs_collects_finished_background_jobs() {
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0)),
        ], || {
            let mut shell = test_shell();
            shell.launch_background_job("exit 0".into(), None, vec![fake_handle(1001)]);
            let finished = shell.reap_jobs();
            assert_eq!(finished, vec![(1, 0)]);
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn run_builtin_returns_correct_flow_signals() {
        run_trace(vec![], || {
            let mut shell = test_shell();

            let flow = shell
                .run_builtin(&["export".into(), "FLOW=1".into()], &[("ASSIGN".into(), "2".into())])
                .expect("builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.get_var("ASSIGN").as_deref(), Some("2"));
            assert_eq!(shell.get_var("FLOW").as_deref(), Some("1"));

            let flow = shell.run_builtin(&["exit".into(), "9".into()], &[]).expect("exit builtin");
            assert!(matches!(flow, FlowSignal::Exit(9)));

            shell.function_depth = 1;
            let flow = shell.run_builtin(&["return".into(), "4".into()], &[]).expect("return builtin");
            assert!(matches!(flow, FlowSignal::Continue(4)));
            assert_eq!(shell.pending_control, Some(PendingControl::Return(4)));
            shell.pending_control = None;
            shell.function_depth = 0;

            shell.loop_depth = 2;
            let flow = shell.run_builtin(&["break".into(), "5".into()], &[]).expect("break builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.pending_control, Some(PendingControl::Break(2)));
            shell.pending_control = None;
        });
    }

    #[test]
    fn reap_jobs_handles_try_wait_errors() {
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ECHILD)),
        ], || {
            let mut shell = test_shell();
            let id = shell.launch_background_job("exit 0".into(), None, vec![fake_handle(1001)]);
            let finished = shell.reap_jobs();
            assert_eq!(finished, vec![(id, 1)]);
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn continue_job_errors_when_job_missing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let error = shell.continue_job(99).expect_err("missing job");
            assert_eq!(error.message, "job 99: not found");

            let error = shell.wait_for_job(99).expect_err("missing job");
            assert_eq!(error.message, "job 99: not found");
        });
    }

    #[test]
    fn source_path_errors_when_file_missing() {
        run_trace(vec![
            t("open", vec![ArgMatcher::Str("/definitely/missing-meiksh-script".into()), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(libc::ENOENT)),
        ], || {
            let mut shell = test_shell();
            let error = shell.source_path(Path::new("/definitely/missing-meiksh-script")).expect_err("missing source");
            assert!(!error.message.is_empty());
        });
    }

    #[test]
    fn shell_error_converts_from_parse_and_expand_errors() {
        assert_no_syscalls(|| {
            let parse_err = syntax::parse("echo 'unterminated").expect_err("parse");
            let shell_err: ShellError = parse_err.into();
            assert!(!shell_err.message.is_empty());

            let expand_err: ShellError = ExpandError { message: "expand".into() }.into();
            assert_eq!(expand_err.message, "expand");
            assert_eq!(format!("{}", shell_err), shell_err.message);
        });
    }

    #[test]
    fn capture_output_and_context_trait_methods_work() {
        fn capture_trace(data: &[u8], exit_status: i32, pid: i32) -> Vec<test_support::TraceEntry> {
            let mut entries = vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t("fork", vec![], TraceResult::Pid(pid)),
                t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
            ];
            if !data.is_empty() {
                entries.push(t("read", vec![ArgMatcher::Fd(200), ArgMatcher::Any],
                    TraceResult::Bytes(data.to_vec())));
            }
            entries.push(t("read", vec![ArgMatcher::Fd(200), ArgMatcher::Any], TraceResult::Int(0)));
            entries.push(t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)));
            entries.push(t("waitpid", vec![ArgMatcher::Int(pid as i64), ArgMatcher::Any, ArgMatcher::Int(0)],
                TraceResult::Status(exit_status)));
            entries
        }

        let mut trace = Vec::new();
        trace.extend(capture_trace(b"hi", 0, 1000));
        trace.extend(capture_trace(b"ok", 0, 1001));
        trace.extend(capture_trace(b"", 127, 1002));

        run_trace(trace, || {
            let mut shell = test_shell();
            let output = shell.capture_output("printf hi").expect("capture");
            assert_eq!(output, "hi");
            assert_eq!(expand::Context::shell_name(&shell), "meiksh");
            assert_eq!(expand::Context::positional_param(&shell, 0).as_deref(), Some("meiksh"));
            expand::Context::set_var(&mut shell, "CTX_SET", "7".into()).expect("ctx set");
            assert_eq!(shell.get_var("CTX_SET").as_deref(), Some("7"));
            shell.mark_readonly("CTX_SET");
            let error = expand::Context::set_var(&mut shell, "CTX_SET", "8".into()).expect_err("readonly ctx set");
            assert_eq!(error.message, "CTX_SET: readonly variable");
            let substituted = expand::Context::command_substitute(&mut shell, "printf ok").expect("subst");
            assert_eq!(substituted, "ok");

            let error = expand::Context::command_substitute(&mut shell, "missing-command").expect_err("subst error");
            assert!(!error.message.is_empty());
        });
    }

    #[test]
    fn parse_options_covers_dashdash_and_unknown_flags() {
        assert_no_syscalls(|| {
            let options = parse_options(&["meiksh".into(), "--".into(), "arg1".into(), "arg2".into()])
                .expect("parse");
            assert_eq!(options.positional, vec!["arg1".to_string(), "arg2".to_string()]);

            let error = parse_options(&["meiksh".into(), "-z".into(), "script.sh".into()])
                .expect_err("invalid option");
            assert_eq!(error.display_message(), "invalid option: z");
            assert_eq!(error.exit_status(), 2);

            let options = parse_options(&["meiksh".into(), "-fC".into(), "+f".into(), "script.sh".into()])
                .expect("parse");
            assert!(!options.noglob);
            assert!(options.noclobber);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

            let options = parse_options(&["meiksh".into(), "-inuv".into(), "+nuv".into(), "script.sh".into()])
                .expect("parse");
            assert!(options.force_interactive);
            assert!(!options.syntax_check_only);
            assert!(!options.nounset);
            assert!(!options.verbose);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

            let options = parse_options(&["meiksh".into(), "-".into()])
                .expect("parse lone dash");
            assert_eq!(options.script_path, None);
            assert!(options.positional.is_empty());
        });
    }

    #[test]
    fn shell_run_executes_script_from_path() {
        run_trace(vec![
            t("open", vec![ArgMatcher::Str("/tmp/run-test.sh".into()), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Fd(10)),
            t("read", vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                TraceResult::Bytes(b"VALUE=77\n".to_vec())),
            t("read", vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                TraceResult::Int(0)),
            t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            shell.options.script_path = Some(PathBuf::from("/tmp/run-test.sh"));
            let status = shell.run().expect("run");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("77"));
        });
    }

    #[test]
    fn capture_output_returns_error_on_command_failure() {
        run_trace(vec![
            t("pipe", vec![], TraceResult::Fds(200, 201)),
            t("fork", vec![], TraceResult::Pid(1000)),
            t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
            t("read", vec![ArgMatcher::Fd(200), ArgMatcher::Any], TraceResult::Int(0)),
            t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
            t("waitpid", vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                TraceResult::Status(127)),
        ], || {
            let mut shell = test_shell();
            let error = shell.capture_output("missing-command").expect_err("capture error");
            assert!(!error.message.is_empty());
        });
    }

    #[test]
    fn shell_error_status_metadata_helpers_work() {
        assert_no_syscalls(|| {
            let error = ShellError::with_status(127, "missing script");
            assert_eq!(error.exit_status(), 127);
            assert_eq!(error.display_message(), "missing script");
            assert_eq!(format!("{error}"), "missing script");
        });
    }

    #[test]
    fn stdin_parse_error_requires_more_input_for_open_constructs() {
        assert_no_syscalls(|| {
            for source in [
                "if true\n",
                "for item in a b\n",
                "cat <<EOF\nhello\n",
                "echo \"unterminated",
                "printf ok |\n",
            ] {
                let error = syntax::parse(source).expect_err("incomplete parse");
                assert!(stdin_parse_error_requires_more_input(&error), "{source}");
            }

            let error = syntax::parse("999999999999999999999999999999999999999999999999999999999999<in")
                .expect_err("syntax error");
            assert!(!stdin_parse_error_requires_more_input(&error));
        });
    }

    #[test]
    fn resolve_script_path_prefers_current_directory() {
        run_trace(vec![
            t("access", vec![ArgMatcher::Str("cwd-script".into()), ArgMatcher::Int(0)],
                TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            shell.env.insert("PATH".into(), "/search-path".into());
            assert_eq!(
                resolve_script_path(&shell, Path::new("cwd-script")),
                Some(PathBuf::from("cwd-script"))
            );
        });
    }

    #[test]
    fn resolve_script_path_searches_executable_path_entries() {
        run_trace(vec![
            t("access", vec![ArgMatcher::Str("path-script".into()), ArgMatcher::Int(0)],
                TraceResult::Err(libc::ENOENT)),
            t("stat", vec![ArgMatcher::Str("/search-path/path-script".into()), ArgMatcher::Any],
                TraceResult::StatFile(0o755)),
        ], || {
            let mut shell = test_shell();
            shell.env.insert("PATH".into(), "/search-path".into());
            assert_eq!(
                resolve_script_path(&shell, Path::new("path-script")),
                Some(PathBuf::from("/search-path/path-script"))
            );
        });
    }

    #[test]
    fn classify_script_read_error_maps_to_sh_exit_statuses() {
        assert_no_syscalls(|| {
            let classified = classify_script_read_error(Path::new("missing"), sys::SysError::Errno(libc::ENOENT));
            assert_eq!(classified.exit_status(), 127);
            let classified = classify_script_read_error(Path::new("bad"), sys::SysError::Errno(libc::EIO));
            assert_eq!(classified.exit_status(), 128);
        });
    }

    #[test]
    fn shell_run_executes_command_string() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.options.command_string = Some("VALUE=13".into());
            let status = shell.run().expect("run command string");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("13"));
        });
    }

    #[test]
    fn capture_output_returns_error_on_spawn_failure() {
        run_trace(vec![
            t("pipe", vec![], TraceResult::Fds(200, 201)),
            t("fork", vec![], TraceResult::Pid(1000)),
            t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
            t("read", vec![ArgMatcher::Fd(200), ArgMatcher::Any], TraceResult::Int(0)),
            t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
            t("waitpid", vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                TraceResult::Status(127)),
        ], || {
            let mut shell = test_shell();
            let error = shell.capture_output("printf hi").expect_err("spawn error");
            assert!(!error.message.is_empty());
        });
    }

    #[test]
    fn print_jobs_covers_running_and_finished_paths() {
        run_trace(vec![
            // reap_jobs for 1001 (explicit call)
            t("waitpid", vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0)),
            // print_jobs → reap_jobs → try_wait_child(1002) WNOHANG → done
            // This covers the "Done" branch in print_jobs
            t("waitpid", vec![ArgMatcher::Int(1002), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0)),
        ], || {
            let mut shell = test_shell();
            shell.launch_background_job("done".into(), None, vec![fake_handle(1001)]);
            shell.reap_jobs();
            shell.launch_background_job("sleep".into(), None, vec![fake_handle(1002)]);
            // Covers "Done" branch in print_jobs (1002 finishes with WNOHANG)
            shell.print_jobs();
            assert!(shell.jobs.is_empty());
        });

        // Cover the "Running" branch
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(1003), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Pid(0)),
            // wait_for_child_pid(1003) blocking (no pgid → no foreground_handoff)
            t("waitpid", vec![ArgMatcher::Int(1003), ArgMatcher::Any, ArgMatcher::Int(0)],
                TraceResult::Status(0)),
        ], || {
            let mut shell = test_shell();
            shell.launch_background_job("sleep".into(), None, vec![fake_handle(1003)]);
            shell.print_jobs();
            if let Some(id) = shell.jobs.first().map(|job| job.id) {
                let _ = shell.wait_for_job(id);
            }
        });
    }

    #[test]
    fn execute_string_uses_current_alias_table() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.execute_string("alias setok='export VALUE=ok'").expect("define alias");
            let status = shell.execute_string("setok").expect("run alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("ok"));

            let status = shell
                .execute_string("alias same='export SAME=1'; same")
                .expect("run same-source alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("SAME").as_deref(), Some("1"));

            shell.aliases.insert("cond".into(), "if".into());
            let status = shell
                .execute_string("cond true; then export BRANCH=hit; fi")
                .expect("run reserved-word alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("BRANCH").as_deref(), Some("hit"));

            let status = shell
                .execute_string("alias cond2='if'; cond2 true; then export TOP=ok; fi")
                .expect("run same-source reserved alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("TOP").as_deref(), Some("ok"));

            shell.aliases.insert("chain".into(), "eval ".into());
            shell.aliases.insert("word".into(), "VALUE=chain".into());
            let status = shell.execute_string("chain word").expect("run blank alias chain");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE").as_deref(), Some("chain"));
        });
    }

    #[test]
    fn print_jobs_emits_finished_branch_when_job_is_done() {
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0)),
        ], || {
            let mut shell = test_shell();
            shell.launch_background_job("done".into(), None, vec![fake_handle(1001)]);
            shell.print_jobs();
            assert!(shell.jobs.is_empty());
        });
    }

    #[test]
    fn set_trap_ignore_and_default_use_signal_syscall() {
        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any], TraceResult::Int(0)),
            t("signal", vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            shell
                .set_trap(TrapCondition::Signal(sys::SIGTERM), Some(TrapAction::Ignore))
                .expect("ignore");
            assert!(matches!(
                shell.trap_action(TrapCondition::Signal(sys::SIGTERM)),
                Some(TrapAction::Ignore)
            ));
            shell.set_trap(TrapCondition::Signal(sys::SIGTERM), None).expect("default");
            assert!(shell.trap_action(TrapCondition::Signal(sys::SIGTERM)).is_none());
        });
    }

    #[test]
    fn wait_operands_return_known_statuses_or_127() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.known_job_statuses.insert(9, 44);
            assert_eq!(shell.wait_for_job_operand(9).expect("known job"), 44);
            shell.known_pid_statuses.insert(55, 12);
            assert_eq!(shell.wait_for_pid_operand(55).expect("known pid"), 12);
            assert_eq!(shell.wait_for_job_operand(999).expect("unknown job"), 127);
            assert_eq!(shell.wait_for_pid_operand(999_999).expect("unknown pid"), 127);
        });
    }

    #[test]
    fn foreground_handoff_switches_terminal_process_group() {
        run_trace(vec![
            t("isatty", vec![ArgMatcher::Fd(sys::STDIN_FILENO)], TraceResult::Int(1)),
            t("isatty", vec![ArgMatcher::Fd(sys::STDERR_FILENO)], TraceResult::Int(1)),
            t("tcgetpgrp", vec![ArgMatcher::Fd(sys::STDIN_FILENO)], TraceResult::Pid(77)),
            t("tcsetpgrp", vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(88)], TraceResult::Int(0)),
            t("tcsetpgrp", vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(77)], TraceResult::Int(0)),
        ], || {
            let shell = test_shell();
            assert_eq!(shell.foreground_handoff(Some(88)), Some(77));
            shell.restore_foreground(Some(77));
        });
    }

    #[test]
    fn foreground_handoff_returns_none_when_tcgetpgrp_fails() {
        run_trace(vec![
            t("isatty", vec![ArgMatcher::Fd(sys::STDIN_FILENO)], TraceResult::Int(1)),
            t("isatty", vec![ArgMatcher::Fd(sys::STDERR_FILENO)], TraceResult::Int(1)),
            t("tcgetpgrp", vec![ArgMatcher::Fd(sys::STDIN_FILENO)], TraceResult::Pid(-1)),
        ], || {
            let shell = test_shell();
            assert_eq!(shell.foreground_handoff(Some(88)), None);
        });
    }

    #[test]
    fn execute_trap_action_and_run_pending_traps_work() {
        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
            t("signal", vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
            t("signal", vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            assert_eq!(shell.execute_trap_action("exit 9", 3).expect("exit trap action"), 9);
            assert!(!shell.running);
            assert_eq!(shell.last_status, 9);
            shell.running = true;

            shell
                .set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command(":".into())))
                .expect("trap");
            sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                shell.run_pending_traps().expect("run traps");
            });
            assert_eq!(shell.last_status, 9);

            shell
                .set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command("exit 7".into())))
                .expect("exit trap");
            sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                shell.run_pending_traps().expect("run exit trap");
            });
            assert!(!shell.running);
            shell.running = true;

            shell
                .set_trap(TrapCondition::Signal(sys::SIGTERM), Some(TrapAction::Ignore))
                .expect("ignore trap");
            sys::test_support::with_pending_signals_for_test(&[sys::SIGTERM], || {
                shell.run_pending_traps().expect("ignored pending");
            });
        });
    }

    #[test]
    fn continue_job_sends_sigcont_to_process_group() {
        run_trace(vec![
            t("kill", vec![ArgMatcher::Int(-11), ArgMatcher::Int(sys::SIGCONT as i64)], TraceResult::Int(0)),
        ], || {
            let mut shell = test_shell();
            let id = shell.launch_background_job("sleep".into(), Some(11), vec![fake_handle(1001)]);
            shell.continue_job(id).expect("continue pgid job");
            shell.jobs.clear();
        });
    }

    #[test]
    fn wait_for_job_operand_returns_130_on_eintr_with_pending_signal() {
        let mut shell = test_shell();
        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            shell.set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command(":".into())))
                .expect("trap");
        });

        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2001)]);
        sys::test_support::set_pending_signals_for_test(&[sys::SIGINT]);
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(2001), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::EINTR)),
        ], || {
            assert_eq!(shell.wait_for_job_operand(1).expect("interrupted wait"), 130);
        });
        assert_eq!(shell.last_status, 130);
    }

    #[test]
    fn wait_for_child_pid_retries_on_eintr_and_pid_zero() {
        let mut shell = test_shell();
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::EINTR)),
            t("waitpid", vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Pid(0)),
            t("waitpid", vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Status(7)),
        ], || {
            assert_eq!(shell.wait_for_child_pid(99, false).expect("retry after none"), 7);
        });
    }

    #[test]
    fn consume_wait_interrupt_parses_interrupt_message() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let message = ShellError { message: "wait interrupted:140".into() };
            assert_eq!(shell.consume_wait_interrupt(&message).expect("consume"), Some(140));
            let message = ShellError { message: "different".into() };
            assert_eq!(shell.consume_wait_interrupt(&message).expect("non interrupt"), None);
        });
    }

    #[test]
    fn wait_operations_fail_on_echild() {
        let mut shell = test_shell();
        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2002)]);
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(2002), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::ECHILD)),
            t("waitpid", vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::ECHILD)),
        ], || {
            assert!(shell.wait_for_job_operand(1).is_err());
            assert!(shell.wait_for_child_pid(99, false).is_err());
        });
    }

    #[test]
    fn wait_for_pid_operand_handles_interrupt_and_echild() {
        let mut shell = test_shell();
        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            shell.set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command(":".into())))
                .expect("trap");
        });

        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2003)]);
        sys::test_support::set_pending_signals_for_test(&[sys::SIGINT]);
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(2003), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::EINTR)),
        ], || {
            assert_eq!(shell.wait_for_pid_operand(2003).expect("pid interrupt"), 130);
        });

        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2004)]);
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(2004), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::ECHILD)),
        ], || {
            assert!(shell.wait_for_pid_operand(2004).is_err());
        });
    }

    #[test]
    fn wait_for_all_jobs_returns_130_on_interrupt() {
        let mut shell = test_shell();
        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            shell.set_trap(TrapCondition::Signal(sys::SIGINT), Some(TrapAction::Command(":".into())))
                .expect("trap");
        });

        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2002)]);
        shell.launch_background_job("sleep".into(), None, vec![fake_handle(2005)]);
        sys::test_support::set_pending_signals_for_test(&[sys::SIGINT]);
        run_trace(vec![
            t("waitpid", vec![ArgMatcher::Int(2002), ArgMatcher::Any, ArgMatcher::Int(0)], TraceResult::Err(libc::EINTR)),
        ], || {
            assert_eq!(shell.wait_for_all_jobs().expect("wait all status"), 130);
        });
    }

    #[test]
    fn known_job_status_fast_path_avoids_syscalls() {
        let mut shell = test_shell();
        let id = shell.launch_background_job("sleep".into(), None, vec![fake_handle(2006)]);
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
}
