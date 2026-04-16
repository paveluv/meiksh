use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::bstr::{self, ByteWriter};
use crate::builtin::{self, BuiltinOutcome};
use crate::exec::program;
use crate::interactive;
use crate::syntax;
use crate::syntax::ast::Program;
use crate::sys;

use super::error::{ShellError, var_error_message};
use super::options::{ShellOptions, option_error_message};
use super::state::{FlowSignal, PendingControl, SharedEnv, Shell};
use super::traps::{TrapAction, TrapCondition};

pub fn run_from_env() -> i32 {
    match Shell::from_env().and_then(|mut shell| shell.run()) {
        Ok(code) => code,
        Err(err) => err.exit_status(),
    }
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
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &out);
        } else {
            let out = ByteWriter::new()
                .bytes(b"meiksh: ")
                .bytes(msg)
                .byte(b'\n')
                .finish();
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &out);
        }
        ShellError::Status(status)
    }

    pub(crate) fn diagnostic(&self, status: i32, msg: &[u8]) -> ShellError {
        self.diagnostic_at(self.lineno, status, msg)
    }

    pub(crate) fn diagnostic_syserr(&self, status: i32, e: &sys::error::SysError) -> ShellError {
        self.diagnostic_at(self.lineno, status, &e.strerror())
    }

    pub(crate) fn diagnostic_prefixed_syserr(
        &self,
        status: i32,
        prefix: &[u8],
        e: &sys::error::SysError,
    ) -> ShellError {
        let msg = ByteWriter::new()
            .bytes(prefix)
            .bytes(&e.strerror())
            .finish();
        self.diagnostic_at(self.lineno, status, &msg)
    }

    pub(crate) fn expand_to_err(&self, e: crate::expand::core::ExpandError) -> ShellError {
        if !e.message.is_empty() {
            self.diagnostic(1, &e.message);
        }
        ShellError::Status(1)
    }

    pub(crate) fn parse_to_err(&self, e: syntax::ParseError) -> ShellError {
        self.diagnostic_at(e.line.unwrap_or(0), 2, &e.message)
    }

    pub(crate) fn from_env() -> Result<Self, ShellError> {
        sys::locale::setup_locale();
        let args = sys::env::env_args_os();
        let mut options = parse_options(&args)?;
        let shell_name: Box<[u8]> = options
            .shell_name_override
            .take()
            .unwrap_or_else(|| shell_name_from_args(&args).into());
        let raw_env = sys::env::env_vars();
        let mut env = HashMap::new();
        let mut exported = BTreeSet::new();
        for (key, value) in raw_env {
            if crate::syntax::is_name(&key) {
                exported.insert(key.clone());
                env.insert(key, value);
            }
        }
        let interactive = options.force_interactive
            || (sys::tty::is_interactive_fd(sys::constants::STDIN_FILENO)
                && sys::tty::is_interactive_fd(sys::constants::STDERR_FILENO));
        let _ = sys::process::default_signal_action(sys::constants::SIGPIPE);
        let ignored_on_entry = Self::probe_ignored_signals();
        let trap_actions: BTreeMap<TrapCondition, TrapAction> = ignored_on_entry
            .iter()
            .map(|&cond| (cond, TrapAction::Ignore))
            .collect();
        env.insert(b"IFS".to_vec(), b" \t\n".to_vec());
        env.insert(
            b"PPID".to_vec(),
            bstr::i64_to_bytes(sys::process::parent_pid() as i64),
        );
        env.insert(b"OPTIND".to_vec(), b"1".to_vec());
        if !env.contains_key(b"MAILCHECK".as_slice()) {
            env.insert(b"MAILCHECK".to_vec(), b"600".to_vec());
        }
        Self::init_pwd(&mut env);
        let positional = std::mem::take(&mut options.positional);
        Ok(Self {
            positional,
            options,
            shell_name,
            shared: Rc::new(SharedEnv {
                env,
                exported,
                readonly: BTreeSet::new(),
                aliases: HashMap::new(),
                functions: HashMap::new(),
                path_cache: HashMap::new(),
                history: Vec::new(),
                mail_sizes: HashMap::new(),
            }),
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
            pid: sys::process::current_pid(),
            lineno: 0,
            mail_last_check: 0,
        })
    }

    fn init_pwd(env: &mut HashMap<Vec<u8>, Vec<u8>>) {
        let Ok(cwd) = sys::fs::get_cwd() else { return };
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
        for signal in sys::process::supported_trap_signals() {
            if sys::process::query_signal_disposition(signal).unwrap_or(false) {
                set.insert(TrapCondition::Signal(signal));
            }
        }
        set
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn from_args(args: &[&str]) -> Result<Self, ShellError> {
        let args: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
        let mut options = parse_options(&args)?;
        let shell_name: Box<[u8]> = options
            .shell_name_override
            .take()
            .unwrap_or_else(|| shell_name_from_args(&args).into());
        let positional = std::mem::take(&mut options.positional);
        Ok(Self {
            positional,
            interactive: options.force_interactive,
            options,
            shell_name,
            shared: Rc::new(SharedEnv {
                env: HashMap::new(),
                exported: BTreeSet::new(),
                readonly: BTreeSet::new(),
                aliases: HashMap::new(),
                functions: HashMap::new(),
                path_cache: HashMap::new(),
                history: Vec::new(),
                mail_sizes: HashMap::new(),
            }),
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
            pid: sys::process::current_pid(),
            lineno: 0,
            mail_last_check: 0,
        })
    }

    pub(crate) fn run(&mut self) -> Result<i32, ShellError> {
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
        } else if let Some(ref script) = self.options.script_path {
            let (resolved, contents) = self.load_script_source(script)?;
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

    pub(super) fn setup_interactive_signals(&self) -> Result<(), ShellError> {
        sys::process::ignore_signal(sys::constants::SIGQUIT)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        sys::process::ignore_signal(sys::constants::SIGTERM)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        sys::process::install_shell_signal_handler(sys::constants::SIGINT)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        if self.options.monitor {
            sys::process::ignore_signal(sys::constants::SIGTSTP)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
            sys::process::ignore_signal(sys::constants::SIGTTIN)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
            sys::process::ignore_signal(sys::constants::SIGTTOU)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
        }
        Ok(())
    }

    fn setup_job_control(&mut self) {
        let pid = sys::process::current_pid();
        let _ = sys::tty::set_process_group(pid, pid);
        let _ = sys::tty::set_foreground_pgrp(sys::constants::STDIN_FILENO, pid);
        self.owns_terminal = true;
    }

    pub(crate) fn is_interactive(&self) -> bool {
        self.interactive
    }

    pub(crate) fn run_source(&mut self, _name: &[u8], source: &[u8]) -> Result<i32, ShellError> {
        self.run_source_buffer(source)
    }

    fn run_source_buffer(&mut self, source: &[u8]) -> Result<i32, ShellError> {
        if self.options.syntax_check_only {
            let _ = syntax::parse_with_aliases(source, &self.aliases())
                .map_err(|e| self.parse_to_err(e))?;
            return Ok(0);
        }
        self.execute_source_incrementally(source)
    }

    pub(crate) fn execute_program(&mut self, program: &Program) -> Result<i32, ShellError> {
        let status = program::execute_program(self, program)?;
        self.last_status = status;
        Ok(status)
    }

    pub(crate) fn execute_string(&mut self, source: &[u8]) -> Result<i32, ShellError> {
        self.execute_source_incrementally(source)
    }

    pub(crate) fn run_standard_input(&mut self) -> Result<i32, ShellError> {
        sys::fd_io::ensure_blocking_read_fd(sys::constants::STDIN_FILENO)
            .map_err(|e| self.diagnostic_syserr(1, &e))?;
        let mut status = 0;
        let mut source = Vec::new();
        let mut line_bytes = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let count = loop {
                match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut byte) {
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
                .next_command(&self.aliases())
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

    pub(super) fn maybe_run_stdin_source(
        &mut self,
        source: &mut Vec<u8>,
        eof: bool,
    ) -> Result<Option<i32>, ShellError> {
        if source.is_empty() {
            return Ok(None);
        }
        match syntax::parse_with_aliases(source, &self.aliases()) {
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
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, source);
        }
    }

    pub(crate) fn capture_output_program(
        &mut self,
        program: &Rc<Program>,
    ) -> Result<Vec<u8>, ShellError> {
        let (read_fd, write_fd) =
            sys::fd_io::create_pipe().map_err(|e| self.diagnostic_syserr(1, &e))?;
        let pid = sys::process::fork_process().map_err(|e| self.diagnostic_syserr(1, &e))?;
        if pid == 0 {
            let _ = sys::fd_io::close_fd(read_fd);
            let _ = sys::fd_io::duplicate_fd(write_fd, sys::constants::STDOUT_FILENO);
            let _ = sys::fd_io::close_fd(write_fd);
            let mut child_shell = self.clone();
            child_shell.owns_terminal = false;
            child_shell.in_subshell = true;
            child_shell.restore_signals_for_child();
            let _ = child_shell.reset_traps_for_subshell();
            let status = child_shell.execute_program(program).unwrap_or(1);
            let status = child_shell.run_exit_trap(status).unwrap_or(status);
            sys::process::exit_process(status as sys::types::RawFd);
        }
        sys::fd_io::close_fd(write_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = sys::fd_io::read_fd(read_fd, &mut buf)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }
        sys::fd_io::close_fd(read_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let ws = sys::process::wait_pid(pid, false)
            .map_err(|e| self.diagnostic_syserr(1, &e))?
            .expect("child status");
        let status = sys::process::decode_wait_status(ws.status);
        self.last_status = status;
        Ok(output)
    }

    #[cfg(test)]
    pub(crate) fn capture_output(&mut self, source: &[u8]) -> Result<Vec<u8>, ShellError> {
        let (read_fd, write_fd) =
            sys::fd_io::create_pipe().map_err(|e| self.diagnostic_syserr(1, &e))?;
        let pid = sys::process::fork_process().map_err(|e| self.diagnostic_syserr(1, &e))?;
        if pid == 0 {
            let _ = sys::fd_io::close_fd(read_fd);
            let _ = sys::fd_io::duplicate_fd(write_fd, sys::constants::STDOUT_FILENO);
            let _ = sys::fd_io::close_fd(write_fd);
            let mut child_shell = self.clone();
            child_shell.owns_terminal = false;
            child_shell.in_subshell = true;
            child_shell.restore_signals_for_child();
            let _ = child_shell.reset_traps_for_subshell();
            let status = child_shell.execute_string(source).unwrap_or(1);
            let status = child_shell.run_exit_trap(status).unwrap_or(status);
            sys::process::exit_process(status as sys::types::RawFd);
        }
        sys::fd_io::close_fd(write_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let mut output = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = sys::fd_io::read_fd(read_fd, &mut buf)
                .map_err(|e| self.diagnostic_syserr(1, &e))?;
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }
        sys::fd_io::close_fd(read_fd).map_err(|e| self.diagnostic_syserr(1, &e))?;
        let ws = sys::process::wait_pid(pid, false)
            .map_err(|e| self.diagnostic_syserr(1, &e))?
            .expect("child status");
        let status = sys::process::decode_wait_status(ws.status);
        self.last_status = status;
        Ok(output)
    }

    pub(crate) fn load_script_source(
        &self,
        script: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), ShellError> {
        let resolved = resolve_script_path(self, script).ok_or_else(|| {
            let msg = ByteWriter::new()
                .bytes(script)
                .bytes(b": not found")
                .finish();
            self.diagnostic(127, &msg)
        })?;
        let bytes = sys::fs::read_file_bytes(&resolved)
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
}

impl Shell {
    pub(crate) fn run_builtin(
        &mut self,
        argv: &[Vec<u8>],
        assignments: &[(Vec<u8>, Vec<u8>)],
    ) -> Result<FlowSignal, ShellError> {
        for (name, value) in assignments {
            self.set_var(name, value).map_err(|e| {
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

    pub(crate) fn has_pending_control(&self) -> bool {
        self.pending_control.is_some()
    }
}

impl Shell {
    pub(super) fn active_option_flags(&self) -> Vec<u8> {
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

pub(super) fn shell_name_from_args(args: &[Vec<u8>]) -> Vec<u8> {
    args.first().cloned().unwrap_or_else(|| b"meiksh".to_vec())
}

pub(super) fn parse_options(args: &[Vec<u8>]) -> Result<ShellOptions, ShellError> {
    let mut options = ShellOptions::default();
    let mut index = 1usize;

    while let Some(arg) = args.get(index) {
        if arg == b"-c" {
            let command = args.get(index + 1).ok_or_else(|| {
                let _ = sys::fd_io::write_all_fd(
                    sys::constants::STDERR_FILENO,
                    b"meiksh: -c requires an argument\n",
                );
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
                let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &out);
                ShellError::Status(2)
            })?;
            options.set_named_option(name, enabled).map_err(|e| {
                let msg = option_error_message(&e);
                let out = ByteWriter::new()
                    .bytes(b"meiksh: ")
                    .bytes(&msg)
                    .byte(b'\n')
                    .finish();
                let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &out);
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
                        let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &out);
                        ShellError::Status(2)
                    })?,
                }
            }
            if saw_c {
                let command = args.get(index + 1).ok_or_else(|| {
                    let _ = sys::fd_io::write_all_fd(
                        sys::constants::STDERR_FILENO,
                        b"meiksh: -c requires an argument\n",
                    );
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

pub(super) fn resolve_script_path(shell: &Shell, script: &[u8]) -> Option<Vec<u8>> {
    if script.contains(&b'/') {
        return Some(script.to_vec());
    }

    if sys::fs::file_exists(script) {
        return Some(script.to_vec());
    }

    search_script_path(shell, script)
}

pub(super) fn search_script_path(shell: &Shell, name: &[u8]) -> Option<Vec<u8>> {
    let path_env_bytes = shell
        .get_var(b"PATH")
        .map(|s| s.to_vec())
        .or_else(|| sys::env::env_var(b"PATH"))
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

pub(super) fn executable_regular_file(path: &[u8]) -> bool {
    sys::fs::stat_path(path)
        .map(|stat| stat.is_regular_file() && stat.is_executable())
        .unwrap_or(false)
}

pub(super) fn stdin_parse_error_requires_more_input(error: &syntax::ParseError) -> bool {
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

pub(super) fn classify_script_read_error(path: &[u8], error: sys::error::SysError) -> ShellError {
    if error.is_enoent() {
        let msg = ByteWriter::new()
            .bytes(b"meiksh: ")
            .bytes(path)
            .bytes(b": not found\n")
            .finish();
        let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
        ShellError::Status(127)
    } else {
        let msg = ByteWriter::new()
            .bytes(b"meiksh: ")
            .bytes(path)
            .bytes(b": ")
            .bytes(&error.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
        ShellError::Status(128)
    }
}

pub(super) fn script_prefix_cannot_be_shell_input(bytes: &[u8]) -> bool {
    const PREFIX_LEN: usize = 4096;
    let prefix = &bytes[..bytes.len().min(PREFIX_LEN)];
    let newline = prefix.iter().position(|&byte| byte == b'\n');
    let scan_end = newline.unwrap_or(prefix.len());
    prefix[..scan_end].contains(&b'\0')
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

    use crate::syntax;
    use crate::sys;
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};

    use crate::shell::state::Shell;
    use crate::shell::test_support::{capture_forked_trace, t_stderr, test_shell};
    use crate::trace_entries;

    #[test]
    fn parse_options_handles_command_script_and_errors() {
        run_trace(
            trace_entries![
                ..vec![
                    t_stderr("meiksh: -c requires an argument"),
                    t_stderr("meiksh: -o requires an argument"),
                    t_stderr("meiksh: invalid option name: bogus"),
                ],
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
    fn setup_interactive_signals_ignores_sigquit_sigterm_installs_sigint() {
        run_trace(
            trace_entries![
                signal(int(sys::constants::SIGQUIT), any) -> 0,
                signal(int(sys::constants::SIGTERM), any) -> 0,
                signal(int(sys::constants::SIGINT), any) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                shell.setup_interactive_signals().expect("signal setup");
            },
        );
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
    fn capture_output_success() {
        run_trace(trace_entries![..capture_forked_trace(0, 1000)], || {
            let mut shell = test_shell();
            let output = shell.capture_output(b"true").expect("capture");
            assert_eq!(output, b"");
        });
    }

    #[test]
    fn capture_output_sets_last_status_on_nonzero_exit() {
        run_trace(trace_entries![..capture_forked_trace(1, 1000)], || {
            let mut shell = test_shell();
            let output = shell.capture_output(b"false").expect("capture ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 1);
        });
    }

    #[test]
    fn parse_options_covers_dashdash_and_unknown_flags() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: invalid option: z")]],
            || {
                let options = parse_options(&[
                    b"meiksh".to_vec(),
                    b"--".to_vec(),
                    b"arg1".to_vec(),
                    b"arg2".to_vec(),
                ])
                .expect("parse");
                assert_eq!(options.positional, vec![b"arg1".to_vec(), b"arg2".to_vec()]);

                let error =
                    parse_options(&[b"meiksh".to_vec(), b"-z".to_vec(), b"script.sh".to_vec()])
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
            },
        );
    }

    #[test]
    fn shell_run_executes_script_from_path() {
        run_trace(
            trace_entries![
                open(str("/tmp/run-test.sh"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"VALUE=77\n"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
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
        run_trace(trace_entries![..capture_forked_trace(127, 1000)], || {
            let mut shell = test_shell();
            let output = shell.capture_output(b"exit 127").expect("capture ok");
            assert_eq!(output, b"");
            assert_eq!(shell.last_status, 127);
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
            trace_entries![access(str("cwd-script"), int(0)) -> 0,],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/search-path".to_vec());
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
            trace_entries![
                access(str("path-script"), int(0)) -> err(sys::constants::ENOENT),
                stat(str("/search-path/path-script"), any) -> stat_file(0o755),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/search-path".to_vec());
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
            trace_entries![
                ..vec![
                    t_stderr("meiksh: missing: not found"),
                    t_stderr("meiksh: bad: Input/output error"),
                ],
            ],
            || {
                let classified = classify_script_read_error(
                    b"missing",
                    sys::error::SysError::Errno(sys::constants::ENOENT),
                );
                assert_eq!(classified.exit_status(), 127);
                let classified = classify_script_read_error(
                    b"bad",
                    sys::error::SysError::Errno(sys::constants::EIO),
                );
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
        run_trace(trace_entries![], || {
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
            trace_entries![
                pipe() -> fds(200, 201),
                fork() -> err(sys::constants::EINVAL),
                write(
                    fd(sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: Invalid argument\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let error = shell.capture_output(b"true").expect_err("fork error");
                assert_ne!(error.exit_status(), 0);
            },
        );
    }

    #[test]
    fn execute_string_uses_current_alias_table() {
        run_trace(trace_entries![], || {
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

            shell
                .aliases_mut()
                .insert(b"cond"[..].into(), b"if"[..].into());
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
                .aliases_mut()
                .insert(b"chain"[..].into(), b"eval "[..].into());
            shell
                .aliases_mut()
                .insert(b"word"[..].into(), b"VALUE=chain"[..].into());
            let status = shell
                .execute_string(b"chain word")
                .expect("run blank alias chain");
            assert_eq!(status, 0);
            assert_eq!(shell.get_var(b"VALUE"), Some(b"chain".as_slice()));
        });
    }

    #[test]
    fn parse_options_combined_c_with_other_flags() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: -c requires an argument")]],
            || {
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

                let error = parse_options(&[b"meiksh".to_vec(), b"-ec".to_vec()])
                    .expect_err("missing -c arg");
                assert_eq!(error.exit_status(), 2);
            },
        );
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
    fn run_standard_input_retries_read_on_eintr() {
        run_trace(
            trace_entries![
                isatty(fd(sys::constants::STDIN_FILENO)) -> 0,
                fstat(fd(sys::constants::STDIN_FILENO), any) -> stat_file(0o644),
                read(fd(sys::constants::STDIN_FILENO), _) -> interrupt(sys::constants::SIGINT),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b":"),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b"\n"),
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let status = shell.run_standard_input().expect("stdin eintr retry");
                assert_eq!(status, 0);
            },
        );
    }

    fn stdin_blocking_trace() -> Vec<crate::sys::test_support::TraceEntry> {
        trace_entries![
            isatty(fd(sys::constants::STDIN_FILENO)) -> 0,
            fstat(fd(sys::constants::STDIN_FILENO), any) -> stat_file(0o644),
        ]
    }

    #[test]
    fn run_standard_input_fatal_read_error() {
        run_trace(
            trace_entries![
                ..stdin_blocking_trace(),
                read(fd(sys::constants::STDIN_FILENO), _) -> err(libc::EIO),
                write(
                    fd(sys::constants::STDERR_FILENO),
                    bytes(b"meiksh: Input/output error\n"),
                ) -> auto,
            ],
            || {
                let mut shell = test_shell();
                assert!(shell.run_standard_input().is_err());
            },
        );
    }

    #[test]
    fn run_standard_input_eof_with_remaining_bytes() {
        run_trace(
            trace_entries![
                ..stdin_blocking_trace(),
                read(fd(sys::constants::STDIN_FILENO), _) -> bytes(b":"),
                read(fd(sys::constants::STDIN_FILENO), _) -> 0,
            ],
            || {
                let mut shell = test_shell();
                let status = shell.run_standard_input().expect("stdin eof partial");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn maybe_run_stdin_source_parse_error() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: line 1: expected command")]],
            || {
                let mut shell = test_shell();
                let mut source = b"if true\n".to_vec();
                let result = shell.maybe_run_stdin_source(&mut source, false);
                assert!(result.expect("non-eof parse yields None").is_none());

                let mut bad = b")\n".to_vec();
                let result = shell.maybe_run_stdin_source(&mut bad, true);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn capture_output_reads_data_from_pipe() {
        run_trace(
            trace_entries![
                pipe() -> fds(200, 201),
                fork() -> pid(1000), child: [
                    close(fd(200)) -> 0,
                    dup2(fd(201), fd(sys::constants::STDOUT_FILENO)) -> 0,
                    close(fd(201)) -> 0,
                ],
                close(fd(201)) -> 0,
                read(fd(200), _) -> bytes(b"data"),
                read(fd(200), _) -> 0,
                close(fd(200)) -> 0,
                waitpid(int(1000), any, int(0)) -> status(0),
            ],
            || {
                let mut shell = test_shell();
                let output = shell.capture_output(b":").expect("capture");
                assert_eq!(output, b"data");
            },
        );
    }

    #[test]
    fn load_script_source_not_found() {
        run_trace(
            trace_entries![
                access(str("nonexistent-script"), int(0)) -> err(sys::constants::ENOENT),
                stat(str("/usr/bin/nonexistent-script"), any) -> err(sys::constants::ENOENT),
                ..vec![t_stderr("meiksh: nonexistent-script: not found")],
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"PATH".to_vec(), b"/usr/bin".to_vec());
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
            trace_entries![
                access(str("binary-script"), int(0)) -> 0,
                open(str("binary-script"), any, any) -> fd(10),
                read(fd(10), _) -> bytes(b"#!/bin/sh\0binary-data"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
                ..vec![t_stderr("meiksh: binary-script: cannot execute")],
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
            trace_entries![
                access(str(b"missing"), int(0)) -> err(sys::constants::ENOENT),
                ..vec![t(
                    "getenv",
                    vec![ArgMatcher::Str(b"PATH".to_vec())],
                    TraceResult::StrVal(b":/nonexistent".to_vec()),
                )],
                stat(str(b"./missing"), any) -> err(sys::constants::ENOENT),
                stat(str(b"/nonexistent/missing"), any) -> err(sys::constants::ENOENT),
            ],
            || {
                let shell = test_shell();
                assert_eq!(resolve_script_path(&shell, b"missing"), None);
            },
        );
    }

    #[test]
    fn from_args_constructs_shell_from_argv() {
        run_trace(trace_entries![getpid() -> pid(999),], || {
            let shell = Shell::from_args(&["meiksh", "-c", "echo hello"]).expect("from_args");
            assert_eq!(&*shell.shell_name, b"meiksh");
        });
    }

    #[test]
    fn run_builtin_assignment_error() {
        run_trace(
            trace_entries![
                write(fd(2), bytes(b"meiksh: x: readonly variable\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"x");
                let result =
                    shell.run_builtin(&[b"eval".to_vec()], &[(b"x".to_vec(), b"2".to_vec())]);
                match result {
                    Err(crate::shell::error::ShellError::Status(1)) => (),
                    _ => panic!("Expected Err(Status(1))"),
                }
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
}
