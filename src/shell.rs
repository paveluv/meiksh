use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
        Err(err) => err.exit_status(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShellOptions {
    pub allexport: bool,
    pub command_string: Option<Box<str>>,
    pub errexit: bool,
    pub syntax_check_only: bool,
    pub force_interactive: bool,
    pub hashall: bool,
    pub monitor: bool,
    pub noclobber: bool,
    pub noglob: bool,
    pub notify: bool,
    pub nounset: bool,
    pub pipefail: bool,
    pub verbose: bool,
    pub xtrace: bool,
    pub script_path: Option<PathBuf>,
    pub shell_name_override: Option<Box<str>>,
    pub positional: Vec<String>,
}

const REPORTABLE_OPTION_NAMES: [(&str, char); 11] = [
    ("allexport", 'a'),
    ("errexit", 'e'),
    ("hashall", 'h'),
    ("monitor", 'm'),
    ("noclobber", 'C'),
    ("noglob", 'f'),
    ("noexec", 'n'),
    ("notify", 'b'),
    ("nounset", 'u'),
    ("verbose", 'v'),
    ("xtrace", 'x'),
];

impl ShellOptions {
    pub fn set_short_option(&mut self, ch: char, enabled: bool) -> Result<(), OptionError> {
        match ch {
            'a' => self.allexport = enabled,
            'b' => self.notify = enabled,
            'C' => self.noclobber = enabled,
            'e' => self.errexit = enabled,
            'f' => self.noglob = enabled,
            'h' => self.hashall = enabled,
            'i' => self.force_interactive = enabled,
            'm' => self.monitor = enabled,
            'n' => self.syntax_check_only = enabled,
            'u' => self.nounset = enabled,
            'v' => self.verbose = enabled,
            'x' => self.xtrace = enabled,
            _ => return Err(OptionError::InvalidShort(ch)),
        }
        Ok(())
    }

    pub fn set_named_option(&mut self, name: &str, enabled: bool) -> Result<(), OptionError> {
        if name == "pipefail" {
            self.pipefail = enabled;
            return Ok(());
        }
        let Some((_, letter)) = REPORTABLE_OPTION_NAMES
            .iter()
            .find(|(option_name, _)| *option_name == name)
        else {
            return Err(OptionError::InvalidName(name.into()));
        };
        self.set_short_option(*letter, enabled)
    }

    pub fn reportable_options(&self) -> [(&'static str, bool); 12] {
        [
            ("allexport", self.allexport),
            ("errexit", self.errexit),
            ("hashall", self.hashall),
            ("monitor", self.monitor),
            ("noclobber", self.noclobber),
            ("noglob", self.noglob),
            ("noexec", self.syntax_check_only),
            ("notify", self.notify),
            ("nounset", self.nounset),
            ("pipefail", self.pipefail),
            ("verbose", self.verbose),
            ("xtrace", self.xtrace),
        ]
    }
}

#[derive(Debug)]
pub enum ShellError {
    Status(i32),
}

impl ShellError {
    pub fn exit_status(&self) -> i32 {
        let ShellError::Status(s) = self;
        *s
    }
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ShellError::Status(s) = self;
        write!(f, "exit status {s}")
    }
}

impl std::error::Error for ShellError {}

#[derive(Debug)]
pub enum VarError {
    Readonly(Box<str>),
}

impl std::fmt::Display for VarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarError::Readonly(name) => write!(f, "{name}: readonly variable"),
        }
    }
}

#[derive(Debug)]
pub enum OptionError {
    InvalidShort(char),
    InvalidName(Box<str>),
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionError::InvalidShort(ch) => write!(f, "invalid option: {ch}"),
            OptionError::InvalidName(name) => write!(f, "invalid option name: {name}"),
        }
    }
}

#[derive(Clone)]
pub struct Shell {
    pub options: ShellOptions,
    pub shell_name: Box<str>,
    pub env: HashMap<String, String>,
    pub exported: BTreeSet<String>,
    pub readonly: BTreeSet<String>,
    pub aliases: HashMap<String, String>,
    pub functions: HashMap<String, crate::syntax::Command<'static>>,
    pub positional: Vec<String>,
    pub last_status: i32,
    pub last_background: Option<sys::Pid>,
    pub running: bool,
    pub jobs: Vec<Job>,
    pub known_pid_statuses: HashMap<sys::Pid, i32>,
    pub known_job_statuses: HashMap<usize, i32>,
    pub trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub ignored_on_entry: BTreeSet<TrapCondition>,
    pub loop_depth: usize,
    pub function_depth: usize,
    /// Nesting depth of dot (`source_path`) files being executed.
    pub source_depth: usize,
    pub pending_control: Option<PendingControl>,
    pub(crate) interactive: bool,
    pub(crate) errexit_suppressed: bool,
    pub(crate) pid: sys::Pid,
    pub(crate) lineno: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Running,
    Stopped(i32),
    Done(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReapedJobState {
    Stopped(i32),
    Done(i32),
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: usize,
    pub command: Box<str>,
    pub pgid: Option<sys::Pid>,
    pub last_pid: Option<sys::Pid>,
    pub last_status: Option<i32>,
    pub children: Vec<sys::ChildHandle>,
    pub state: JobState,
    pub saved_termios: Option<libc::termios>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrapCondition {
    Exit,
    Signal(sys::Pid),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrapAction {
    Ignore,
    Command(Box<str>),
}

pub enum FlowSignal {
    Continue(i32),
    UtilityError(i32),
    Exit(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildWaitResult {
    Exited(i32),
    Stopped(i32),
    Interrupted(i32),
}

fn try_wait_child(pid: sys::Pid) -> sys::SysResult<Option<ChildWaitResult>> {
    match sys::wait_pid_untraced(pid, true) {
        Ok(Some(waited)) => {
            if sys::wifstopped(waited.status) {
                Ok(Some(ChildWaitResult::Stopped(sys::wstopsig(waited.status))))
            } else {
                Ok(Some(ChildWaitResult::Exited(sys::decode_wait_status(
                    waited.status,
                ))))
            }
        }
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
    fn diagnostic_at(&self, line: usize, status: i32, msg: impl std::fmt::Display) -> ShellError {
        if line > 0 && !self.interactive {
            sys_eprintln!("meiksh: line {}: {}", line, msg);
        } else {
            sys_eprintln!("meiksh: {}", msg);
        }
        ShellError::Status(status)
    }

    pub fn diagnostic(&self, status: i32, msg: impl std::fmt::Display) -> ShellError {
        self.diagnostic_at(self.lineno, status, msg)
    }

    pub fn expand_to_err(&self, e: crate::expand::ExpandError) -> ShellError {
        if !e.message.is_empty() {
            self.diagnostic(1, &e);
        }
        ShellError::Status(1)
    }

    pub fn parse_to_err(&self, e: syntax::ParseError) -> ShellError {
        self.diagnostic_at(e.line.unwrap_or(0), 2, &e.message)
    }

    pub fn from_env() -> Result<Self, ShellError> {
        sys::setup_locale();
        let raw_args = sys::env_args_os();
        let args: Vec<String> = raw_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let options = parse_options(&args)?;
        let shell_name: Box<str> = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| sys::shell_name_from_args(&args).to_string().into());
        let raw_env: HashMap<String, String> = sys::env_vars();
        let mut env = HashMap::new();
        let mut exported = BTreeSet::new();
        for (key, value) in &raw_env {
            if crate::syntax::is_name(key) {
                env.insert(key.clone(), value.clone());
                exported.insert(key.clone());
            }
        }
        let interactive = options.force_interactive
            || (sys::is_interactive_fd(sys::STDIN_FILENO)
                && sys::is_interactive_fd(sys::STDERR_FILENO));
        let ignored_on_entry = Self::probe_ignored_signals();
        env.insert("IFS".into(), " \t\n".into());
        env.insert("PPID".into(), sys::parent_pid().to_string());
        env.insert("OPTIND".into(), "1".into());
        Self::init_pwd(&mut env);
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
            ignored_on_entry,
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive,
            errexit_suppressed: false,
            pid: sys::current_pid(),
            lineno: 0,
        })
    }

    fn init_pwd(env: &mut HashMap<String, String>) {
        let Ok(cwd) = sys::get_cwd() else { return };
        let valid = env.get("PWD").is_some_and(|p| {
            p.starts_with('/')
                && !p.split('/').any(|c| c == "." || c == "..")
                && std::path::Path::new(p) == std::path::Path::new(&cwd)
        });
        if !valid {
            env.insert("PWD".into(), cwd);
        }
    }

    fn probe_ignored_signals() -> BTreeSet<TrapCondition> {
        let mut set = BTreeSet::new();
        for signal in sys::supported_trap_signals() {
            if sys::query_signal_disposition(signal).unwrap_or(false) {
                set.insert(TrapCondition::Signal(signal));
            }
        }
        set
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn from_args(args: &[&str]) -> Result<Self, ShellError> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let options = parse_options(&args)?;
        let shell_name: Box<str> = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| sys::shell_name_from_args(&args).to_string().into());
        Ok(Self {
            positional: options.positional.clone(),
            interactive: options.force_interactive,
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
            ignored_on_entry: BTreeSet::new(),
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            errexit_suppressed: false,
            pid: sys::current_pid(),
            lineno: 0,
        })
    }

    pub fn run(&mut self) -> Result<i32, ShellError> {
        if self.interactive && !self.options.monitor {
            self.options.monitor = true;
        }
        if self.interactive {
            self.setup_interactive_signals()?;
            if self.options.monitor {
                self.setup_job_control();
            }
        }
        if self.interactive {
            interactive::load_env_file(self)?;
        }
        let result = if let Some(command) = self.options.command_string.clone() {
            self.run_source("<command>", &command)
        } else if let Some(script) = self.options.script_path.clone() {
            let (resolved, contents) = self.load_script_source(&script)?;
            self.run_source(resolved.to_string_lossy().as_ref(), &contents)
        } else if self.interactive {
            interactive::run(self)
        } else {
            self.run_standard_input()
        };
        match result {
            Ok(status) => self.run_exit_trap(status),
            Err(error) => self.run_exit_trap(error.exit_status()),
        }
    }

    fn setup_interactive_signals(&self) -> Result<(), ShellError> {
        sys::ignore_signal(sys::SIGQUIT).map_err(|e| self.diagnostic(1, &e))?;
        sys::ignore_signal(sys::SIGTERM).map_err(|e| self.diagnostic(1, &e))?;
        sys::install_shell_signal_handler(sys::SIGINT).map_err(|e| self.diagnostic(1, &e))?;
        if self.options.monitor {
            sys::ignore_signal(sys::SIGTSTP).map_err(|e| self.diagnostic(1, &e))?;
            sys::ignore_signal(sys::SIGTTIN).map_err(|e| self.diagnostic(1, &e))?;
            sys::ignore_signal(sys::SIGTTOU).map_err(|e| self.diagnostic(1, &e))?;
        }
        Ok(())
    }

    fn setup_job_control(&self) {
        let pid = sys::current_pid();
        let _ = sys::set_process_group(pid, pid);
        let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pid);
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    pub fn run_source(&mut self, _name: &str, source: &str) -> Result<i32, ShellError> {
        self.echo_verbose_input(source);
        self.run_source_buffer(source)
    }

    fn run_source_buffer(&mut self, source: &str) -> Result<i32, ShellError> {
        if self.options.syntax_check_only {
            let _ = syntax::parse_with_aliases(source, &self.aliases)
                .map_err(|e| self.parse_to_err(e))?;
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
        sys::ensure_blocking_read_fd(sys::STDIN_FILENO).map_err(|e| self.diagnostic(1, &e))?;
        let mut status = 0;
        let mut source = String::new();
        let mut line_bytes = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let count = loop {
                match sys::read_fd(sys::STDIN_FILENO, &mut byte) {
                    Ok(n) => break n,
                    Err(e) if e.is_eintr() => continue,
                    Err(e) => return Err(self.diagnostic(1, &e)),
                }
            };
            if count == 0 {
                if !line_bytes.is_empty() {
                    let chunk = decode_stdin_chunk(std::mem::take(&mut line_bytes))
                        .map_err(|e| self.diagnostic(1, &e))?;
                    self.echo_verbose_input(&chunk);
                    source.push_str(&chunk);
                }
                break;
            }
            line_bytes.push(byte[0]);
            if byte[0] == b'\n' {
                let chunk = decode_stdin_chunk(std::mem::take(&mut line_bytes))
                    .map_err(|e| self.diagnostic(1, &e))?;
                self.echo_verbose_input(&chunk);
                source.push_str(&chunk);
                self.apply_stdin_fragment(&mut source, false, &mut status)?;
                if !self.running || self.has_pending_control() {
                    return Ok(status);
                }
            }
        }

        self.apply_stdin_fragment(&mut source, true, &mut status)?;
        Ok(status)
    }

    fn apply_stdin_fragment(
        &mut self,
        source: &mut String,
        eof: bool,
        status: &mut i32,
    ) -> Result<(), ShellError> {
        if let Some(s) = self.maybe_run_stdin_source(source, eof)? {
            *status = s;
        }
        Ok(())
    }

    fn execute_source_incrementally(&mut self, source: &str) -> Result<i32, ShellError> {
        let saved_lineno = self.lineno;
        let mut session =
            syntax::ParseSession::new(source).map_err(|e| self.parse_to_err(e))?;
        let mut status = 0;
        self.run_pending_traps()?;
        loop {
            let program = match session
                .next_command(&self.aliases)
                .map_err(|e| self.parse_to_err(e))?
            {
                Some(p) => p,
                None => break,
            };
            status = self.execute_program(&program)?;
            self.run_pending_traps()?;
            if !self.running || self.has_pending_control() {
                break;
            }
        }
        self.lineno = saved_lineno;
        if let Some(PendingControl::Return(rs)) = self.pending_control.take() {
            if self.source_depth > 0 && self.function_depth == 0 {
                return Ok(rs);
            }
            self.pending_control = Some(PendingControl::Return(rs));
        }
        Ok(status)
    }

    fn maybe_run_stdin_source(
        &mut self,
        source: &mut String,
        eof: bool,
    ) -> Result<Option<i32>, ShellError> {
        if source.is_empty() {
            return Ok(None);
        }
        match syntax::parse_with_aliases(source, &self.aliases) {
            Ok(_) => {
                let buffered = std::mem::take(source);
                self.run_source_buffer(&buffered).map(Some)
            }
            Err(error) if !eof && stdin_parse_error_requires_more_input(&error) => Ok(None),
            Err(error) => Err(self.parse_to_err(error)),
        }
    }

    fn echo_verbose_input(&self, source: &str) {
        if self.options.verbose && !source.is_empty() {
            eprint!("{source}");
        }
    }

    pub fn capture_output(&mut self, source: &str) -> Result<String, ShellError> {
        let (read_fd, write_fd) = sys::create_pipe().map_err(|e| self.diagnostic(1, &e))?;
        let pid = sys::fork_process().map_err(|e| self.diagnostic(1, &e))?;
        if pid == 0 {
            let _ = sys::close_fd(read_fd);
            let _ = sys::duplicate_fd(write_fd, sys::STDOUT_FILENO);
            let _ = sys::close_fd(write_fd);
            let mut child_shell = self.clone();
            let _ = child_shell.reset_traps_for_subshell();
            let status = child_shell.execute_string(source).unwrap_or(1);
            sys::exit_process(status as sys::RawFd);
        }
        sys::close_fd(write_fd).map_err(|e| self.diagnostic(1, &e))?;
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = sys::read_fd(read_fd, &mut buf).map_err(|e| self.diagnostic(1, &e))?;
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }
        sys::close_fd(read_fd).map_err(|e| self.diagnostic(1, &e))?;
        let ws = sys::wait_pid(pid, false)
            .map_err(|e| self.diagnostic(1, &e))?
            .expect("child status");
        let status = sys::decode_wait_status(ws.status);
        self.last_status = status;
        let text = match String::from_utf8(output) {
            Ok(s) => s,
            Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
        };
        Ok(text)
    }

    pub fn env_for_child(&self) -> Vec<(String, String)> {
        self.exported
            .iter()
            .filter_map(|name| {
                self.env
                    .get(name)
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect()
    }

    /// Environment for [`exec`](crate::builtin) with a utility: exported variables from the
    /// shell plus prefix assignments for this command (even when not exported).
    pub fn env_for_exec_utility(
        &self,
        cmd_assignments: &[(String, String)],
    ) -> Vec<(String, String)> {
        let mut env = self.env_for_child();
        for (k, v) in cmd_assignments {
            if let Some(pos) = env.iter().position(|(name, _)| name == k) {
                env[pos] = (k.clone(), v.clone());
            } else {
                env.push((k.clone(), v.clone()));
            }
        }
        env
    }

    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.env.get(name).map(String::as_str)
    }

    pub fn input_is_incomplete(&self, error: &crate::syntax::ParseError) -> bool {
        stdin_parse_error_requires_more_input(error)
    }

    pub fn history_number(&self) -> usize {
        1
    }

    pub fn set_var(&mut self, name: &str, value: String) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        if let Some(existing) = self.env.get_mut(name) {
            *existing = value;
        } else {
            self.env.insert(name.to_string(), value);
        }
        if self.options.allexport && !self.exported.contains(name) {
            self.exported.insert(name.to_string());
        }
        Ok(())
    }

    pub fn export_var(&mut self, name: &str, value: Option<String>) -> Result<(), ShellError> {
        if let Some(value) = value {
            self.set_var(name, value)
                .map_err(|e| self.diagnostic(1, &e))?;
        }
        if !self.exported.contains(name) {
            self.exported.insert(name.to_string());
        }
        Ok(())
    }

    pub fn mark_readonly(&mut self, name: &str) {
        self.readonly.insert(name.to_string());
    }

    pub fn unset_var(&mut self, name: &str) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        self.env.remove(name);
        self.exported.remove(name);
        Ok(())
    }

    pub fn set_positional(&mut self, values: Vec<String>) {
        self.positional = values;
    }

    pub fn register_background_job(
        &mut self,
        command: Box<str>,
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
            state: JobState::Running,
            saved_termios: None,
        });
        id
    }

    pub fn reap_jobs(&mut self) -> Vec<(usize, ReapedJobState)> {
        let mut finished = Vec::new();
        let mut remaining = Vec::new();

        for mut job in self.jobs.drain(..) {
            if matches!(job.state, JobState::Stopped(_)) {
                remaining.push(job);
                continue;
            }
            let mut running = Vec::new();
            let mut any_stopped = false;
            let mut stop_signal = 0i32;
            for child in job.children.drain(..) {
                match try_wait_child(child.pid) {
                    Ok(Some(ChildWaitResult::Exited(code))) => {
                        self.known_pid_statuses.insert(child.pid, code);
                        if job.last_pid == Some(child.pid) {
                            job.last_status = Some(code);
                        }
                    }
                    Ok(Some(ChildWaitResult::Stopped(sig))) => {
                        any_stopped = true;
                        stop_signal = sig;
                        running.push(child);
                    }
                    Ok(Some(ChildWaitResult::Interrupted(_))) => {
                        unreachable!("non-blocking wait")
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
                job.state = JobState::Done(final_status);
                finished.push((job.id, ReapedJobState::Done(final_status)));
            } else if any_stopped {
                job.state = JobState::Stopped(stop_signal);
                finished.push((job.id, ReapedJobState::Stopped(stop_signal)));
                remaining.push(job);
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
            .ok_or_else(|| self.diagnostic(1, format_args!("job {id}: not found")))?;
        let pgid = self.jobs[index].pgid;
        if let Some(ref termios) = self.jobs[index].saved_termios {
            let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, termios);
        }
        let saved_foreground = self.foreground_handoff(pgid);
        self.jobs[index].state = JobState::Running;
        self.jobs[index].saved_termios = None;
        let mut status = self.jobs[index].last_status.unwrap_or(0);
        let children: Vec<sys::ChildHandle> = self.jobs[index].children.clone();
        for child in &children {
            match self.wait_for_child_pid(child.pid, false)? {
                ChildWaitResult::Exited(code) => {
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
                ChildWaitResult::Stopped(sig) => {
                    self.restore_foreground(saved_foreground);
                    let idx = self
                        .jobs
                        .iter()
                        .position(|j| j.id == id)
                        .expect("job vanished");
                    self.jobs[idx].state = JobState::Stopped(sig);
                    if self.interactive {
                        self.jobs[idx].saved_termios =
                            sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                    }
                    self.last_status = 128 + sig;
                    return Ok(128 + sig);
                }
                ChildWaitResult::Interrupted(_) => unreachable!("non-interruptible wait"),
            }
        }
        self.restore_foreground(saved_foreground);
        let idx = self.jobs.iter().position(|j| j.id == id);
        if let Some(idx) = idx {
            self.jobs.remove(idx);
        }
        self.last_status = status;
        Ok(status)
    }

    pub fn continue_job(&mut self, id: usize, foreground: bool) -> Result<(), ShellError> {
        let idx = self
            .jobs
            .iter()
            .position(|job| job.id == id)
            .ok_or_else(|| self.diagnostic(1, format_args!("job {id}: not found")))?;
        let was_stopped = matches!(self.jobs[idx].state, JobState::Stopped(_));
        self.jobs[idx].state = JobState::Running;
        if was_stopped {
            if let Some(pgid) = self.jobs[idx].pgid {
                if foreground {
                    let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
                }
                sys::send_signal(-pgid, sys::SIGCONT).map_err(|e| self.diagnostic(1, &e))?;
            } else {
                let pids: Vec<sys::Pid> = self.jobs[idx].children.iter().map(|c| c.pid).collect();
                for pid in pids {
                    sys::send_signal(pid, sys::SIGCONT).map_err(|e| self.diagnostic(1, &e))?;
                }
            }
        }
        Ok(())
    }

    pub fn source_path(&mut self, path: &Path) -> Result<i32, ShellError> {
        let contents =
            sys::read_file(&path.display().to_string()).map_err(|e| self.diagnostic(1, &e))?;
        self.source_depth += 1;
        let result = self.execute_string(&contents);
        self.source_depth -= 1;
        result
    }

    fn load_script_source(&self, script: &Path) -> Result<(PathBuf, String), ShellError> {
        let resolved = resolve_script_path(self, script)
            .ok_or_else(|| self.diagnostic(127, format_args!("{}: not found", script.display())))?;
        let bytes = sys::read_file_bytes(&resolved.display().to_string())
            .map_err(|error| classify_script_read_error(&resolved, error))?;
        if script_prefix_cannot_be_shell_input(&bytes) {
            return Err(
                self.diagnostic(126, format_args!("{}: cannot execute", resolved.display()))
            );
        }
        let contents = String::from_utf8_lossy(&bytes).into_owned();
        Ok((resolved, contents))
    }

    pub fn print_jobs(&mut self) {
        let finished = self.reap_jobs();
        for (id, state) in finished {
            if let ReapedJobState::Done(status) = state {
                sys_println!("[{id}] Done {status}");
            }
        }
        for job in &self.jobs {
            if let JobState::Stopped(sig) = job.state {
                sys_println!(
                    "[{}] Stopped ({}) {}",
                    job.id,
                    sys::signal_name(sig),
                    job.command
                );
            } else {
                sys_println!("[{}] Running {}", job.id, job.command);
            }
        }
    }

    pub fn run_builtin(
        &mut self,
        argv: &[String],
        assignments: &[(String, String)],
    ) -> Result<FlowSignal, ShellError> {
        for (name, value) in assignments {
            self.set_var(name, value.clone())
                .map_err(|e| self.diagnostic(1, &e))?;
        }
        match builtin::run(self, argv, assignments)? {
            BuiltinOutcome::Status(status) => Ok(FlowSignal::Continue(status)),
            BuiltinOutcome::UtilityError(status) => Ok(FlowSignal::UtilityError(status)),
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

    pub fn set_trap(
        &mut self,
        condition: TrapCondition,
        action: Option<TrapAction>,
    ) -> Result<(), ShellError> {
        if !self.interactive && self.ignored_on_entry.contains(&condition) {
            return Ok(());
        }
        if let TrapCondition::Signal(signal) = condition {
            match action.as_ref() {
                Some(TrapAction::Ignore) => {
                    sys::ignore_signal(signal).map_err(|e| self.diagnostic(1, &e))?
                }
                Some(TrapAction::Command(_)) => {
                    sys::install_shell_signal_handler(signal).map_err(|e| self.diagnostic(1, &e))?
                }
                None => sys::default_signal_action(signal).map_err(|e| self.diagnostic(1, &e))?,
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

    pub fn reset_traps_for_subshell(&mut self) -> Result<(), ShellError> {
        let to_reset: Vec<TrapCondition> = self
            .trap_actions
            .iter()
            .filter_map(|(cond, action)| match action {
                TrapAction::Command(_) => Some(*cond),
                TrapAction::Ignore => None,
            })
            .collect();
        for cond in to_reset {
            if let TrapCondition::Signal(signal) = cond {
                sys::default_signal_action(signal).map_err(|e| self.diagnostic(1, &e))?;
            }
            self.trap_actions.remove(&cond);
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
            Ok(ChildWaitResult::Exited(status)) => {
                self.record_completed_child(job_index, child_index, pid, status);
                Ok(status)
            }
            Ok(ChildWaitResult::Stopped(sig)) => Ok(128 + sig),
            Ok(ChildWaitResult::Interrupted(status)) => Ok(status),
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
            let Some(TrapAction::Command(action)) = self
                .trap_actions
                .get(&TrapCondition::Signal(signal))
                .cloned()
            else {
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
        let Some(TrapAction::Command(action)) =
            self.trap_actions.get(&TrapCondition::Exit).cloned()
        else {
            self.last_status = status;
            return Ok(status);
        };
        self.execute_trap_action(&action, status)
    }

    fn execute_trap_action(
        &mut self,
        action: &str,
        preserved_status: i32,
    ) -> Result<i32, ShellError> {
        let saved_lineno = self.lineno;
        let was_running = self.running;
        self.running = true;
        self.last_status = preserved_status;
        let status = self.execute_string(action)?;
        self.lineno = saved_lineno;
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
                Ok(ChildWaitResult::Exited(code)) => {
                    status = code;
                    self.record_completed_child(index, child_index, pid, code);
                }
                Ok(ChildWaitResult::Stopped(_sig)) => {
                    self.restore_foreground(saved_foreground);
                    return Ok(128 + _sig);
                }
                Ok(ChildWaitResult::Interrupted(int_status)) => {
                    self.restore_foreground(saved_foreground);
                    self.last_status = int_status;
                    self.run_pending_traps()?;
                    self.last_status = int_status;
                    return Ok(int_status);
                }
                Err(error) => {
                    self.restore_foreground(saved_foreground);
                    return Err(error);
                }
            }
        }
        let job = self.jobs.remove(index);
        let final_status = job.last_status.unwrap_or(status);
        self.restore_foreground(saved_foreground);
        self.last_status = final_status;
        Ok(final_status)
    }

    pub fn wait_for_child_pid(
        &mut self,
        pid: sys::Pid,
        interruptible: bool,
    ) -> Result<ChildWaitResult, ShellError> {
        loop {
            match sys::wait_pid_untraced(pid, false) {
                Ok(Some(waited)) => {
                    if sys::wifstopped(waited.status) {
                        return Ok(ChildWaitResult::Stopped(sys::wstopsig(waited.status)));
                    }
                    return Ok(ChildWaitResult::Exited(sys::decode_wait_status(
                        waited.status,
                    )));
                }
                Ok(None) => continue,
                Err(error)
                    if interruptible
                        && sys::interrupted(&error)
                        && sys::has_pending_signal().is_some() =>
                {
                    let signal = sys::has_pending_signal().unwrap_or(sys::SIGINT);
                    return Ok(ChildWaitResult::Interrupted(128 + signal));
                }
                Err(error) if sys::interrupted(&error) => continue,
                Err(error) => return Err(self.diagnostic(1, &error)),
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

    fn record_completed_child(
        &mut self,
        job_index: usize,
        child_index: usize,
        pid: sys::Pid,
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

    pub fn current_job_id(&self) -> Option<usize> {
        self.jobs
            .iter()
            .rev()
            .find(|j| matches!(j.state, JobState::Stopped(_)))
            .or_else(|| self.jobs.last())
            .map(|j| j.id)
    }

    pub fn previous_job_id(&self) -> Option<usize> {
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

    pub fn find_job_by_prefix(&self, prefix: &str) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.starts_with(prefix))
            .map(|j| j.id)
    }

    pub fn find_job_by_substring(&self, substring: &str) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.contains(substring))
            .map(|j| j.id)
    }

    fn foreground_handoff(&self, pgid: Option<sys::Pid>) -> Option<sys::Pid> {
        let Some(pgid) = pgid else {
            return None;
        };
        if !(sys::is_interactive_fd(sys::STDIN_FILENO)
            && sys::is_interactive_fd(sys::STDERR_FILENO))
        {
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
    fn env_var(&self, name: &str) -> Option<Cow<'_, str>> {
        self.env.get(name).map(|v| Cow::Borrowed(v.as_str()))
    }

    fn special_param(&self, name: char) -> Option<Cow<'_, str>> {
        match name {
            '?' => Some(Cow::Owned(self.last_status.to_string())),
            '$' => Some(Cow::Owned(self.pid.to_string())),
            '!' => self.last_background.map(|pid| Cow::Owned(pid.to_string())),
            '#' => Some(Cow::Owned(self.positional.len().to_string())),
            '-' => Some(Cow::Owned(self.active_option_flags())),
            '*' | '@' => Some(Cow::Owned(self.positional.join(" "))),
            '0' => Some(Cow::Borrowed(&self.shell_name)),
            digit if digit.is_ascii_digit() => {
                let index = digit.to_digit(10)? as usize;
                self.positional
                    .get(index.saturating_sub(1))
                    .map(|v| Cow::Borrowed(v.as_str()))
            }
            _ => None,
        }
    }

    fn positional_param(&self, index: usize) -> Option<Cow<'_, str>> {
        if index == 0 {
            Some(Cow::Borrowed(&self.shell_name))
        } else {
            self.positional
                .get(index - 1)
                .map(|v| Cow::Borrowed(v.as_str()))
        }
    }

    fn positional_params(&self) -> &[String] {
        &self.positional
    }

    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError> {
        self.set_var(name, value).map_err(|e| ExpandError {
            message: e.to_string().into(),
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
        self.capture_output(command)
            .map_err(|_| ExpandError { message: "".into() })
    }

    fn home_dir_for_user(&self, name: &str) -> Option<Cow<'_, str>> {
        sys::home_dir_for_user(name).map(Cow::Owned)
    }

    fn set_lineno(&mut self, line: usize) { self.lineno = line; }
    fn inc_lineno(&mut self) { self.lineno += 1; }
    fn lineno(&self) -> usize { self.lineno }
}

fn parse_options(args: &[String]) -> Result<ShellOptions, ShellError> {
    let mut options = ShellOptions::default();
    let mut index = 1usize;

    while let Some(arg) = args.get(index) {
        if arg == "-c" {
            let command = args.get(index + 1).ok_or_else(|| {
                sys_eprintln!("meiksh: {}", "-c requires an argument");
                ShellError::Status(2)
            })?;
            options.command_string = Some(command.clone().into());
            options.shell_name_override = args.get(index + 2).map(|s| s.clone().into());
            options.positional = args.iter().skip(index + 3).cloned().collect();
            return Ok(options);
        }
        if arg == "-o" || arg == "+o" {
            let enabled = arg == "-o";
            let name = args.get(index + 1).ok_or_else(|| {
                sys_eprintln!("meiksh: {}", format_args!("{arg} requires an argument"));
                ShellError::Status(2)
            })?;
            options.set_named_option(name, enabled).map_err(|e| {
                sys_eprintln!("meiksh: {}", e);
                ShellError::Status(2)
            })?;
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
            let mut saw_c = false;
            for ch in arg[1..].chars() {
                match ch {
                    'c' if enabled => saw_c = true,
                    's' if enabled => read_stdin = true,
                    _ => options.set_short_option(ch, enabled).map_err(|e| {
                        sys_eprintln!("meiksh: {}", e);
                        ShellError::Status(2)
                    })?,
                }
            }
            if saw_c {
                let command = args.get(index + 1).ok_or_else(|| {
                    sys_eprintln!("meiksh: {}", "-c requires an argument");
                    ShellError::Status(2)
                })?;
                options.command_string = Some(command.clone().into());
                options.shell_name_override = args.get(index + 2).map(|s| s.clone().into());
                options.positional = args.iter().skip(index + 3).cloned().collect();
                return Ok(options);
            }
            if read_stdin {
                options.positional = args.iter().skip(index + 1).cloned().collect();
                return Ok(options);
            }
            index += 1;
            continue;
        }
        options.script_path = Some(PathBuf::from(arg));
        options.shell_name_override = Some(arg.clone().into());
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
        if self.options.notify {
            flags.push('b');
        }
        if self.options.noclobber {
            flags.push('C');
        }
        if self.options.errexit {
            flags.push('e');
        }
        if self.options.noglob {
            flags.push('f');
        }
        if self.options.hashall {
            flags.push('h');
        }
        if self.is_interactive() {
            flags.push('i');
        }
        if self.options.monitor {
            flags.push('m');
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
        if self.options.xtrace {
            flags.push('x');
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
        .map(|s| s.to_string())
        .or_else(|| sys::env_var("PATH"))
        .unwrap_or_default();
    for dir in path_env.split(':') {
        let base = if dir.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(dir)
        };
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
    String::from_utf8(bytes).map_err(|_| sys::SysError::Errno(sys::EILSEQ))
}

fn stdin_parse_error_requires_more_input(error: &syntax::ParseError) -> bool {
    matches!(
        &*error.message,
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
        sys_eprintln!("meiksh: {}", format_args!("{}: not found", path.display()));
        ShellError::Status(127)
    } else {
        sys_eprintln!("meiksh: {}", format_args!("{}: {}", path.display(), error));
        ShellError::Status(128)
    }
}

fn script_prefix_cannot_be_shell_input(bytes: &[u8]) -> bool {
    const PREFIX_LEN: usize = 4096;
    let prefix = &bytes[..bytes.len().min(PREFIX_LEN)];
    let newline = prefix.iter().position(|&byte| byte == b'\n');
    let scan_end = newline.unwrap_or(prefix.len());
    prefix[..scan_end].contains(&b'\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::sys::test_support::{
        self, ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t, t_fork,
    };

    fn fake_handle(pid: sys::Pid) -> sys::ChildHandle {
        sys::ChildHandle {
            pid,
            stdout_fd: None,
        }
    }

    fn t_stderr(msg: &str) -> test_support::TraceEntry {
        t(
            "write",
            vec![
                ArgMatcher::Fd(sys::STDERR_FILENO),
                ArgMatcher::Bytes(format!("{msg}\n").into_bytes()),
            ],
            TraceResult::Auto,
        )
    }

    fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: "meiksh".into(),
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
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive: false,
            errexit_suppressed: false,
            pid: 0,
            lineno: 0,
        }
    }

    #[test]
    fn parse_options_handles_command_script_and_errors() {
        run_trace(
            vec![
                t_stderr("meiksh: -c requires an argument"),
                t_stderr("meiksh: -o requires an argument"),
                t_stderr("meiksh: invalid option name: bogus"),
            ],
            || {
                let options = parse_options(&[
                    "meiksh".into(),
                    "-c".into(),
                    "echo ok".into(),
                    "name".into(),
                    "arg".into(),
                ])
                .expect("parse");
                assert_eq!(options.command_string.as_deref(), Some("echo ok"));
                assert_eq!(options.shell_name_override.as_deref(), Some("name"));
                assert_eq!(options.positional, vec!["arg".to_string()]);

                let options = parse_options(&[
                    "meiksh".into(),
                    "-n".into(),
                    "-i".into(),
                    "-f".into(),
                    "script.sh".into(),
                    "a".into(),
                ])
                .expect("parse");
                assert!(options.syntax_check_only);
                assert!(options.force_interactive);
                assert!(options.noglob);
                assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));
                assert_eq!(options.positional, vec!["a".to_string()]);

                let options =
                    parse_options(&["meiksh".into(), "-s".into(), "arg1".into(), "arg2".into()])
                        .expect("parse -s");
                assert_eq!(options.script_path, None);
                assert_eq!(
                    options.positional,
                    vec!["arg1".to_string(), "arg2".to_string()]
                );

                let options = parse_options(&["meiksh".into(), "-is".into(), "arg".into()])
                    .expect("parse -is");
                assert!(options.force_interactive);
                assert_eq!(options.positional, vec!["arg".to_string()]);

                let options = parse_options(&[
                    "meiksh".into(),
                    "-a".into(),
                    "-u".into(),
                    "-o".into(),
                    "noglob".into(),
                    "-v".into(),
                    "script.sh".into(),
                ])
                .expect("parse -a -u -o noglob -v");
                assert!(options.allexport);
                assert!(options.nounset);
                assert!(options.noglob);
                assert!(options.verbose);
                assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

                let error =
                    parse_options(&["meiksh".into(), "-c".into()]).expect_err("missing arg");
                assert_eq!(error.exit_status(), 2);

                let error =
                    parse_options(&["meiksh".into(), "-o".into()]).expect_err("missing -o arg");
                assert_eq!(error.exit_status(), 2);

                let options = parse_options(&[
                    "meiksh".into(),
                    "-o".into(),
                    "pipefail".into(),
                    "s.sh".into(),
                ])
                .expect("parse -o pipefail");
                assert!(options.pipefail);

                let error = parse_options(&["meiksh".into(), "-o".into(), "bogus".into()])
                    .expect_err("bad -o name");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn env_for_child_filters_exported_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("A".into(), "1".into());
            shell.env.insert("B".into(), "2".into());
            shell.exported.insert("A".into());
            let env = shell.env_for_child();
            assert_eq!(
                env.iter().find(|(k, _)| k == "A").map(|(_, v)| v.as_str()),
                Some("1")
            );
            assert!(!env.iter().any(|(k, _)| k == "B"));

            shell.options.allexport = true;
            shell.set_var("B", "3".into()).expect("allexport set");
            let env = shell.env_for_child();
            assert_eq!(
                env.iter().find(|(k, _)| k == "B").map(|(_, v)| v.as_str()),
                Some("3")
            );
        });
    }

    #[test]
    fn readonly_variables_reject_mutation_and_unset() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.set_var("NAME", "value".into()).expect("set");
            shell.mark_readonly("NAME");
            let set_error = shell.set_var("NAME", "new".into()).expect_err("readonly");
            assert_eq!(set_error.to_string(), "NAME: readonly variable");
            let unset_error = shell.unset_var("NAME").expect_err("readonly");
            assert_eq!(unset_error.to_string(), "NAME: readonly variable");
        });
    }

    #[test]
    fn special_parameters_reflect_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.pid = 12345;
            shell.positional = vec!["first".into(), "second".into()];
            shell.last_status = 17;
            shell.last_background = Some(42);
            shell.options.allexport = true;
            shell.options.noclobber = true;
            shell.options.command_string = Some("printf ok".into());
            assert_eq!(
                expand::Context::special_param(&shell, '?').as_deref(),
                Some("17")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '$').as_deref(),
                Some("12345")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '#').as_deref(),
                Some("2")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '!').as_deref(),
                Some("42")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '-').as_deref(),
                Some("aCc")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '*').as_deref(),
                Some("first second")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '@').as_deref(),
                Some("first second")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '1').as_deref(),
                Some("first")
            );
            assert_eq!(
                expand::Context::special_param(&shell, '0').as_deref(),
                Some("meiksh")
            );
            assert_eq!(expand::Context::special_param(&shell, '9'), None);
            assert_eq!(expand::Context::special_param(&shell, 'x'), None);
        });
    }

    #[test]
    fn dollar_hyphen_includes_i_when_interactive() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.interactive = true;
            assert!(shell.active_option_flags().contains('i'));
        });
    }

    #[test]
    fn dollar_hyphen_excludes_i_when_not_interactive() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert!(!shell.active_option_flags().contains('i'));
        });
    }

    #[test]
    fn setup_interactive_signals_ignores_sigquit_sigterm_installs_sigint() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGQUIT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                shell.setup_interactive_signals().expect("signal setup");
            },
        );
    }

    #[test]
    fn launch_and_wait_for_background_job_updates_state() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(1001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::Status(7),
            )],
            || {
                let mut shell = test_shell();
                let id =
                    shell.register_background_job("exit 7".into(), None, vec![fake_handle(1001)]);
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
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/source-test.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"VALUE=42\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                let status = shell
                    .source_path(Path::new("/tmp/source-test.sh"))
                    .expect("source");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("VALUE"), Some("42"));
            },
        );
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
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Status(0),
            )],
            || {
                let mut shell = test_shell();
                shell.register_background_job("exit 0".into(), None, vec![fake_handle(1001)]);
                let finished = shell.reap_jobs();
                assert_eq!(finished, vec![(1, ReapedJobState::Done(0))]);
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn run_builtin_returns_correct_flow_signals() {
        run_trace(vec![], || {
            let mut shell = test_shell();

            let flow = shell
                .run_builtin(
                    &["export".into(), "FLOW=1".into()],
                    &[("ASSIGN".into(), "2".into())],
                )
                .expect("builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.get_var("ASSIGN"), Some("2"));
            assert_eq!(shell.get_var("FLOW"), Some("1"));

            let flow = shell
                .run_builtin(&["exit".into(), "9".into()], &[])
                .expect("exit builtin");
            assert!(matches!(flow, FlowSignal::Exit(9)));

            shell.function_depth = 1;
            let flow = shell
                .run_builtin(&["return".into(), "4".into()], &[])
                .expect("return builtin");
            assert!(matches!(flow, FlowSignal::Continue(4)));
            assert_eq!(shell.pending_control, Some(PendingControl::Return(4)));
            shell.pending_control = None;
            shell.function_depth = 0;

            shell.loop_depth = 2;
            let flow = shell
                .run_builtin(&["break".into(), "5".into()], &[])
                .expect("break builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.pending_control, Some(PendingControl::Break(2)));
            shell.pending_control = None;
        });
    }

    #[test]
    fn reap_jobs_handles_try_wait_errors() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::Err(sys::ECHILD),
            )],
            || {
                let mut shell = test_shell();
                let id =
                    shell.register_background_job("exit 0".into(), None, vec![fake_handle(1001)]);
                let finished = shell.reap_jobs();
                assert_eq!(finished, vec![(id, ReapedJobState::Done(1))]);
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn continue_job_errors_when_job_missing() {
        run_trace(
            vec![
                t_stderr("meiksh: job 99: not found"),
                t_stderr("meiksh: job 99: not found"),
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
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/definitely/missing-meiksh-script".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: No such file or directory\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let error = shell
                    .source_path(Path::new("/definitely/missing-meiksh-script"))
                    .expect_err("missing source");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn shell_error_converts_from_parse_and_expand_errors() {
        run_trace(
            vec![
                t_stderr("meiksh: line 1: unterminated single quote"),
                t_stderr("meiksh: expand"),
            ],
            || {
                let shell = test_shell();
                let parse_err = syntax::parse("echo 'unterminated").expect_err("parse");
                let shell_err = shell.parse_to_err(parse_err);
                assert_eq!(shell_err.exit_status(), 2);

                let expand_err = shell.expand_to_err(ExpandError {
                    message: "expand".into(),
                });
                assert_eq!(expand_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn context_trait_methods_work() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(expand::Context::shell_name(&shell), "meiksh");
            assert_eq!(
                expand::Context::positional_param(&shell, 0).as_deref(),
                Some("meiksh")
            );
            expand::Context::set_var(&mut shell, "CTX_SET", "7".into()).expect("ctx set");
            assert_eq!(shell.get_var("CTX_SET"), Some("7"));
            shell.mark_readonly("CTX_SET");
            let error = expand::Context::set_var(&mut shell, "CTX_SET", "8".into())
                .expect_err("readonly ctx set");
            assert_eq!(&*error.message, "CTX_SET: readonly variable");
        });
    }

    fn capture_forked_trace(exit_status: i32, pid: i32) -> Vec<test_support::TraceEntry> {
        let child = vec![
            t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
            t(
                "dup2",
                vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                TraceResult::Int(0),
            ),
            t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
        ];
        vec![
            t("pipe", vec![], TraceResult::Fds(200, 201)),
            t_fork(TraceResult::Pid(pid), child),
            t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
            t(
                "read",
                vec![ArgMatcher::Fd(200), ArgMatcher::Any],
                TraceResult::Int(0),
            ),
            t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
            t(
                "waitpid",
                vec![
                    ArgMatcher::Int(pid as i64),
                    ArgMatcher::Any,
                    ArgMatcher::Int(0),
                ],
                TraceResult::Status(exit_status),
            ),
        ]
    }

    #[test]
    fn capture_output_success() {
        run_trace(capture_forked_trace(0, 1000), || {
            let mut shell = test_shell();
            let output = shell.capture_output("true").expect("capture");
            assert_eq!(output, "");
        });
    }

    #[test]
    fn capture_output_sets_last_status_on_nonzero_exit() {
        run_trace(capture_forked_trace(1, 1000), || {
            let mut shell = test_shell();
            let output = shell.capture_output("false").expect("capture ok");
            assert_eq!(output, "");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn command_substitute_success() {
        run_trace(capture_forked_trace(0, 1000), || {
            let mut shell = test_shell();
            let substituted =
                expand::Context::command_substitute(&mut shell, "true").expect("subst");
            assert_eq!(substituted, "");
            assert_eq!(shell.last_status, 0);
        });
    }

    #[test]
    fn command_substitute_sets_last_status_on_nonzero_exit() {
        run_trace(capture_forked_trace(1, 1000), || {
            let mut shell = test_shell();
            let output =
                expand::Context::command_substitute(&mut shell, "false").expect("subst ok");
            assert_eq!(output, "");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn parse_options_covers_dashdash_and_unknown_flags() {
        run_trace(vec![t_stderr("meiksh: invalid option: z")], || {
            let options =
                parse_options(&["meiksh".into(), "--".into(), "arg1".into(), "arg2".into()])
                    .expect("parse");
            assert_eq!(
                options.positional,
                vec!["arg1".to_string(), "arg2".to_string()]
            );

            let error = parse_options(&["meiksh".into(), "-z".into(), "script.sh".into()])
                .expect_err("invalid option");
            assert_eq!(error.exit_status(), 2);

            let options = parse_options(&[
                "meiksh".into(),
                "-fC".into(),
                "+f".into(),
                "script.sh".into(),
            ])
            .expect("parse");
            assert!(!options.noglob);
            assert!(options.noclobber);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

            let options = parse_options(&[
                "meiksh".into(),
                "-inuv".into(),
                "+nuv".into(),
                "script.sh".into(),
            ])
            .expect("parse");
            assert!(options.force_interactive);
            assert!(!options.syntax_check_only);
            assert!(!options.nounset);
            assert!(!options.verbose);
            assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

            let options = parse_options(&["meiksh".into(), "-".into()]).expect("parse lone dash");
            assert_eq!(options.script_path, None);
            assert!(options.positional.is_empty());
        });
    }

    #[test]
    fn shell_run_executes_script_from_path() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/run-test.sh".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"VALUE=77\n".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.options.script_path = Some(PathBuf::from("/tmp/run-test.sh"));
                let status = shell.run().expect("run");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var("VALUE"), Some("77"));
            },
        );
    }

    #[test]
    fn capture_output_sets_last_status_127() {
        run_trace(capture_forked_trace(127, 1000), || {
            let mut shell = test_shell();
            let output = shell.capture_output("exit 127").expect("capture ok");
            assert_eq!(output, "");
            assert_eq!(shell.last_status, 127);
        });
    }

    #[test]
    fn shell_error_status_helpers_work() {
        run_trace(vec![t_stderr("meiksh: missing script")], || {
            let shell = test_shell();
            let error = shell.diagnostic(127, "missing script");
            assert_eq!(error.exit_status(), 127);

            let silent = ShellError::Status(42);
            assert_eq!(silent.exit_status(), 42);
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

            let program = syntax::parse(
                "999999999999999999999999999999999999999999999999999999999999<in",
            )
            .expect("overflowing number is a word, not an io_number");
            assert_eq!(program.items.len(), 1);
        });
    }

    #[test]
    fn resolve_script_path_prefers_current_directory() {
        run_trace(
            vec![t(
                "access",
                vec![ArgMatcher::Str("cwd-script".into()), ArgMatcher::Int(0)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/search-path".into());
                assert_eq!(
                    resolve_script_path(&shell, Path::new("cwd-script")),
                    Some(PathBuf::from("cwd-script"))
                );
            },
        );
    }

    #[test]
    fn resolve_script_path_searches_executable_path_entries() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("path-script".into()), ArgMatcher::Int(0)],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/search-path/path-script".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::StatFile(0o755),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/search-path".into());
                assert_eq!(
                    resolve_script_path(&shell, Path::new("path-script")),
                    Some(PathBuf::from("/search-path/path-script"))
                );
            },
        );
    }

    #[test]
    fn classify_script_read_error_maps_to_sh_exit_statuses() {
        run_trace(
            vec![
                t_stderr("meiksh: missing: not found"),
                t_stderr("meiksh: bad: Input/output error"),
            ],
            || {
                let classified = classify_script_read_error(
                    Path::new("missing"),
                    sys::SysError::Errno(sys::ENOENT),
                );
                assert_eq!(classified.exit_status(), 127);
                let classified =
                    classify_script_read_error(Path::new("bad"), sys::SysError::Errno(sys::EIO));
                assert_eq!(classified.exit_status(), 128);
            },
        );
    }

    #[test]
    fn script_prefix_heuristic_rejects_nul_before_newline() {
        assert_no_syscalls(|| {
            assert!(script_prefix_cannot_be_shell_input(b"\0echo hi\n"));
            assert!(script_prefix_cannot_be_shell_input(b"abc\0def"));
            assert!(!script_prefix_cannot_be_shell_input(b"echo hi\n\0tail"));
            assert!(!script_prefix_cannot_be_shell_input(b"echo hi\n"));
        });
    }

    #[test]
    fn shell_run_executes_command_string() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell.options.command_string = Some("VALUE=13".into());
            let status = shell.run().expect("run command string");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("13"));
        });
    }

    #[test]
    fn capture_output_returns_error_on_fork_failure() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t("fork", vec![], TraceResult::Err(sys::EINVAL)),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: Invalid argument\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                let error = shell.capture_output("true").expect_err("fork error");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn print_jobs_shows_done_for_finished_job() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] Done 0\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job("done".into(), None, vec![fake_handle(1001)]);
                shell.reap_jobs();
                shell.register_background_job("sleep".into(), None, vec![fake_handle(1002)]);
                shell.print_jobs();
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn print_jobs_shows_running_for_active_job() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1003), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] Running sleep\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(1003),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job("sleep".into(), None, vec![fake_handle(1003)]);
                shell.print_jobs();
                if let Some(id) = shell.jobs.first().map(|job| job.id) {
                    let _ = shell.wait_for_job(id);
                }
            },
        );
    }

    #[test]
    fn execute_string_uses_current_alias_table() {
        run_trace(vec![], || {
            let mut shell = test_shell();
            shell
                .execute_string("alias setok='export VALUE=ok'")
                .expect("define alias");
            let status = shell.execute_string("setok").expect("run alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("ok"));

            let status = shell
                .execute_string("alias same='export SAME=1'\nsame")
                .expect("run same-source alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("SAME"), Some("1"));

            shell.aliases.insert("cond".into(), "if".into());
            let status = shell
                .execute_string("cond true; then export BRANCH=hit; fi")
                .expect("run reserved-word alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("BRANCH"), Some("hit"));

            let status = shell
                .execute_string("alias cond2='if'\ncond2 true; then export TOP=ok; fi")
                .expect("run same-source reserved alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("TOP"), Some("ok"));

            shell.aliases.insert("chain".into(), "eval ".into());
            shell.aliases.insert("word".into(), "VALUE=chain".into());
            let status = shell
                .execute_string("chain word")
                .expect("run blank alias chain");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var("VALUE"), Some("chain"));
        });
    }

    #[test]
    fn print_jobs_emits_finished_branch_when_job_is_done() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] Done 0\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job("done".into(), None, vec![fake_handle(1001)]);
                shell.print_jobs();
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn set_trap_ignore_and_default_use_signal_syscall() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore");
                assert!(matches!(
                    shell.trap_action(TrapCondition::Signal(sys::SIGTERM)),
                    Some(TrapAction::Ignore)
                ));
                shell
                    .set_trap(TrapCondition::Signal(sys::SIGTERM), None)
                    .expect("default");
                assert!(
                    shell
                        .trap_action(TrapCondition::Signal(sys::SIGTERM))
                        .is_none()
                );
            },
        );
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
            assert_eq!(
                shell.wait_for_pid_operand(999_999).expect("unknown pid"),
                127
            );
        });
    }

    #[test]
    fn foreground_handoff_switches_terminal_process_group() {
        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "tcgetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                    TraceResult::Pid(77),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(88)],
                    TraceResult::Int(0),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(77)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let shell = test_shell();
                assert_eq!(shell.foreground_handoff(Some(88)), Some(77));
                shell.restore_foreground(Some(77));
            },
        );
    }

    #[test]
    fn foreground_handoff_returns_none_when_tcgetpgrp_fails() {
        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(sys::STDERR_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "tcgetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                    TraceResult::Pid(-1),
                ),
            ],
            || {
                let shell = test_shell();
                assert_eq!(shell.foreground_handoff(Some(88)), None);
            },
        );
    }

    #[test]
    fn execute_trap_action_and_run_pending_traps_work() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                assert_eq!(
                    shell
                        .execute_trap_action("exit 9", 3)
                        .expect("exit trap action"),
                    9
                );
                assert!(!shell.running);
                assert_eq!(shell.last_status, 9);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(":".into())),
                    )
                    .expect("trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                    shell.run_pending_traps().expect("run traps");
                });
                assert_eq!(shell.last_status, 9);

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command("exit 7".into())),
                    )
                    .expect("exit trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                    shell.run_pending_traps().expect("run exit trap");
                });
                assert!(!shell.running);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGTERM),
                        Some(TrapAction::Ignore),
                    )
                    .expect("ignore trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGTERM], || {
                    shell.run_pending_traps().expect("ignored pending");
                });
            },
        );
    }

    #[test]
    fn continue_job_sends_sigcont_to_process_group() {
        run_trace(
            vec![t(
                "kill",
                vec![ArgMatcher::Int(-11), ArgMatcher::Int(sys::SIGCONT as i64)],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    "sleep".into(),
                    Some(11),
                    vec![fake_handle(1001)],
                );
                let idx = shell.jobs.iter().position(|j| j.id == id).unwrap();
                shell.jobs[idx].state = JobState::Stopped(sys::SIGTSTP);
                shell.continue_job(id, false).expect("continue pgid job");
                shell.jobs.clear();
            },
        );
    }

    #[test]
    fn wait_for_job_operand_returns_130_on_eintr_with_pending_signal() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2001),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(":".into())),
                    )
                    .expect("trap");
                shell.register_background_job("sleep".into(), None, vec![fake_handle(2001)]);
                assert_eq!(
                    shell.wait_for_job_operand(1).expect("interrupted wait"),
                    130
                );
                assert_eq!(shell.last_status, 130);
            },
        );
    }

    #[test]
    fn wait_for_child_pid_retries_on_eintr_and_pid_zero() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(99),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Err(sys::EINTR),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(99),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Pid(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(99),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(7),
                ),
            ],
            || {
                let mut shell = test_shell();
                assert_eq!(
                    shell
                        .wait_for_child_pid(99, false)
                        .expect("retry after none"),
                    ChildWaitResult::Exited(7)
                );
            },
        );
    }

    #[test]
    fn wait_operations_fail_on_echild() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2002),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Err(sys::ECHILD),
                ),
                t_stderr("meiksh: No child processes"),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(99),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Err(sys::ECHILD),
                ),
                t_stderr("meiksh: No child processes"),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job("sleep".into(), None, vec![fake_handle(2002)]);
                assert!(shell.wait_for_job_operand(1).is_err());
                assert!(shell.wait_for_child_pid(99, false).is_err());
            },
        );
    }

    #[test]
    fn wait_for_pid_operand_handles_interrupt_and_echild() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2003),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2004),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Err(sys::ECHILD),
                ),
                t_stderr("meiksh: No child processes"),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(":".into())),
                    )
                    .expect("trap");

                shell.register_background_job("sleep".into(), None, vec![fake_handle(2003)]);
                assert_eq!(
                    shell.wait_for_pid_operand(2003).expect("pid interrupt"),
                    130
                );

                shell.register_background_job("sleep".into(), None, vec![fake_handle(2004)]);
                assert!(shell.wait_for_pid_operand(2004).is_err());
            },
        );
    }

    #[test]
    fn wait_for_all_jobs_returns_130_on_interrupt() {
        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(sys::SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2002),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(":".into())),
                    )
                    .expect("trap");
                shell.register_background_job("sleep".into(), None, vec![fake_handle(2002)]);
                shell.register_background_job("sleep".into(), None, vec![fake_handle(2005)]);
                assert_eq!(shell.wait_for_all_jobs().expect("wait all status"), 130);
            },
        );
    }

    #[test]
    fn wait_for_job_operand_consumes_status_second_wait_returns_127() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(3001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::Status(42),
            )],
            || {
                let mut shell = test_shell();
                let id =
                    shell.register_background_job("sleep".into(), None, vec![fake_handle(3001)]);
                assert_eq!(shell.wait_for_job_operand(id).expect("first wait"), 42);
                assert_eq!(shell.wait_for_job_operand(id).expect("second wait"), 127);
            },
        );
    }

    #[test]
    fn known_job_status_fast_path_avoids_syscalls() {
        let mut shell = test_shell();
        let id = shell.register_background_job("sleep".into(), None, vec![fake_handle(2006)]);
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
    fn parse_options_combined_c_with_other_flags() {
        run_trace(vec![t_stderr("meiksh: -c requires an argument")], || {
            let options = parse_options(&[
                "meiksh".into(),
                "-ac".into(),
                "echo ok".into(),
                "name".into(),
            ])
            .expect("parse -ac");
            assert!(options.allexport);
            assert_eq!(options.command_string.as_deref(), Some("echo ok"));
            assert_eq!(options.shell_name_override.as_deref(), Some("name"));

            let options = parse_options(&["meiksh".into(), "-euc".into(), "echo ok".into()])
                .expect("parse -euc");
            assert!(options.errexit);
            assert!(options.nounset);
            assert_eq!(options.command_string.as_deref(), Some("echo ok"));

            let error =
                parse_options(&["meiksh".into(), "-ec".into()]).expect_err("missing -c arg");
            assert_eq!(error.exit_status(), 2);
        });
    }

    #[test]
    fn set_short_option_accepts_new_options() {
        assert_no_syscalls(|| {
            let mut opts = ShellOptions::default();
            opts.set_short_option('e', true).expect("set -e");
            assert!(opts.errexit);
            opts.set_short_option('e', false).expect("set +e");
            assert!(!opts.errexit);

            opts.set_short_option('x', true).expect("set -x");
            assert!(opts.xtrace);
            opts.set_short_option('x', false).expect("set +x");
            assert!(!opts.xtrace);

            opts.set_short_option('b', true).expect("set -b");
            assert!(opts.notify);

            opts.set_short_option('h', true).expect("set -h");
            assert!(opts.hashall);

            opts.set_short_option('m', true).expect("set -m");
        });
    }

    #[test]
    fn set_named_option_accepts_new_options() {
        assert_no_syscalls(|| {
            let mut opts = ShellOptions::default();
            opts.set_named_option("errexit", true).expect("errexit");
            assert!(opts.errexit);
            opts.set_named_option("xtrace", true).expect("xtrace");
            assert!(opts.xtrace);
            opts.set_named_option("notify", true).expect("notify");
            assert!(opts.notify);
            opts.set_named_option("hashall", true).expect("hashall");
            assert!(opts.hashall);
            opts.set_named_option("monitor", true).expect("monitor");
        });
    }

    #[test]
    fn active_option_flags_includes_new_options() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.errexit = true;
            shell.options.xtrace = true;
            shell.options.notify = true;
            shell.options.hashall = true;
            let flags = shell.active_option_flags();
            assert!(flags.contains('e'));
            assert!(flags.contains('x'));
            assert!(flags.contains('b'));
            assert!(flags.contains('h'));
        });
    }

    #[test]
    fn reset_traps_for_subshell_keeps_ignore_removes_command() {
        run_trace(
            vec![t(
                "signal",
                vec![ArgMatcher::Int(crate::sys::SIGTERM as i64), ArgMatcher::Any],
                TraceResult::Int(0),
            )],
            || {
                let mut shell = test_shell();
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::SIGINT),
                    TrapAction::Ignore,
                );
                shell.trap_actions.insert(
                    TrapCondition::Signal(crate::sys::SIGTERM),
                    TrapAction::Command("echo trapped".into()),
                );
                shell
                    .trap_actions
                    .insert(TrapCondition::Exit, TrapAction::Command("echo bye".into()));

                shell.reset_traps_for_subshell().expect("reset");

                assert_eq!(
                    shell.trap_action(TrapCondition::Signal(crate::sys::SIGINT)),
                    Some(&TrapAction::Ignore),
                );
                assert_eq!(
                    shell.trap_action(TrapCondition::Signal(crate::sys::SIGTERM)),
                    None,
                );
                assert_eq!(shell.trap_action(TrapCondition::Exit), None);
            },
        );
    }

    #[test]
    fn reportable_options_includes_new_options() {
        assert_no_syscalls(|| {
            let mut opts = ShellOptions::default();
            opts.errexit = true;
            opts.xtrace = true;
            let reported = opts.reportable_options();
            let names: Vec<&str> = reported.iter().map(|(n, _)| *n).collect();
            assert!(names.contains(&"errexit"));
            assert!(names.contains(&"xtrace"));
            assert!(names.contains(&"notify"));
            assert!(names.contains(&"hashall"));
            assert!(names.contains(&"monitor"));
            let errexit = reported.iter().find(|(n, _)| *n == "errexit").unwrap();
            assert!(errexit.1);
            let xtrace = reported.iter().find(|(n, _)| *n == "xtrace").unwrap();
            assert!(xtrace.1);
        });
    }

    #[test]
    fn try_wait_child_returns_stopped_for_stopped_process() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(2222), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::StoppedSig(sys::SIGTSTP),
            )],
            || {
                let result = try_wait_child(2222).expect("try_wait_child");
                assert_eq!(result, Some(ChildWaitResult::Stopped(sys::SIGTSTP)));
            },
        );
    }

    #[test]
    fn run_standard_input_retries_read_on_eintr() {
        run_trace(
            vec![
                // ensure_blocking_read_fd: isatty → not a tty
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                    TraceResult::Int(0),
                ),
                // ensure_blocking_read_fd: fstat → regular file (not FIFO)
                t(
                    "fstat",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
                // first read: interrupted
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                // retry: read ':'
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b":".to_vec()),
                ),
                // read newline
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"\n".to_vec()),
                ),
                // read EOF
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let status = shell.run_standard_input().expect("stdin eintr retry");
                assert_eq!(status, 0);
            },
        );
    }

    fn stdin_blocking_trace() -> Vec<test_support::TraceEntry> {
        vec![
            t(
                "isatty",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                TraceResult::Int(0),
            ),
            t(
                "fstat",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                TraceResult::StatFile(0o644),
            ),
        ]
    }

    #[test]
    fn run_standard_input_fatal_read_error() {
        let mut trace = stdin_blocking_trace();
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
            TraceResult::Err(libc::EIO),
        ));
        trace.push(t_stderr("meiksh: Input/output error"));
        run_trace(trace, || {
            let mut shell = test_shell();
            assert!(shell.run_standard_input().is_err());
        });
    }

    #[test]
    fn run_standard_input_eof_with_remaining_bytes() {
        let mut trace = stdin_blocking_trace();
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
            TraceResult::Bytes(b":".to_vec()),
        ));
        trace.push(t(
            "read",
            vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        run_trace(trace, || {
            let mut shell = test_shell();
            let status = shell.run_standard_input().expect("stdin eof partial");
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn maybe_run_stdin_source_parse_error() {
        run_trace(vec![t_stderr("meiksh: line 1: expected command")], || {
            let mut shell = test_shell();
            let mut source = "if true\n".to_string();
            let result = shell.maybe_run_stdin_source(&mut source, false);
            assert!(result.expect("non-eof parse yields None").is_none());

            let mut bad = ")\n".to_string();
            let result = shell.maybe_run_stdin_source(&mut bad, true);
            assert!(result.is_err());
        });
    }

    #[test]
    fn capture_output_reads_data_from_pipe() {
        let child = vec![
            t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
            t(
                "dup2",
                vec![ArgMatcher::Fd(201), ArgMatcher::Fd(1)],
                TraceResult::Int(0),
            ),
            t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
        ];
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(200, 201)),
                t_fork(TraceResult::Pid(1000), child),
                t("close", vec![ArgMatcher::Fd(201)], TraceResult::Int(0)),
                t(
                    "read",
                    vec![ArgMatcher::Fd(200), ArgMatcher::Any],
                    TraceResult::Bytes(b"data".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(200), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(200)], TraceResult::Int(0)),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(1000), ArgMatcher::Any, ArgMatcher::Int(0)],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let output = shell.capture_output(":").expect("capture");
                assert_eq!(output, "data");
            },
        );
    }

    #[test]
    fn command_substitute_maps_error() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Err(sys::EIO)),
                t_stderr("meiksh: Input/output error"),
            ],
            || {
                let mut shell = test_shell();
                let result = crate::expand::Context::command_substitute(&mut shell, "true");
                assert!(result.is_err());
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
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2001),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::StoppedSig(20),
                ),
                t(
                    "tcgetattr",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let id =
                    shell.register_background_job("sleep 99".into(), None, vec![fake_handle(2001)]);
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
            vec![
                t(
                    "tcsetattr",
                    vec![
                        ArgMatcher::Fd(sys::STDIN_FILENO),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(2002),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                let id =
                    shell.register_background_job("exit 0".into(), None, vec![fake_handle(2002)]);
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(3001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::StoppedSig(20),
            )],
            || {
                let mut shell = test_shell();
                shell.register_background_job("sleep".into(), None, vec![fake_handle(3001)]);
                let status = shell.wait_for_pid_operand(3001).expect("wait stopped pid");
                assert_eq!(status, 128 + 20);
            },
        );
    }

    #[test]
    fn wait_on_job_index_stopped() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(4001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::StoppedSig(20),
            )],
            || {
                let mut shell = test_shell();
                shell.register_background_job("sleep".into(), None, vec![fake_handle(4001)]);
                let status = shell
                    .wait_on_job_index(0, false)
                    .expect("wait stopped index");
                assert_eq!(status, 128 + 20);
            },
        );
    }

    #[test]
    fn load_script_source_not_found() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![
                        ArgMatcher::Str("nonexistent-script".into()),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/usr/bin/nonexistent-script".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
                t_stderr("meiksh: nonexistent-script: not found"),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert("PATH".into(), "/usr/bin".into());
                let err = shell.load_script_source(Path::new("nonexistent-script"));
                assert!(err.is_err());
                let e = err.unwrap_err();
                assert_eq!(e.exit_status(), 127);
            },
        );
    }

    #[test]
    fn load_script_source_binary_file_rejected() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("binary-script".into()), ArgMatcher::Int(0)],
                    TraceResult::Int(0),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("binary-script".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"#!/bin/sh\0binary-data".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t_stderr("meiksh: binary-script: cannot execute"),
            ],
            || {
                let shell = test_shell();
                let err = shell.load_script_source(Path::new("binary-script"));
                assert!(err.is_err());
                let e = err.unwrap_err();
                assert_eq!(e.exit_status(), 126);
            },
        );
    }

    #[test]
    fn print_jobs_shows_stopped_running_and_done() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(3001),
                        ArgMatcher::Any,
                        ArgMatcher::Int((sys::WUNTRACED | sys::WNOHANG) as i64),
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] Done 0\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] Stopped (SIGTSTP) sleep 99\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[3] Running sleep 300\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.jobs.push(Job {
                    id: 1,
                    command: "sleep 99".into(),
                    children: vec![],
                    last_pid: None,
                    last_status: None,
                    pgid: None,
                    state: JobState::Stopped(sys::SIGTSTP),
                    saved_termios: None,
                });
                shell.jobs.push(Job {
                    id: 2,
                    command: "exit 0".into(),
                    children: vec![],
                    last_pid: None,
                    last_status: None,
                    pgid: None,
                    state: JobState::Done(0),
                    saved_termios: None,
                });
                shell.jobs.push(Job {
                    id: 3,
                    command: "sleep 300".into(),
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
    fn active_option_flags_monitor_and_syntax_check() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.monitor = true;
            assert!(shell.active_option_flags().contains('m'));
            shell.options.monitor = false;

            shell.options.syntax_check_only = true;
            assert!(shell.active_option_flags().contains('n'));
        });
    }

    #[test]
    fn search_script_path_empty_dir_and_not_found() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str("missing".into()), ArgMatcher::Int(0)],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "getenv",
                    vec![ArgMatcher::Str("PATH".into())],
                    TraceResult::Str(":/nonexistent".into()),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("./missing".into()), ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str("/nonexistent/missing".into()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let shell = test_shell();
                assert_eq!(resolve_script_path(&shell, Path::new("missing")), None);
            },
        );
    }

    #[test]
    fn return_in_dot_sourced_file_exits_source_with_status() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.source_depth = 1;
            let status = shell
                .execute_string(":; return 5; :")
                .expect("return from source");
            assert_eq!(status, 5);
            assert!(shell.pending_control.is_none());
        });
    }

    #[test]
    fn env_for_exec_utility_overlays_and_appends() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert("A".into(), "1".into());
            shell.exported.insert("A".into());
            let env =
                shell.env_for_exec_utility(&[("A".into(), "2".into()), ("B".into(), "3".into())]);
            assert!(env.iter().any(|(k, v)| k == "A" && v == "2"));
            assert!(env.iter().any(|(k, v)| k == "B" && v == "3"));
        });
    }

    #[test]
    fn from_args_constructs_shell_from_argv() {
        run_trace(vec![t("getpid", vec![], TraceResult::Pid(999))], || {
            let shell = Shell::from_args(&["meiksh", "-c", "echo hello"]).expect("from_args");
            assert_eq!(&*shell.shell_name, "meiksh");
        });
    }
}
