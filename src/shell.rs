use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::bstr::{self, ByteWriter};
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
    pub command_string: Option<Box<[u8]>>,
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
    pub script_path: Option<Vec<u8>>,
    pub shell_name_override: Option<Box<[u8]>>,
    pub positional: Vec<Vec<u8>>,
    pub vi_mode: bool,
}

const REPORTABLE_OPTION_NAMES: [(&[u8], u8); 11] = [
    (b"allexport", b'a'),
    (b"errexit", b'e'),
    (b"hashall", b'h'),
    (b"monitor", b'm'),
    (b"noclobber", b'C'),
    (b"noglob", b'f'),
    (b"noexec", b'n'),
    (b"notify", b'b'),
    (b"nounset", b'u'),
    (b"verbose", b'v'),
    (b"xtrace", b'x'),
];

impl ShellOptions {
    pub fn set_short_option(&mut self, ch: u8, enabled: bool) -> Result<(), OptionError> {
        match ch {
            b'a' => self.allexport = enabled,
            b'b' => self.notify = enabled,
            b'C' => self.noclobber = enabled,
            b'e' => self.errexit = enabled,
            b'f' => self.noglob = enabled,
            b'h' => self.hashall = enabled,
            b'i' => self.force_interactive = enabled,
            b'm' => self.monitor = enabled,
            b'n' => self.syntax_check_only = enabled,
            b'u' => self.nounset = enabled,
            b'v' => self.verbose = enabled,
            b'x' => self.xtrace = enabled,
            _ => return Err(OptionError::InvalidShort(ch)),
        }
        Ok(())
    }

    pub fn set_named_option(&mut self, name: &[u8], enabled: bool) -> Result<(), OptionError> {
        if name == b"pipefail" {
            self.pipefail = enabled;
            return Ok(());
        }
        if name == b"vi" {
            self.vi_mode = enabled;
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

    pub fn reportable_options(&self) -> [(&'static [u8], bool); 12] {
        [
            (b"allexport" as &[u8], self.allexport),
            (b"errexit", self.errexit),
            (b"hashall", self.hashall),
            (b"monitor", self.monitor),
            (b"noclobber", self.noclobber),
            (b"noglob", self.noglob),
            (b"noexec", self.syntax_check_only),
            (b"notify", self.notify),
            (b"nounset", self.nounset),
            (b"pipefail", self.pipefail),
            (b"verbose", self.verbose),
            (b"xtrace", self.xtrace),
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

    pub fn message_bytes(&self) -> Vec<u8> {
        crate::bstr::ByteWriter::new()
            .bytes(b"exit status ")
            .i64_val(self.exit_status() as i64)
            .finish()
    }
}

#[derive(Debug)]
pub enum VarError {
    Readonly(Box<[u8]>),
}

#[derive(Debug)]
pub enum OptionError {
    InvalidShort(u8),
    InvalidName(Box<[u8]>),
}

#[derive(Clone)]
pub struct Shell {
    pub options: ShellOptions,
    pub shell_name: Box<[u8]>,
    pub env: HashMap<Vec<u8>, Vec<u8>>,
    pub exported: BTreeSet<Vec<u8>>,
    pub readonly: BTreeSet<Vec<u8>>,
    pub aliases: HashMap<Box<[u8]>, Box<[u8]>>,
    pub functions: HashMap<Vec<u8>, crate::syntax::Command>,
    pub positional: Vec<Vec<u8>>,
    pub last_status: i32,
    pub last_background: Option<sys::Pid>,
    pub running: bool,
    pub jobs: Vec<Job>,
    pub known_pid_statuses: HashMap<sys::Pid, i32>,
    pub known_job_statuses: HashMap<usize, i32>,
    pub trap_actions: BTreeMap<TrapCondition, TrapAction>,
    pub ignored_on_entry: BTreeSet<TrapCondition>,
    pub(crate) subshell_saved_traps: Option<BTreeMap<TrapCondition, TrapAction>>,
    pub loop_depth: usize,
    pub function_depth: usize,
    /// Nesting depth of dot (`source_path`) files being executed.
    pub source_depth: usize,
    pub pending_control: Option<PendingControl>,
    pub(crate) interactive: bool,
    pub(crate) errexit_suppressed: bool,
    pub(crate) owns_terminal: bool,
    pub(crate) in_subshell: bool,
    pub(crate) wait_was_interrupted: bool,
    pub(crate) pid: sys::Pid,
    pub(crate) lineno: usize,
    pub path_cache: HashMap<Box<[u8]>, Vec<u8>>,
    pub history: Vec<Box<[u8]>>,
    pub(crate) mail_last_check: u64,
    pub(crate) mail_sizes: HashMap<Box<[u8]>, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Running,
    Stopped(i32),
    Done(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReapedJobState {
    Stopped(i32, Box<[u8]>),
    Done(i32, Box<[u8]>),
    Signaled(i32, Box<[u8]>),
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: usize,
    pub command: Box<[u8]>,
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
    Command(Box<[u8]>),
}

pub enum FlowSignal {
    Continue(i32),
    UtilityError(i32),
    Exit(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    Exited(i32),
    Signaled(i32),
    Stopped(i32),
    Continued,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockingWaitOutcome {
    Exited(i32),
    Signaled(i32),
    Stopped(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildWaitResult {
    Exited(i32),
    Stopped(i32),
    Interrupted(i32),
}

fn try_wait_child(pid: sys::Pid) -> sys::SysResult<Option<WaitOutcome>> {
    match sys::wait_pid_job_status(pid) {
        Ok(Some(waited)) => {
            if sys::wifcontinued(waited.status) {
                Ok(Some(WaitOutcome::Continued))
            } else if sys::wifstopped(waited.status) {
                Ok(Some(WaitOutcome::Stopped(sys::wstopsig(waited.status))))
            } else if sys::wifsignaled(waited.status) {
                Ok(Some(WaitOutcome::Signaled(sys::wtermsig(waited.status))))
            } else {
                Ok(Some(WaitOutcome::Exited(sys::wexitstatus(waited.status))))
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
    fn diagnostic_at(&self, line: usize, status: i32, msg: &[u8]) -> ShellError {
        if line > 0 && !self.interactive {
            let out = ByteWriter::new()
                .bytes(b"meiksh: line ")
                .usize_val(line)
                .bytes(b": ")
                .bytes(msg)
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &out);
        } else {
            let out = ByteWriter::new()
                .bytes(b"meiksh: ")
                .bytes(msg)
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &out);
        }
        ShellError::Status(status)
    }

    pub fn diagnostic(&self, status: i32, msg: &[u8]) -> ShellError {
        self.diagnostic_at(self.lineno, status, msg)
    }

    pub fn diagnostic_syserr(&self, status: i32, e: &sys::SysError) -> ShellError {
        self.diagnostic_at(self.lineno, status, &e.strerror())
    }

    pub fn diagnostic_prefixed_syserr(
        &self,
        status: i32,
        prefix: &[u8],
        e: &sys::SysError,
    ) -> ShellError {
        let msg = ByteWriter::new()
            .bytes(prefix)
            .bytes(&e.strerror())
            .finish();
        self.diagnostic_at(self.lineno, status, &msg)
    }

    pub fn expand_to_err(&self, e: crate::expand::ExpandError) -> ShellError {
        if !e.message.is_empty() {
            self.diagnostic(1, &e.message);
        }
        ShellError::Status(1)
    }

    pub fn parse_to_err(&self, e: syntax::ParseError) -> ShellError {
        self.diagnostic_at(e.line.unwrap_or(0), 2, &e.message)
    }

    pub fn from_env() -> Result<Self, ShellError> {
        sys::setup_locale();
        let args = sys::env_args_os();
        let options = parse_options(&args)?;
        let shell_name: Box<[u8]> = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| shell_name_from_args(&args).into());
        let raw_env = sys::env_vars();
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
        let _ = sys::default_signal_action(sys::SIGPIPE);
        let ignored_on_entry = Self::probe_ignored_signals();
        let trap_actions: BTreeMap<TrapCondition, TrapAction> = ignored_on_entry
            .iter()
            .map(|&cond| (cond, TrapAction::Ignore))
            .collect();
        env.insert(b"IFS".to_vec(), b" \t\n".to_vec());
        env.insert(
            b"PPID".to_vec(),
            bstr::i64_to_bytes(sys::parent_pid() as i64),
        );
        env.insert(b"OPTIND".to_vec(), b"1".to_vec());
        if !env.contains_key(&b"MAILCHECK".to_vec()) {
            env.insert(b"MAILCHECK".to_vec(), b"600".to_vec());
        }
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
            trap_actions,
            ignored_on_entry,
            subshell_saved_traps: None,
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive,
            errexit_suppressed: false,
            owns_terminal: false,
            in_subshell: false,
            wait_was_interrupted: false,
            pid: sys::current_pid(),
            lineno: 0,
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
        })
    }

    fn init_pwd(env: &mut HashMap<Vec<u8>, Vec<u8>>) {
        let Ok(cwd) = sys::get_cwd() else { return };
        let valid = env.get(b"PWD".as_slice()).is_some_and(|p| {
            p.starts_with(b"/")
                && !p.split(|&b| b == b'/').any(|c| c == b"." || c == b"..")
                && p == &cwd
        });
        if !valid {
            env.insert(b"PWD".to_vec(), cwd);
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
        let args: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
        let options = parse_options(&args)?;
        let shell_name: Box<[u8]> = options
            .shell_name_override
            .clone()
            .unwrap_or_else(|| shell_name_from_args(&args).into());
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
            subshell_saved_traps: None,
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            errexit_suppressed: false,
            owns_terminal: false,
            in_subshell: false,
            wait_was_interrupted: false,
            pid: sys::current_pid(),
            lineno: 0,
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
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
            self.run_source(b"<command>", &command)
        } else if let Some(script) = self.options.script_path.clone() {
            let (resolved, contents) = self.load_script_source(&script)?;
            self.run_source(&resolved, &contents)
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
        sys::ignore_signal(sys::SIGQUIT).map_err(|e| self.diagnostic_syserr(1, &e))?;
        sys::ignore_signal(sys::SIGTERM).map_err(|e| self.diagnostic_syserr(1, &e))?;
        sys::install_shell_signal_handler(sys::SIGINT)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        if self.options.monitor {
            sys::ignore_signal(sys::SIGTSTP).map_err(|e| self.diagnostic_syserr(1, &e))?;
            sys::ignore_signal(sys::SIGTTIN).map_err(|e| self.diagnostic_syserr(1, &e))?;
            sys::ignore_signal(sys::SIGTTOU).map_err(|e| self.diagnostic_syserr(1, &e))?;
        }
        Ok(())
    }

    fn setup_job_control(&mut self) {
        let pid = sys::current_pid();
        let _ = sys::set_process_group(pid, pid);
        let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pid);
        self.owns_terminal = true;
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    pub fn run_source(&mut self, _name: &[u8], source: &[u8]) -> Result<i32, ShellError> {
        self.run_source_buffer(source)
    }

    fn run_source_buffer(&mut self, source: &[u8]) -> Result<i32, ShellError> {
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

    pub fn execute_string(&mut self, source: &[u8]) -> Result<i32, ShellError> {
        self.execute_source_incrementally(source)
    }

    fn run_standard_input(&mut self) -> Result<i32, ShellError> {
        sys::ensure_blocking_read_fd(sys::STDIN_FILENO)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        let mut status = 0;
        let mut source = Vec::new();
        let mut line_bytes = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let count = loop {
                match sys::read_fd(sys::STDIN_FILENO, &mut byte) {
                    Ok(n) => break n,
                    Err(e) if e.is_eintr() => continue,
                    Err(e) => return Err(self.diagnostic_syserr(1, &e)),
                }
            };
            if count == 0 {
                if !line_bytes.is_empty() {
                    let chunk = std::mem::take(&mut line_bytes);
                    self.echo_verbose_input(&chunk);
                    source.extend_from_slice(&chunk);
                }
                break;
            }
            line_bytes.push(byte[0]);
            if byte[0] == b'\n' {
                let chunk = std::mem::take(&mut line_bytes);
                self.echo_verbose_input(&chunk);
                source.extend_from_slice(&chunk);
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
        source: &mut Vec<u8>,
        eof: bool,
        status: &mut i32,
    ) -> Result<(), ShellError> {
        if let Some(s) = self.maybe_run_stdin_source(source, eof)? {
            *status = s;
        }
        Ok(())
    }

    fn execute_source_incrementally(&mut self, source: &[u8]) -> Result<i32, ShellError> {
        let saved_lineno = self.lineno;
        let mut session = syntax::ParseSession::new(source).map_err(|e| self.parse_to_err(e))?;
        let mut status = 0;
        self.run_pending_traps()?;
        loop {
            let prev_pos = session.current_pos();
            let program = match session
                .next_command(&self.aliases)
                .map_err(|e| self.parse_to_err(e))?
            {
                Some(p) => p,
                None => break,
            };
            if self.options.syntax_check_only {
                continue;
            }
            if self.options.verbose {
                let cmd_source = &source[prev_pos..session.current_pos()];
                self.echo_verbose_input(cmd_source);
            }
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
        source: &mut Vec<u8>,
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

    fn echo_verbose_input(&self, source: &[u8]) {
        if self.options.verbose && !source.is_empty() {
            let _ = sys::write_all_fd(sys::STDERR_FILENO, source);
        }
    }

    pub fn capture_output(&mut self, source: &[u8]) -> Result<Vec<u8>, ShellError> {
        let (read_fd, write_fd) = sys::create_pipe().map_err(|e| self.diagnostic_syserr(1, &e))?;
        let pid = sys::fork_process().map_err(|e| self.diagnostic_syserr(1, &e))?;
        if pid == 0 {
            let _ = sys::close_fd(read_fd);
            let _ = sys::duplicate_fd(write_fd, sys::STDOUT_FILENO);
            let _ = sys::close_fd(write_fd);
            let mut child_shell = self.clone();
            child_shell.owns_terminal = false;
            child_shell.in_subshell = true;
            child_shell.restore_signals_for_child();
            let _ = child_shell.reset_traps_for_subshell();
            let status = child_shell.execute_string(source).unwrap_or(1);
            let status = child_shell.run_exit_trap(status).unwrap_or(status);
            sys::exit_process(status as sys::RawFd);
        }
        sys::close_fd(write_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = sys::read_fd(read_fd, &mut buf).map_err(|e| self.diagnostic_syserr(1, &e))?;
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }
        sys::close_fd(read_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let ws = sys::wait_pid(pid, false)
            .map_err(|e| self.diagnostic_syserr(1, &e))?
            .expect("child status");
        let status = sys::decode_wait_status(ws.status);
        self.last_status = status;
        Ok(output)
    }

    pub fn env_for_child(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.exported
            .iter()
            .filter_map(|name| {
                self.env
                    .get(name)
                    .map(|value| (name.clone(), value.clone()))
            })
            .collect()
    }

    pub fn env_for_exec_utility(
        &self,
        cmd_assignments: &[(Vec<u8>, Vec<u8>)],
    ) -> Vec<(Vec<u8>, Vec<u8>)> {
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

    pub fn get_var(&self, name: &[u8]) -> Option<&[u8]> {
        self.env.get(name).map(Vec::as_slice)
    }

    pub fn input_is_incomplete(&self, error: &crate::syntax::ParseError) -> bool {
        stdin_parse_error_requires_more_input(error)
    }

    pub fn history_number(&self) -> usize {
        self.history.len() + 1
    }

    pub fn add_history(&mut self, line: &[u8]) {
        let mut end = line.len();
        while end > 0
            && (line[end - 1] == b' '
                || line[end - 1] == b'\t'
                || line[end - 1] == b'\n'
                || line[end - 1] == b'\r')
        {
            end -= 1;
        }
        let trimmed = &line[..end];
        if trimmed.is_empty() {
            return;
        }
        let histsize = self
            .get_var(b"HISTSIZE")
            .and_then(bstr::parse_i64)
            .and_then(|v| if v >= 0 { Some(v as usize) } else { None })
            .unwrap_or(128);
        if self.history.len() >= histsize && histsize > 0 {
            self.history.remove(0);
        }
        self.history.push(trimmed.into());
    }

    pub fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        if name == b"PATH" {
            self.path_cache.clear();
        }
        if let Some(existing) = self.env.get_mut(name) {
            *existing = value;
        } else {
            self.env.insert(name.to_vec(), value);
        }
        if self.options.allexport && !self.exported.contains(name) {
            self.exported.insert(name.to_vec());
        }
        Ok(())
    }

    pub fn export_var(&mut self, name: &[u8], value: Option<Vec<u8>>) -> Result<(), ShellError> {
        if let Some(value) = value {
            self.set_var(name, value).map_err(|e| {
                let msg = var_error_message(&e);
                self.diagnostic(1, &msg)
            })?;
        }
        if !self.exported.contains(name) {
            self.exported.insert(name.to_vec());
        }
        Ok(())
    }

    pub fn mark_readonly(&mut self, name: &[u8]) {
        self.readonly.insert(name.to_vec());
    }

    pub fn unset_var(&mut self, name: &[u8]) -> Result<(), VarError> {
        if self.readonly.contains(name) {
            return Err(VarError::Readonly(name.into()));
        }
        self.env.remove(name);
        self.exported.remove(name);
        Ok(())
    }

    pub fn set_positional(&mut self, values: Vec<Vec<u8>>) {
        self.positional = values;
    }

    pub fn register_background_job(
        &mut self,
        command: Box<[u8]>,
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

    pub fn wait_for_job(&mut self, id: usize) -> Result<i32, ShellError> {
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
            let _ = sys::set_terminal_attrs(sys::STDIN_FILENO, termios);
        }
        let saved_foreground = if self.owns_terminal {
            if let Some(pg) = pgid {
                let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pg);
            }
            Some(self.pid)
        } else {
            None
        };
        self.jobs[index].state = JobState::Running;
        self.jobs[index].saved_termios = None;
        let mut status = self.jobs[index].last_status.unwrap_or(0);
        let children: Vec<sys::ChildHandle> = self.jobs[index].children.clone();
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
                            sys::get_terminal_attrs(sys::STDIN_FILENO).ok();
                        let msg = ByteWriter::new()
                            .bytes(b"\n[")
                            .usize_val(id)
                            .bytes(b"] Stopped (")
                            .bytes(sys::signal_name(sig))
                            .bytes(b")\t")
                            .bytes(&self.jobs[idx].command)
                            .byte(b'\n')
                            .finish();
                        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
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

    pub fn continue_job(&mut self, id: usize, foreground: bool) -> Result<(), ShellError> {
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
                let _ = sys::set_foreground_pgrp(sys::STDIN_FILENO, pgid);
            }
            sys::send_signal(-pgid, sys::SIGCONT).map_err(|e| self.diagnostic_syserr(1, &e))?;
        } else {
            let pids: Vec<sys::Pid> = self.jobs[idx].children.iter().map(|c| c.pid).collect();
            for pid in pids {
                sys::send_signal(pid, sys::SIGCONT).map_err(|e| self.diagnostic_syserr(1, &e))?;
            }
        }
        Ok(())
    }

    pub fn source_path(&mut self, path: &[u8]) -> Result<i32, ShellError> {
        let contents = sys::read_file(path).map_err(|e| self.diagnostic_syserr(1, &e))?;
        self.source_depth += 1;
        let result = self.execute_string(&contents);
        self.source_depth -= 1;
        result
    }

    fn load_script_source(&self, script: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ShellError> {
        let resolved = resolve_script_path(self, script).ok_or_else(|| {
            let msg = ByteWriter::new()
                .bytes(script)
                .bytes(b": not found")
                .finish();
            self.diagnostic(127, &msg)
        })?;
        let bytes = sys::read_file_bytes(&resolved)
            .map_err(|error| classify_script_read_error(&resolved, error))?;
        if script_prefix_cannot_be_shell_input(&bytes) {
            let msg = ByteWriter::new()
                .bytes(&resolved)
                .bytes(b": cannot execute")
                .finish();
            return Err(self.diagnostic(126, &msg));
        }
        Ok((resolved, bytes))
    }

    pub fn print_jobs(&mut self) {
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
                        let _ = sys::write_all_fd(sys::STDOUT_FILENO, &msg);
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
                        let _ = sys::write_all_fd(sys::STDOUT_FILENO, &msg);
                    }
                }
                ReapedJobState::Signaled(sig, cmd) => {
                    let msg = ByteWriter::new()
                        .bytes(b"[")
                        .usize_val(id)
                        .bytes(b"] Terminated (")
                        .bytes(sys::signal_name(sig))
                        .bytes(b")\t")
                        .bytes(&cmd)
                        .byte(b'\n')
                        .finish();
                    let _ = sys::write_all_fd(sys::STDOUT_FILENO, &msg);
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
                    .bytes(sys::signal_name(sig))
                    .bytes(b") ")
                    .bytes(&job.command)
                    .byte(b'\n')
                    .finish();
                let _ = sys::write_all_fd(sys::STDOUT_FILENO, &msg);
            } else {
                let msg = ByteWriter::new()
                    .bytes(b"[")
                    .usize_val(job.id)
                    .bytes(b"] Running ")
                    .bytes(&job.command)
                    .byte(b'\n')
                    .finish();
                let _ = sys::write_all_fd(sys::STDOUT_FILENO, &msg);
            }
        }
    }

    pub fn run_builtin(
        &mut self,
        argv: &[Vec<u8>],
        assignments: &[(Vec<u8>, Vec<u8>)],
    ) -> Result<FlowSignal, ShellError> {
        for (name, value) in assignments {
            self.set_var(name, value.clone()).map_err(|e| {
                let msg = var_error_message(&e);
                self.diagnostic(1, &msg)
            })?;
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
        self.subshell_saved_traps = None;
        if let TrapCondition::Signal(signal) = condition {
            match action.as_ref() {
                Some(TrapAction::Ignore) => {
                    sys::ignore_signal(signal).map_err(|e| self.diagnostic_syserr(1, &e))?
                }
                Some(TrapAction::Command(_)) => sys::install_shell_signal_handler(signal)
                    .map_err(|e| self.diagnostic_syserr(1, &e))?,
                None => {
                    sys::default_signal_action(signal).map_err(|e| self.diagnostic_syserr(1, &e))?
                }
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
        if self.subshell_saved_traps.is_none() {
            self.subshell_saved_traps = Some(self.trap_actions.clone());
        }
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
                sys::default_signal_action(signal).map_err(|e| self.diagnostic_syserr(1, &e))?;
            }
            self.trap_actions.remove(&cond);
        }
        Ok(())
    }

    pub fn restore_signals_for_child(&self) {
        let user_ignored = |sig: i32| -> bool {
            matches!(
                self.trap_actions.get(&TrapCondition::Signal(sig)),
                Some(TrapAction::Ignore)
            )
        };
        if self.interactive {
            for sig in [sys::SIGTERM, sys::SIGQUIT] {
                if !user_ignored(sig) {
                    let _ = sys::default_signal_action(sig);
                }
            }
            if !user_ignored(sys::SIGINT) {
                let _ = sys::default_signal_action(sys::SIGINT);
            }
        }
        if self.options.monitor {
            for sig in [sys::SIGTSTP, sys::SIGTTIN, sys::SIGTTOU] {
                if !user_ignored(sig) {
                    let _ = sys::default_signal_action(sig);
                }
            }
        }
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

    pub fn wait_for_all_jobs(&mut self) -> Result<i32, ShellError> {
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

    pub(crate) fn run_exit_trap(&mut self, status: i32) -> Result<i32, ShellError> {
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
        action: &[u8],
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

    pub fn wait_for_child_blocking(
        &mut self,
        pid: sys::Pid,
        report_stopped: bool,
    ) -> Result<BlockingWaitOutcome, ShellError> {
        loop {
            match sys::wait_pid_untraced(pid, false) {
                Ok(Some(waited)) => {
                    self.run_pending_traps()?;
                    if sys::wifstopped(waited.status) {
                        if report_stopped {
                            return Ok(BlockingWaitOutcome::Stopped(sys::wstopsig(waited.status)));
                        }
                        continue;
                    } else if sys::wifsignaled(waited.status) {
                        return Ok(BlockingWaitOutcome::Signaled(sys::wtermsig(waited.status)));
                    } else {
                        return Ok(BlockingWaitOutcome::Exited(sys::wexitstatus(waited.status)));
                    }
                }
                Ok(None) => continue,
                Err(error) if sys::interrupted(&error) => {
                    self.run_pending_traps()?;
                    continue;
                }
                Err(error) => return Err(self.diagnostic_syserr(1, &error)),
            }
        }
    }

    pub fn wait_for_child_interruptible(
        &mut self,
        pid: sys::Pid,
    ) -> Result<ChildWaitResult, ShellError> {
        loop {
            match sys::wait_pid_untraced(pid, false) {
                Ok(Some(waited)) => {
                    return if sys::wifstopped(waited.status) {
                        Ok(ChildWaitResult::Stopped(sys::wstopsig(waited.status)))
                    } else if sys::wifsignaled(waited.status) {
                        Ok(ChildWaitResult::Exited(128 + sys::wtermsig(waited.status)))
                    } else {
                        Ok(ChildWaitResult::Exited(sys::wexitstatus(waited.status)))
                    };
                }
                Ok(None) => continue,
                Err(error) if sys::interrupted(&error) && sys::has_pending_signal().is_some() => {
                    let signal = sys::has_pending_signal().unwrap_or(sys::SIGINT);
                    return Ok(ChildWaitResult::Interrupted(128 + signal));
                }
                Err(error) if sys::interrupted(&error) => continue,
                Err(error) => return Err(self.diagnostic_syserr(1, &error)),
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

    pub fn find_job_by_prefix(&self, prefix: &[u8]) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.starts_with(prefix))
            .map(|j| j.id)
    }

    pub fn find_job_by_substring(&self, substring: &[u8]) -> Option<usize> {
        self.jobs
            .iter()
            .find(|j| j.command.windows(substring.len()).any(|w| w == substring))
            .map(|j| j.id)
    }

    fn foreground_handoff(&self, pgid: Option<sys::Pid>) -> Option<sys::Pid> {
        let Some(pgid) = pgid else {
            return None;
        };
        if !self.owns_terminal {
            return None;
        }
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
    fn env_var(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.env.get(name).map(|v| Cow::Borrowed(v.as_slice()))
    }

    fn special_param(&self, name: u8) -> Option<Cow<'_, [u8]>> {
        match name {
            b'?' => Some(Cow::Owned(bstr::i64_to_bytes(self.last_status as i64))),
            b'$' => Some(Cow::Owned(bstr::i64_to_bytes(self.pid as i64))),
            b'!' => self
                .last_background
                .map(|pid| Cow::Owned(bstr::i64_to_bytes(pid as i64))),
            b'#' => Some(Cow::Owned(bstr::u64_to_bytes(self.positional.len() as u64))),
            b'-' => Some(Cow::Owned(self.active_option_flags())),
            b'*' | b'@' => Some(Cow::Owned(bstr::join_bstrings(&self.positional, b" "))),
            b'0' => Some(Cow::Borrowed(&self.shell_name)),
            digit if digit.is_ascii_digit() => {
                let index = (digit - b'0') as usize;
                if index == 0 {
                    return None;
                }
                self.positional
                    .get(index.saturating_sub(1))
                    .map(|v| Cow::Borrowed(v.as_slice()))
            }
            _ => None,
        }
    }

    fn positional_param(&self, index: usize) -> Option<Cow<'_, [u8]>> {
        if index == 0 {
            Some(Cow::Borrowed(&self.shell_name))
        } else {
            self.positional
                .get(index - 1)
                .map(|v| Cow::Borrowed(v.as_slice()))
        }
    }

    fn positional_params(&self) -> &[Vec<u8>] {
        &self.positional
    }

    fn set_var(&mut self, name: &[u8], value: Vec<u8>) -> Result<(), ExpandError> {
        self.set_var(name, value).map_err(|e| {
            let msg = var_error_message(&e);
            ExpandError {
                message: msg.into(),
            }
        })
    }

    fn pathname_expansion_enabled(&self) -> bool {
        !self.options.noglob
    }

    fn nounset_enabled(&self) -> bool {
        self.options.nounset
    }

    fn shell_name(&self) -> &[u8] {
        &self.shell_name
    }

    fn command_substitute(&mut self, command: &[u8]) -> Result<Vec<u8>, ExpandError> {
        self.capture_output(command).map_err(|_| ExpandError {
            message: Vec::new().into(),
        })
    }

    fn home_dir_for_user(&self, name: &[u8]) -> Option<Cow<'_, [u8]>> {
        sys::home_dir_for_user(name).map(Cow::Owned)
    }

    fn set_lineno(&mut self, line: usize) {
        self.lineno = line;
    }
    fn inc_lineno(&mut self) {
        self.lineno += 1;
    }
    fn lineno(&self) -> usize {
        self.lineno
    }
}

fn shell_name_from_args(args: &[Vec<u8>]) -> Vec<u8> {
    args.first().cloned().unwrap_or_else(|| b"meiksh".to_vec())
}

fn parse_options(args: &[Vec<u8>]) -> Result<ShellOptions, ShellError> {
    let mut options = ShellOptions::default();
    let mut index = 1usize;

    while let Some(arg) = args.get(index) {
        if arg == b"-c" {
            let command = args.get(index + 1).ok_or_else(|| {
                let _ = sys::write_all_fd(sys::STDERR_FILENO, b"meiksh: -c requires an argument\n");
                ShellError::Status(2)
            })?;
            options.command_string = Some(command.clone().into());
            options.shell_name_override = args.get(index + 2).map(|s| s.clone().into());
            options.positional = args.iter().skip(index + 3).cloned().collect();
            return Ok(options);
        }
        if arg == b"-o" || arg == b"+o" {
            let enabled = arg == b"-o";
            let name = args.get(index + 1).ok_or_else(|| {
                let out = ByteWriter::new()
                    .bytes(b"meiksh: ")
                    .bytes(arg)
                    .bytes(b" requires an argument\n")
                    .finish();
                let _ = sys::write_all_fd(sys::STDERR_FILENO, &out);
                ShellError::Status(2)
            })?;
            options.set_named_option(name, enabled).map_err(|e| {
                let msg = option_error_message(&e);
                let out = ByteWriter::new()
                    .bytes(b"meiksh: ")
                    .bytes(&msg)
                    .byte(b'\n')
                    .finish();
                let _ = sys::write_all_fd(sys::STDERR_FILENO, &out);
                ShellError::Status(2)
            })?;
            index += 2;
            continue;
        }
        if arg == b"-i" {
            options.force_interactive = true;
            index += 1;
            continue;
        }
        if arg == b"-s" {
            options.positional = args.iter().skip(index + 1).cloned().collect();
            return Ok(options);
        }
        if arg == b"-" {
            index += 1;
            continue;
        }
        if arg == b"--" {
            index += 1;
            break;
        }
        if !arg.is_empty() && (arg[0] == b'-' || arg[0] == b'+') && arg != b"-" && arg != b"+" {
            let enabled = arg[0] == b'-';
            let mut read_stdin = false;
            let mut saw_c = false;
            for &ch in &arg[1..] {
                match ch {
                    b'c' if enabled => saw_c = true,
                    b's' if enabled => read_stdin = true,
                    _ => options.set_short_option(ch, enabled).map_err(|e| {
                        let msg = option_error_message(&e);
                        let out = ByteWriter::new()
                            .bytes(b"meiksh: ")
                            .bytes(&msg)
                            .byte(b'\n')
                            .finish();
                        let _ = sys::write_all_fd(sys::STDERR_FILENO, &out);
                        ShellError::Status(2)
                    })?,
                }
            }
            if saw_c {
                let command = args.get(index + 1).ok_or_else(|| {
                    let _ =
                        sys::write_all_fd(sys::STDERR_FILENO, b"meiksh: -c requires an argument\n");
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
        options.script_path = Some(arg.clone());
        options.shell_name_override = Some(arg.clone().into());
        options.positional = args.iter().skip(index + 1).cloned().collect();
        return Ok(options);
    }

    if index < args.len() {
        options.positional = args.iter().skip(index).cloned().collect();
    }

    Ok(options)
}

fn var_error_message(e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(name)
            .bytes(b": readonly variable")
            .finish(),
    }
}

fn option_error_message(e: &OptionError) -> Vec<u8> {
    match e {
        OptionError::InvalidShort(ch) => ByteWriter::new()
            .bytes(b"invalid option: ")
            .byte(*ch)
            .finish(),
        OptionError::InvalidName(name) => ByteWriter::new()
            .bytes(b"invalid option name: ")
            .bytes(name)
            .finish(),
    }
}

impl Shell {
    fn active_option_flags(&self) -> Vec<u8> {
        let mut flags = Vec::new();
        if self.options.allexport {
            flags.push(b'a');
        }
        if self.options.notify {
            flags.push(b'b');
        }
        if self.options.noclobber {
            flags.push(b'C');
        }
        if self.options.errexit {
            flags.push(b'e');
        }
        if self.options.noglob {
            flags.push(b'f');
        }
        if self.options.hashall {
            flags.push(b'h');
        }
        if self.is_interactive() {
            flags.push(b'i');
        }
        if self.options.monitor {
            flags.push(b'm');
        }
        if self.options.syntax_check_only {
            flags.push(b'n');
        }
        if self.options.nounset {
            flags.push(b'u');
        }
        if self.options.verbose {
            flags.push(b'v');
        }
        if self.options.xtrace {
            flags.push(b'x');
        }
        if self.options.command_string.is_some() {
            flags.push(b'c');
        } else if self.options.script_path.is_none() {
            flags.push(b's');
        }
        flags
    }
}

fn resolve_script_path(shell: &Shell, script: &[u8]) -> Option<Vec<u8>> {
    if script.contains(&b'/') {
        return Some(script.to_vec());
    }

    if sys::file_exists(script) {
        return Some(script.to_vec());
    }

    search_script_path(shell, script)
}

fn search_script_path(shell: &Shell, name: &[u8]) -> Option<Vec<u8>> {
    let path_env_bytes = shell
        .get_var(b"PATH")
        .map(|s| s.to_vec())
        .or_else(|| sys::env_var(b"PATH"))
        .unwrap_or_default();
    for dir in path_env_bytes.split(|&b| b == b':') {
        let base = if dir.is_empty() { b".".as_slice() } else { dir };
        let mut candidate = base.to_vec();
        candidate.push(b'/');
        candidate.extend_from_slice(name);
        if executable_regular_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn executable_regular_file(path: &[u8]) -> bool {
    sys::stat_path(path)
        .map(|stat| stat.is_regular_file() && stat.is_executable())
        .unwrap_or(false)
}

fn stdin_parse_error_requires_more_input(error: &syntax::ParseError) -> bool {
    matches!(
        &*error.message,
        b"unterminated single quote"
            | b"unterminated double quote"
            | b"unterminated here-document"
            | b"expected command"
            | b"expected for loop variable name"
            | b"expected for loop word list"
            | b"expected case word"
            | b"expected 'in'"
            | b"expected case pattern"
            | b"expected ';;' or 'esac'"
            | b"expected redirection target"
            | b"missing here-document body"
            | b"unexpected end of tokens"
            | b"expected 'then'"
            | b"expected 'fi'"
            | b"expected 'do'"
            | b"expected 'done'"
            | b"expected 'esac'"
            | b"expected ')' to close subshell"
            | b"expected '}'"
    )
}

fn classify_script_read_error(path: &[u8], error: sys::SysError) -> ShellError {
    if error.is_enoent() {
        let msg = ByteWriter::new()
            .bytes(b"meiksh: ")
            .bytes(path)
            .bytes(b": not found\n")
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        ShellError::Status(127)
    } else {
        let msg = ByteWriter::new()
            .bytes(b"meiksh: ")
            .bytes(path)
            .bytes(b": ")
            .bytes(&error.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
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
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
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
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
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
                    b"meiksh".to_vec(),
                    b"-c".to_vec(),
                    b"echo ok".to_vec(),
                    b"name".to_vec(),
                    b"arg".to_vec(),
                ])
                .expect("parse");
                assert_eq!(
                    options.command_string.as_deref(),
                    Some(b"echo ok".as_slice())
                );
                assert_eq!(
                    options.shell_name_override.as_deref(),
                    Some(b"name".as_slice())
                );
                assert_eq!(options.positional, vec![b"arg".to_vec()]);

                let options = parse_options(&[
                    b"meiksh".to_vec(),
                    b"-n".to_vec(),
                    b"-i".to_vec(),
                    b"-f".to_vec(),
                    b"script.sh".to_vec(),
                    b"a".to_vec(),
                ])
                .expect("parse");
                assert!(options.syntax_check_only);
                assert!(options.force_interactive);
                assert!(options.noglob);
                assert_eq!(options.script_path, Some(b"script.sh".to_vec()));
                assert_eq!(options.positional, vec![b"a".to_vec()]);

                let options = parse_options(&[
                    b"meiksh".to_vec(),
                    b"-s".to_vec(),
                    b"arg1".to_vec(),
                    b"arg2".to_vec(),
                ])
                .expect("parse -s");
                assert_eq!(options.script_path, None);
                assert_eq!(options.positional, vec![b"arg1".to_vec(), b"arg2".to_vec()]);

                let options =
                    parse_options(&[b"meiksh".to_vec(), b"-is".to_vec(), b"arg".to_vec()])
                        .expect("parse -is");
                assert!(options.force_interactive);
                assert_eq!(options.positional, vec![b"arg".to_vec()]);

                let options = parse_options(&[
                    b"meiksh".to_vec(),
                    b"-a".to_vec(),
                    b"-u".to_vec(),
                    b"-o".to_vec(),
                    b"noglob".to_vec(),
                    b"-v".to_vec(),
                    b"script.sh".to_vec(),
                ])
                .expect("parse -a -u -o noglob -v");
                assert!(options.allexport);
                assert!(options.nounset);
                assert!(options.noglob);
                assert!(options.verbose);
                assert_eq!(options.script_path, Some(b"script.sh".to_vec()));

                let error =
                    parse_options(&[b"meiksh".to_vec(), b"-c".to_vec()]).expect_err("missing arg");
                assert_eq!(error.exit_status(), 2);

                let error = parse_options(&[b"meiksh".to_vec(), b"-o".to_vec()])
                    .expect_err("missing -o arg");
                assert_eq!(error.exit_status(), 2);

                let options = parse_options(&[
                    b"meiksh".to_vec(),
                    b"-o".to_vec(),
                    b"pipefail".to_vec(),
                    b"s.sh".to_vec(),
                ])
                .expect("parse -o pipefail");
                assert!(options.pipefail);

                let error = parse_options(&[b"meiksh".to_vec(), b"-o".to_vec(), b"bogus".to_vec()])
                    .expect_err("bad -o name");
                assert_eq!(error.exit_status(), 2);
            },
        );
    }

    #[test]
    fn env_for_child_filters_exported_values() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"A".to_vec(), b"1".to_vec());
            shell.env.insert(b"B".to_vec(), b"2".to_vec());
            shell.exported.insert(b"A".to_vec());
            let env = shell.env_for_child();
            assert_eq!(
                env.iter()
                    .find(|(k, _)| k == b"A")
                    .map(|(_, v)| v.as_slice()),
                Some(b"1".as_slice())
            );
            assert!(!env.iter().any(|(k, _)| k == b"B"));

            shell.options.allexport = true;
            shell.set_var(b"B", b"3".to_vec()).expect("allexport set");
            let env = shell.env_for_child();
            assert_eq!(
                env.iter()
                    .find(|(k, _)| k == b"B")
                    .map(|(_, v)| v.as_slice()),
                Some(b"3".as_slice())
            );
        });
    }

    #[test]
    fn readonly_variables_reject_mutation_and_unset() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.set_var(b"NAME", b"value".to_vec()).expect("set");
            shell.mark_readonly(b"NAME");
            let set_error = shell
                .set_var(b"NAME", b"new".to_vec())
                .expect_err("readonly");
            let msg = var_error_message(&set_error);
            assert_eq!(msg, b"NAME: readonly variable");
            let unset_error = shell.unset_var(b"NAME").expect_err("readonly");
            let msg = var_error_message(&unset_error);
            assert_eq!(msg, b"NAME: readonly variable");
        });
    }

    #[test]
    fn special_parameters_reflect_shell_state() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.pid = 12345;
            shell.positional = vec![b"first".to_vec(), b"second".to_vec()];
            shell.last_status = 17;
            shell.last_background = Some(42);
            shell.options.allexport = true;
            shell.options.noclobber = true;
            shell.options.command_string = Some(b"printf ok"[..].into());
            assert_eq!(
                expand::Context::special_param(&shell, b'?').as_deref(),
                Some(b"17".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'$').as_deref(),
                Some(b"12345".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'#').as_deref(),
                Some(b"2".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'!').as_deref(),
                Some(b"42".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'-').as_deref(),
                Some(b"aCc".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'*').as_deref(),
                Some(b"first second".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'@').as_deref(),
                Some(b"first second".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'1').as_deref(),
                Some(b"first".as_slice())
            );
            assert_eq!(
                expand::Context::special_param(&shell, b'0').as_deref(),
                Some(b"meiksh".as_slice())
            );
            assert_eq!(expand::Context::special_param(&shell, b'9'), None);
            assert_eq!(expand::Context::special_param(&shell, b'x'), None);
        });
    }

    #[test]
    fn dollar_hyphen_includes_i_when_interactive() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.interactive = true;
            assert!(shell.active_option_flags().contains(&b'i'));
        });
    }

    #[test]
    fn dollar_hyphen_excludes_i_when_not_interactive() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            assert!(!shell.active_option_flags().contains(&b'i'));
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
                let status = shell.source_path(b"/tmp/source-test.sh").expect("source");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), Some(b"42".as_slice()));
            },
        );
    }

    #[test]
    fn export_without_value_marks_variable_exported() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"NAME".to_vec(), b"value".to_vec());
            shell.export_var(b"NAME", None).expect("export");
            assert!(shell.exported.contains(b"NAME".as_slice()));
        });
    }

    #[test]
    fn run_source_syntax_only_parses_without_executing() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.syntax_check_only = true;
            let status = shell
                .run_source(b"<test>", b"echo ok")
                .expect("syntax only");
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
                shell.register_background_job(b"exit 0"[..].into(), None, vec![fake_handle(1001)]);
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(1, ReapedJobState::Done(0, b"exit 0"[..].into()))]
                );
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
                    &[b"export".to_vec(), b"FLOW=1".to_vec()],
                    &[(b"ASSIGN".to_vec(), b"2".to_vec())],
                )
                .expect("builtin");
            assert!(matches!(flow, FlowSignal::Continue(0)));
            assert_eq!(shell.get_var(b"ASSIGN"), Some(b"2".as_slice()));
            assert_eq!(shell.get_var(b"FLOW"), Some(b"1".as_slice()));

            let flow = shell
                .run_builtin(&[b"exit".to_vec(), b"9".to_vec()], &[])
                .expect("exit builtin");
            assert!(matches!(flow, FlowSignal::Exit(9)));

            shell.function_depth = 1;
            let flow = shell
                .run_builtin(&[b"return".to_vec(), b"4".to_vec()], &[])
                .expect("return builtin");
            assert!(matches!(flow, FlowSignal::Continue(4)));
            assert_eq!(shell.pending_control, Some(PendingControl::Return(4)));
            shell.pending_control = None;
            shell.function_depth = 0;

            shell.loop_depth = 2;
            let flow = shell
                .run_builtin(&[b"break".to_vec(), b"5".to_vec()], &[])
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
                    .source_path(b"/definitely/missing-meiksh-script")
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
                let parse_err = syntax::parse(b"echo 'unterminated").expect_err("parse");
                let shell_err = shell.parse_to_err(parse_err);
                assert_eq!(shell_err.exit_status(), 2);

                let expand_err = shell.expand_to_err(ExpandError {
                    message: (*b"expand").into(),
                });
                assert_eq!(expand_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn context_trait_methods_work() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(expand::Context::shell_name(&shell), b"meiksh");
            assert_eq!(
                expand::Context::positional_param(&shell, 0).as_deref(),
                Some(b"meiksh".as_slice())
            );
            expand::Context::set_var(&mut shell, b"CTX_SET", b"7".to_vec()).expect("ctx set");
            assert_eq!(shell.get_var(b"CTX_SET"), Some(b"7".as_slice()));
            shell.mark_readonly(b"CTX_SET");
            let error = expand::Context::set_var(&mut shell, b"CTX_SET", b"8".to_vec())
                .expect_err("readonly ctx set");
            assert_eq!(&*error.message, b"CTX_SET: readonly variable".as_slice());
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
            let output = shell.capture_output(b"true").expect("capture");
            assert_eq!(output, b"");
        });
    }

    #[test]
    fn capture_output_sets_last_status_on_nonzero_exit() {
        run_trace(capture_forked_trace(1, 1000), || {
            let mut shell = test_shell();
            let output = shell.capture_output(b"false").expect("capture ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn command_substitute_success() {
        run_trace(capture_forked_trace(0, 1000), || {
            let mut shell = test_shell();
            let substituted =
                expand::Context::command_substitute(&mut shell, b"true").expect("subst");
            assert_eq!(substituted, b"");
            assert_eq!(shell.last_status, 0);
        });
    }

    #[test]
    fn command_substitute_sets_last_status_on_nonzero_exit() {
        run_trace(capture_forked_trace(1, 1000), || {
            let mut shell = test_shell();
            let output =
                expand::Context::command_substitute(&mut shell, b"false").expect("subst ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn parse_options_covers_dashdash_and_unknown_flags() {
        run_trace(vec![t_stderr("meiksh: invalid option: z")], || {
            let options = parse_options(&[
                b"meiksh".to_vec(),
                b"--".to_vec(),
                b"arg1".to_vec(),
                b"arg2".to_vec(),
            ])
            .expect("parse");
            assert_eq!(options.positional, vec![b"arg1".to_vec(), b"arg2".to_vec()]);

            let error = parse_options(&[b"meiksh".to_vec(), b"-z".to_vec(), b"script.sh".to_vec()])
                .expect_err("invalid option");
            assert_eq!(error.exit_status(), 2);

            let options = parse_options(&[
                b"meiksh".to_vec(),
                b"-fC".to_vec(),
                b"+f".to_vec(),
                b"script.sh".to_vec(),
            ])
            .expect("parse");
            assert!(!options.noglob);
            assert!(options.noclobber);
            assert_eq!(options.script_path, Some(b"script.sh".to_vec()));

            let options = parse_options(&[
                b"meiksh".to_vec(),
                b"-inuv".to_vec(),
                b"+nuv".to_vec(),
                b"script.sh".to_vec(),
            ])
            .expect("parse");
            assert!(options.force_interactive);
            assert!(!options.syntax_check_only);
            assert!(!options.nounset);
            assert!(!options.verbose);
            assert_eq!(options.script_path, Some(b"script.sh".to_vec()));

            let options =
                parse_options(&[b"meiksh".to_vec(), b"-".to_vec()]).expect("parse lone dash");
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
                shell.options.script_path = Some(b"/tmp/run-test.sh".to_vec());
                let status = shell.run().expect("run");
                assert_eq!(status, 0);
                assert_eq!(shell.get_var(b"VALUE"), Some(b"77".as_slice()));
            },
        );
    }

    #[test]
    fn capture_output_sets_last_status_127() {
        run_trace(capture_forked_trace(127, 1000), || {
            let mut shell = test_shell();
            let output = shell.capture_output(b"exit 127").expect("capture ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 127);
        });
    }

    #[test]
    fn shell_error_status_helpers_work() {
        run_trace(vec![t_stderr("meiksh: missing script")], || {
            let shell = test_shell();
            let error = shell.diagnostic(127, b"missing script");
            assert_eq!(error.exit_status(), 127);

            let silent = ShellError::Status(42);
            assert_eq!(silent.exit_status(), 42);
        });
    }

    #[test]
    fn stdin_parse_error_requires_more_input_for_open_constructs() {
        assert_no_syscalls(|| {
            for source in [
                &b"if true\n"[..],
                b"for item in a b\n",
                b"cat <<EOF\nhello\n",
                b"echo \"unterminated",
                b"printf ok |\n",
            ] {
                let error = syntax::parse(source).expect_err("incomplete parse");
                assert!(
                    stdin_parse_error_requires_more_input(&error),
                    "expected more input for: {:?}",
                    source,
                );
            }

            let program =
                syntax::parse(b"999999999999999999999999999999999999999999999999999999999999<in")
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
                shell.env.insert(b"PATH".to_vec(), b"/search-path".to_vec());
                assert_eq!(
                    resolve_script_path(&shell, b"cwd-script"),
                    Some(b"cwd-script".to_vec())
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
                shell.env.insert(b"PATH".to_vec(), b"/search-path".to_vec());
                assert_eq!(
                    resolve_script_path(&shell, b"path-script"),
                    Some(b"/search-path/path-script".to_vec())
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
                let classified =
                    classify_script_read_error(b"missing", sys::SysError::Errno(sys::ENOENT));
                assert_eq!(classified.exit_status(), 127);
                let classified = classify_script_read_error(b"bad", sys::SysError::Errno(sys::EIO));
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
            shell.options.command_string = Some(b"VALUE=13"[..].into());
            let status = shell.run().expect("run command string");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"13".as_slice()));
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
                let error = shell.capture_output(b"true").expect_err("fork error");
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
                        ArgMatcher::Bytes(b"[1] Done\tsleep\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(1003)]);
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
                .execute_string(b"alias setok='export VALUE=ok'")
                .expect("define alias");
            let status = shell.execute_string(b"setok").expect("run alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"ok".as_slice()));

            let status = shell
                .execute_string(b"alias same='export SAME=1'\nsame")
                .expect("run same-source alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"SAME"), Some(b"1".as_slice()));

            shell.aliases.insert(b"cond"[..].into(), b"if"[..].into());
            let status = shell
                .execute_string(b"cond true; then export BRANCH=hit; fi")
                .expect("run reserved-word alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"BRANCH"), Some(b"hit".as_slice()));

            let status = shell
                .execute_string(b"alias cond2='if'\ncond2 true; then export TOP=ok; fi")
                .expect("run same-source reserved alias");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"TOP"), Some(b"ok".as_slice()));

            shell
                .aliases
                .insert(b"chain"[..].into(), b"eval "[..].into());
            shell
                .aliases
                .insert(b"word"[..].into(), b"VALUE=chain"[..].into());
            let status = shell
                .execute_string(b"chain word")
                .expect("run blank alias chain");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"chain".as_slice()));
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
                        ArgMatcher::Bytes(b"[1] Done\tdone\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
        run_trace(
            vec![t(
                "write",
                vec![
                    ArgMatcher::Fd(2),
                    ArgMatcher::Bytes(
                        b"meiksh: wait: pid 999999 is not a child of this shell\n".to_vec(),
                    ),
                ],
                TraceResult::Auto,
            )],
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
                let mut shell = test_shell();
                shell.owns_terminal = true;
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
                        .execute_trap_action(b"exit 9", 3)
                        .expect("exit trap action"),
                    9
                );
                assert!(!shell.running);
                assert_eq!(shell.last_status, 9);
                shell.running = true;

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(b":"[..].into())),
                    )
                    .expect("trap");
                sys::test_support::with_pending_signals_for_test(&[sys::SIGINT], || {
                    shell.run_pending_traps().expect("run traps");
                });
                assert_eq!(shell.last_status, 9);

                shell
                    .set_trap(
                        TrapCondition::Signal(sys::SIGINT),
                        Some(TrapAction::Command(b"exit 7"[..].into())),
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
                    b"sleep"[..].into(),
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
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(2002)]);
                assert!(shell.wait_for_job_operand(1).is_err());
                assert!(shell.wait_for_child_blocking(99, true).is_err());
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
    fn parse_options_combined_c_with_other_flags() {
        run_trace(vec![t_stderr("meiksh: -c requires an argument")], || {
            let options = parse_options(&[
                b"meiksh".to_vec(),
                b"-ac".to_vec(),
                b"echo ok".to_vec(),
                b"name".to_vec(),
            ])
            .expect("parse -ac");
            assert!(options.allexport);
            assert_eq!(
                options.command_string.as_deref(),
                Some(b"echo ok".as_slice())
            );
            assert_eq!(
                options.shell_name_override.as_deref(),
                Some(b"name".as_slice())
            );

            let options =
                parse_options(&[b"meiksh".to_vec(), b"-euc".to_vec(), b"echo ok".to_vec()])
                    .expect("parse -euc");
            assert!(options.errexit);
            assert!(options.nounset);
            assert_eq!(
                options.command_string.as_deref(),
                Some(b"echo ok".as_slice())
            );

            let error =
                parse_options(&[b"meiksh".to_vec(), b"-ec".to_vec()]).expect_err("missing -c arg");
            assert_eq!(error.exit_status(), 2);
        });
    }

    #[test]
    fn set_short_option_accepts_new_options() {
        assert_no_syscalls(|| {
            let mut opts = ShellOptions::default();
            opts.set_short_option(b'e', true).expect("set -e");
            assert!(opts.errexit);
            opts.set_short_option(b'e', false).expect("set +e");
            assert!(!opts.errexit);

            opts.set_short_option(b'x', true).expect("set -x");
            assert!(opts.xtrace);
            opts.set_short_option(b'x', false).expect("set +x");
            assert!(!opts.xtrace);

            opts.set_short_option(b'b', true).expect("set -b");
            assert!(opts.notify);

            opts.set_short_option(b'h', true).expect("set -h");
            assert!(opts.hashall);

            opts.set_short_option(b'm', true).expect("set -m");
        });
    }

    #[test]
    fn set_named_option_accepts_new_options() {
        assert_no_syscalls(|| {
            let mut opts = ShellOptions::default();
            opts.set_named_option(b"errexit", true).expect("errexit");
            assert!(opts.errexit);
            opts.set_named_option(b"xtrace", true).expect("xtrace");
            assert!(opts.xtrace);
            opts.set_named_option(b"notify", true).expect("notify");
            assert!(opts.notify);
            opts.set_named_option(b"hashall", true).expect("hashall");
            assert!(opts.hashall);
            opts.set_named_option(b"monitor", true).expect("monitor");
            opts.set_named_option(b"vi", true).expect("vi");
            assert!(opts.vi_mode);
            opts.set_named_option(b"vi", false).expect("vi off");
            assert!(!opts.vi_mode);
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
            assert!(flags.contains(&b'e'));
            assert!(flags.contains(&b'x'));
            assert!(flags.contains(&b'b'));
            assert!(flags.contains(&b'h'));
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
                    TrapAction::Command(b"echo trapped"[..].into()),
                );
                shell.trap_actions.insert(
                    TrapCondition::Exit,
                    TrapAction::Command(b"echo bye"[..].into()),
                );

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
            let names: Vec<&[u8]> = reported.iter().map(|(n, _)| *n).collect();
            assert!(names.contains(&b"errexit".as_slice()));
            assert!(names.contains(&b"xtrace".as_slice()));
            assert!(names.contains(&b"notify".as_slice()));
            assert!(names.contains(&b"hashall".as_slice()));
            assert!(names.contains(&b"monitor".as_slice()));
            let errexit = reported.iter().find(|(n, _)| *n == b"errexit").unwrap();
            assert!(errexit.1);
            let xtrace = reported.iter().find(|(n, _)| *n == b"xtrace").unwrap();
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
                assert_eq!(result, Some(WaitOutcome::Stopped(sys::SIGTSTP)));
            },
        );
    }

    #[test]
    fn run_standard_input_retries_read_on_eintr() {
        run_trace(
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
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Interrupt(sys::SIGINT),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b":".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Any],
                    TraceResult::Bytes(b"\n".to_vec()),
                ),
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
            let mut source = b"if true\n".to_vec();
            let result = shell.maybe_run_stdin_source(&mut source, false);
            assert!(result.expect("non-eof parse yields None").is_none());

            let mut bad = b")\n".to_vec();
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
                let output = shell.capture_output(b":").expect("capture");
                assert_eq!(output, b"data");
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
                let result = crate::expand::Context::command_substitute(&mut shell, b"true");
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
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"\n[1] Stopped (SIGTSTP)\tsleep 99\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(3001)]);
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
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(4001)]);
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
                shell.env.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
                let err = shell.load_script_source(b"nonexistent-script");
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
                let err = shell.load_script_source(b"binary-script");
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
                        ArgMatcher::Int((sys::WUNTRACED | sys::WCONTINUED | sys::WNOHANG) as i64),
                    ],
                    TraceResult::Int(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] Done\texit 0\n".to_vec()),
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
                    command: b"sleep 99"[..].into(),
                    children: vec![],
                    last_pid: None,
                    last_status: None,
                    pgid: None,
                    state: JobState::Stopped(sys::SIGTSTP),
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
    fn active_option_flags_monitor_and_syntax_check() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.monitor = true;
            assert!(shell.active_option_flags().contains(&b'm'));
            shell.options.monitor = false;

            shell.options.syntax_check_only = true;
            assert!(shell.active_option_flags().contains(&b'n'));
        });
    }

    #[test]
    fn search_script_path_empty_dir_and_not_found() {
        run_trace(
            vec![
                t(
                    "access",
                    vec![ArgMatcher::Str(b"missing".to_vec()), ArgMatcher::Int(0)],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "getenv",
                    vec![ArgMatcher::Str(b"PATH".to_vec())],
                    TraceResult::StrVal(b":/nonexistent".to_vec()),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str(b"./missing".to_vec()), ArgMatcher::Any],
                    TraceResult::Err(sys::ENOENT),
                ),
                t(
                    "stat",
                    vec![
                        ArgMatcher::Str(b"/nonexistent/missing".to_vec()),
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::ENOENT),
                ),
            ],
            || {
                let shell = test_shell();
                assert_eq!(resolve_script_path(&shell, b"missing"), None);
            },
        );
    }

    #[test]
    fn return_in_dot_sourced_file_exits_source_with_status() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.source_depth = 1;
            let status = shell
                .execute_string(b":; return 5; :")
                .expect("return from source");
            assert_eq!(status, 5);
            assert!(shell.pending_control.is_none());
        });
    }

    #[test]
    fn env_for_exec_utility_overlays_and_appends() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.env.insert(b"A".to_vec(), b"1".to_vec());
            shell.exported.insert(b"A".to_vec());
            let env = shell.env_for_exec_utility(&[
                (b"A".to_vec(), b"2".to_vec()),
                (b"B".to_vec(), b"3".to_vec()),
            ]);
            assert!(env.iter().any(|(k, v)| k == b"A" && v == b"2"));
            assert!(env.iter().any(|(k, v)| k == b"B" && v == b"3"));
        });
    }

    #[test]
    fn from_args_constructs_shell_from_argv() {
        run_trace(vec![t("getpid", vec![], TraceResult::Pid(999))], || {
            let shell = Shell::from_args(&["meiksh", "-c", "echo hello"]).expect("from_args");
            assert_eq!(&*shell.shell_name, b"meiksh");
        });
    }

    #[test]
    fn shell_error_message_bytes_and_exit_status() {
        let err = ShellError::Status(42);
        assert_eq!(err.message_bytes(), b"exit status 42");
        assert_eq!(err.exit_status(), 42);
        assert!(err.message_bytes().windows(2).any(|w| w == b"42"));
    }

    #[test]
    fn wait_on_job_index_blocking_exited() {
        run_trace(
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(5001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::Status(0),
            )],
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
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5002),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Err(sys::ECHILD),
                ),
                t_stderr("meiksh: No child processes"),
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(5003),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::StoppedSig(sys::SIGTSTP),
            )],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"sleep"[..].into(), None, vec![fake_handle(5003)]);
                let status = shell
                    .wait_on_job_index(0, true)
                    .expect("wait interruptible stopped");
                assert_eq!(status, 128 + sys::SIGTSTP);
            },
        );
    }

    #[test]
    fn wait_for_child_interruptible_retries_on_pid_zero() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5004),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Pid(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5004),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(42),
                ),
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
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(3333), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::ContinuedStatus,
            )],
            || {
                let result = try_wait_child(3333).expect("try_wait_child");
                assert_eq!(result, Some(WaitOutcome::Continued));
            },
        );
    }

    #[test]
    fn try_wait_child_returns_signaled() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(3334), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::SignaledSig(9),
            )],
            || {
                let result = try_wait_child(3334).expect("try_wait_child");
                assert_eq!(result, Some(WaitOutcome::Signaled(9)));
            },
        );
    }

    #[test]
    fn reap_jobs_signaled_child() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(4001), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::SignaledSig(9),
            )],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"killed"[..].into(), None, vec![fake_handle(4001)]);
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(1, ReapedJobState::Signaled(9, b"killed"[..].into()))]
                );
                assert!(shell.jobs.is_empty());
            },
        );
    }

    #[test]
    fn reap_jobs_continued_child_transitions_to_running() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(4002), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::ContinuedStatus,
            )],
            || {
                let mut shell = test_shell();
                let id = shell.register_background_job(
                    b"cont"[..].into(),
                    None,
                    vec![fake_handle(4002)],
                );
                shell.jobs[0].state = JobState::Stopped(sys::SIGTSTP);
                let finished = shell.reap_jobs();
                assert!(finished.is_empty());
                let job = shell.jobs.iter().find(|j| j.id == id).expect("job");
                assert!(matches!(job.state, JobState::Running));
            },
        );
    }

    #[test]
    fn reap_jobs_stopped_then_continued() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4003), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4003), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::ContinuedStatus,
                ),
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
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4005), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(4005), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.register_background_job(b"stopped"[..].into(), None, vec![fake_handle(4005)]);
                let finished = shell.reap_jobs();
                assert_eq!(
                    finished,
                    vec![(
                        1,
                        ReapedJobState::Stopped(sys::SIGTSTP, b"stopped"[..].into())
                    )]
                );
                assert!(matches!(
                    shell.jobs[0].state,
                    JobState::Stopped(sys::SIGTSTP)
                ));
            },
        );
    }

    #[test]
    fn reap_jobs_signaled_produces_finished_entry() {
        run_trace(
            vec![t(
                "waitpid",
                vec![ArgMatcher::Int(4004), ArgMatcher::Any, ArgMatcher::Any],
                TraceResult::SignaledSig(15),
            )],
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(5001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::SignaledSig(9),
            )],
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(5003),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::Status(42),
            )],
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
            vec![
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(6001)],
                    TraceResult::Int(0),
                ),
                t(
                    "kill",
                    vec![ArgMatcher::Int(-6001), ArgMatcher::Int(sys::SIGCONT as i64)],
                    TraceResult::Int(0),
                ),
            ],
            || {
                let mut shell = test_shell();
                shell.owns_terminal = true;
                let id = shell.register_background_job(
                    b"fg"[..].into(),
                    Some(6001),
                    vec![fake_handle(6001)],
                );
                shell.jobs[0].state = JobState::Stopped(sys::SIGTSTP);
                shell.continue_job(id, true).expect("continue");
                assert!(matches!(shell.jobs[0].state, JobState::Running));
            },
        );
    }

    #[test]
    fn print_jobs_signaled_and_done_nonzero() {
        run_trace(
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7001), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::SignaledSig(15),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7002), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(3),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[1] Terminated (SIGTERM)\tsig-job\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes(b"[2] Done(3)\tfail-job\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(8001),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::SignaledSig(11),
            )],
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(9002),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::SignaledSig(6),
            )],
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
                    TraceResult::Pid(1000),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(2000)],
                    TraceResult::Int(0),
                ),
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
            vec![t(
                "isatty",
                vec![ArgMatcher::Fd(sys::STDIN_FILENO)],
                TraceResult::Int(0),
            )],
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
            vec![
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(5010)],
                    TraceResult::Int(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5010),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::SignaledSig(9),
                ),
                t(
                    "tcsetpgrp",
                    vec![ArgMatcher::Fd(sys::STDIN_FILENO), ArgMatcher::Int(100)],
                    TraceResult::Int(0),
                ),
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
            vec![
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7010), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::StoppedSig(sys::SIGTSTP),
                ),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(7010), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Pid(0),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(1),
                        ArgMatcher::Bytes({
                            let mut v = b"[1] Stopped (".to_vec();
                            v.extend_from_slice(sys::signal_name(sys::SIGTSTP));
                            v.extend_from_slice(b") stopped-job\n");
                            v
                        }),
                    ],
                    TraceResult::Auto,
                ),
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
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5020),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(5021),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(0),
                ),
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
            vec![t(
                "waitpid",
                vec![
                    ArgMatcher::Int(8010),
                    ArgMatcher::Any,
                    ArgMatcher::Int(sys::WUNTRACED as i64),
                ],
                TraceResult::SignaledSig(11),
            )],
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
            vec![
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(7070),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::StoppedSig(19),
                ),
                t(
                    "waitpid",
                    vec![
                        ArgMatcher::Int(7070),
                        ArgMatcher::Any,
                        ArgMatcher::Int(sys::WUNTRACED as i64),
                    ],
                    TraceResult::Status(42),
                ),
            ],
            || {
                let mut shell = test_shell();
                let outcome = shell.wait_for_child_blocking(7070, false).expect("wait");
                assert_eq!(outcome, BlockingWaitOutcome::Exited(42));
            },
        );
    }

    #[test]
    fn syntax_check_only_skips_execution() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            shell.options.syntax_check_only = true;
            let status = shell
                .execute_string(b"echo this should not run; false")
                .expect("syntax check");
            assert_eq!(status, 0);
        });
    }

    #[test]
    fn add_history_skips_empty_and_respects_histsize() {
        let mut shell = test_shell();
        shell.add_history(b"");
        shell.add_history(b"   ");
        assert!(shell.history.is_empty());

        shell.add_history(b"first");
        assert_eq!(shell.history.len(), 1);

        shell.env.insert(b"HISTSIZE".to_vec(), b"2".to_vec());
        shell.add_history(b"second");
        shell.add_history(b"third");
        assert_eq!(shell.history.len(), 2);
        assert_eq!(&*shell.history[0], b"second".as_slice());
        assert_eq!(&*shell.history[1], b"third".as_slice());
    }

    #[test]
    fn export_var_error_on_readonly() {
        run_trace(vec![t_stderr("meiksh: RO: readonly variable")], || {
            let mut shell = test_shell();
            shell.set_var(b"RO", b"orig".to_vec()).expect("set");
            shell.mark_readonly(b"RO");
            let error = shell
                .export_var(b"RO", Some(b"new".to_vec()))
                .expect_err("readonly export");
            assert_eq!(error.exit_status(), 1);
        });
    }

    #[test]
    fn set_trap_noop_when_signal_ignored_on_entry() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let cond = TrapCondition::Signal(sys::SIGQUIT);
            shell.ignored_on_entry.insert(cond);
            shell
                .set_trap(cond, Some(TrapAction::Command(b"echo trapped"[..].into())))
                .expect("set_trap");
            assert!(shell.trap_action(cond).is_none());
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
