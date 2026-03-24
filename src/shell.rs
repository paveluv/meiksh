use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, ExitStatus};

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
            eprintln!("meiksh: {err}");
            1
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ShellOptions {
    pub command_string: Option<String>,
    pub syntax_check_only: bool,
    pub force_interactive: bool,
    pub noclobber: bool,
    pub noglob: bool,
    pub script_path: Option<PathBuf>,
    pub positional: Vec<String>,
}

#[derive(Debug)]
pub struct ShellError {
    pub message: String,
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ShellError {}

impl From<io::Error> for ShellError {
    fn from(value: io::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<syntax::ParseError> for ShellError {
    fn from(value: syntax::ParseError) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<ExpandError> for ShellError {
    fn from(value: ExpandError) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

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
    pub last_background: Option<u32>,
    pub running: bool,
    pub jobs: Vec<Job>,
    pub current_exe: PathBuf,
    pub loop_depth: usize,
    pub function_depth: usize,
    pub pending_control: Option<PendingControl>,
}

pub struct Job {
    pub id: usize,
    pub command: String,
    pub children: Vec<Child>,
}

pub enum FlowSignal {
    Continue(i32),
    Exit(i32),
}

fn try_wait_child(child: &mut Child) -> io::Result<Option<ExitStatus>> {
    #[cfg(test)]
    {
        if let Some(override_fn) = TEST_TRY_WAIT.with(|cell| *cell.borrow()) {
            return override_fn(child);
        }
    }
    child.try_wait()
}

#[cfg(test)]
thread_local! {
    static TEST_TRY_WAIT: std::cell::RefCell<Option<fn(&mut Child) -> io::Result<Option<ExitStatus>>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn with_try_wait_for_test<T>(
    override_fn: fn(&mut Child) -> io::Result<Option<ExitStatus>>,
    f: impl FnOnce() -> T,
) -> T {
    TEST_TRY_WAIT.with(|cell| {
        let previous = cell.replace(Some(override_fn));
        let result = f();
        cell.replace(previous);
        result
    })
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
        let shell_name = sys::shell_name_from_args(&args).to_string();
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
            current_exe: std::env::current_exe()?,
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        })
    }

    pub fn run(&mut self) -> Result<i32, ShellError> {
        if let Some(command) = self.options.command_string.clone() {
            return self.run_source("<command>", &command);
        }

        if let Some(script) = self.options.script_path.clone() {
            let contents = fs::read_to_string(&script)?;
            return self.run_source(script.to_string_lossy().as_ref(), &contents);
        }

        let interactive = self.options.force_interactive
            || (sys::is_interactive_fd(sys::STDIN_FILENO) && sys::is_interactive_fd(sys::STDERR_FILENO));
        if interactive {
            interactive::run(self)
        } else {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            self.run_source("<stdin>", &buffer)
        }
    }

    pub fn run_source(&mut self, _name: &str, source: &str) -> Result<i32, ShellError> {
        let program = syntax::parse_with_aliases(source, &self.aliases)?;
        if self.options.syntax_check_only {
            return Ok(0);
        }
        self.execute_program(&program)
    }

    pub fn execute_program(&mut self, program: &Program) -> Result<i32, ShellError> {
        let status = exec::execute_program(self, program)?;
        self.last_status = status;
        Ok(status)
    }

    pub fn execute_string(&mut self, source: &str) -> Result<i32, ShellError> {
        let program = syntax::parse_with_aliases(source, &self.aliases)?;
        self.execute_program(&program)
    }

    pub fn capture_output(&mut self, source: &str) -> Result<String, ShellError> {
        let mut command = ProcessCommand::new(&self.current_exe);
        command.arg("-c").arg(source);
        command.env_clear();
        command.envs(self.env_for_child());
        let output = command.output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(ShellError {
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
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

    pub fn launch_background_job(&mut self, command: String, children: Vec<Child>) -> usize {
        let id = self.jobs.last().map(|job| job.id + 1).unwrap_or(1);
        if let Some(last) = children.last() {
            self.last_background = Some(last.id());
        }
        self.jobs.push(Job { id, command, children });
        id
    }

    pub fn reap_jobs(&mut self) -> Vec<(usize, i32)> {
        let mut finished = Vec::new();
        let mut remaining = Vec::new();

        for mut job in self.jobs.drain(..) {
            let mut all_done = true;
            let mut final_status = 0;
            for child in &mut job.children {
                match try_wait_child(child) {
                    Ok(Some(status)) => {
                        final_status = status.code().unwrap_or(128);
                    }
                    Ok(None) => {
                        all_done = false;
                    }
                    Err(_) => {
                        final_status = 1;
                    }
                }
            }
            if all_done {
                finished.push((job.id, final_status));
            } else {
                remaining.push(job);
            }
        }

        self.jobs = remaining;
        finished
    }

    pub fn wait_for_job(&mut self, id: usize) -> Result<i32, ShellError> {
        let index = self
            .jobs
            .iter()
            .position(|job| job.id == id)
            .ok_or_else(|| ShellError {
                message: format!("job {id}: not found"),
            })?;
        let mut job = self.jobs.remove(index);
        let mut status = 0;
        for child in &mut job.children {
            status = child.wait()?.code().unwrap_or(128);
        }
        self.last_status = status;
        Ok(status)
    }

    pub fn continue_job(&mut self, id: usize) -> Result<(), ShellError> {
        let job = self.jobs.iter().find(|job| job.id == id).ok_or_else(|| ShellError {
            message: format!("job {id}: not found"),
        })?;
        for child in &job.children {
            sys::send_signal(child.id() as i32, sys::SIGCONT)?;
        }
        Ok(())
    }

    pub fn source_path(&mut self, path: &Path) -> Result<i32, ShellError> {
        let contents = fs::read_to_string(path)?;
        self.execute_string(&contents)
    }

    pub fn print_jobs(&mut self) {
        let finished = self.reap_jobs();
        for (id, status) in finished {
            eprintln!("[{id}] Done {status}");
        }
        for job in &self.jobs {
            eprintln!("[{}] Running {}", job.id, job.command);
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
            '*' | '@' => Some(self.positional.join(" ")),
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
                message: "-c requires an argument".to_string(),
            })?;
            options.command_string = Some(command.clone());
            options.positional = args.iter().skip(index + 3).cloned().collect();
            return Ok(options);
        }
        if arg == "-n" {
            options.syntax_check_only = true;
            index += 1;
            continue;
        }
        if arg == "-i" {
            options.force_interactive = true;
            index += 1;
            continue;
        }
        if arg == "--" {
            index += 1;
            break;
        }
        if (arg.starts_with('-') || arg.starts_with('+')) && arg != "-" && arg != "+" {
            let enabled = arg.starts_with('-');
            for ch in arg[1..].chars() {
                match ch {
                    'C' => options.noclobber = enabled,
                    'f' => options.noglob = enabled,
                    'i' => options.force_interactive = enabled,
                    'n' => options.syntax_check_only = enabled,
                    _ => {}
                }
            }
            index += 1;
            continue;
        }
        options.script_path = Some(PathBuf::from(arg));
        options.positional = args.iter().skip(index + 1).cloned().collect();
        return Ok(options);
    }

    if index < args.len() {
        options.positional = args.iter().skip(index).cloned().collect();
    }

    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn meiksh_bin_path() -> PathBuf {
        let exe = std::env::current_exe().expect("current exe");
        exe.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("meiksh"))
            .expect("meiksh path")
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
            current_exe: meiksh_bin_path(),
            loop_depth: 0,
            function_depth: 0,
            pending_control: None,
        }
    }

    #[test]
    fn parse_options_handles_command_script_and_errors() {
        let options = parse_options(&["meiksh".into(), "-c".into(), "echo ok".into(), "name".into(), "arg".into()])
            .expect("parse");
        assert_eq!(options.command_string.as_deref(), Some("echo ok"));
        assert_eq!(options.positional, vec!["arg".to_string()]);

        let options =
            parse_options(&["meiksh".into(), "-n".into(), "-i".into(), "-f".into(), "script.sh".into(), "a".into()])
            .expect("parse");
        assert!(options.syntax_check_only);
        assert!(options.force_interactive);
        assert!(options.noglob);
        assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));
        assert_eq!(options.positional, vec!["a".to_string()]);

        let error = parse_options(&["meiksh".into(), "-c".into()]).expect_err("missing arg");
        assert_eq!(error.message, "-c requires an argument");
    }

    #[test]
    fn env_for_child_filters_exported_values() {
        let mut shell = test_shell();
        shell.env.insert("A".into(), "1".into());
        shell.env.insert("B".into(), "2".into());
        shell.exported.insert("A".into());
        let env = shell.env_for_child();
        assert_eq!(env.get("A").map(String::as_str), Some("1"));
        assert!(!env.contains_key("B"));
    }

    #[test]
    fn readonly_variables_reject_mutation_and_unset() {
        let mut shell = test_shell();
        shell.set_var("NAME", "value".into()).expect("set");
        shell.mark_readonly("NAME");
        let set_error = shell.set_var("NAME", "new".into()).expect_err("readonly");
        assert_eq!(set_error.message, "NAME: readonly variable");
        let unset_error = shell.unset_var("NAME").expect_err("readonly");
        assert_eq!(unset_error.message, "NAME: readonly variable");
    }

    #[test]
    fn special_parameters_reflect_shell_state() {
        let mut shell = test_shell();
        shell.positional = vec!["first".into(), "second".into()];
        shell.last_status = 17;
        shell.last_background = Some(42);
        assert_eq!(expand::Context::special_param(&shell, '?').as_deref(), Some("17"));
        assert!(expand::Context::special_param(&shell, '$').is_some());
        assert_eq!(expand::Context::special_param(&shell, '#').as_deref(), Some("2"));
        assert_eq!(expand::Context::special_param(&shell, '!').as_deref(), Some("42"));
        assert_eq!(expand::Context::special_param(&shell, '*').as_deref(), Some("first second"));
        assert_eq!(expand::Context::special_param(&shell, '@').as_deref(), Some("first second"));
        assert_eq!(expand::Context::special_param(&shell, '1').as_deref(), Some("first"));
        assert_eq!(expand::Context::special_param(&shell, '0').as_deref(), Some("meiksh"));
        assert_eq!(expand::Context::special_param(&shell, '9'), None);
        assert_eq!(expand::Context::special_param(&shell, 'x'), None);
    }

    #[test]
    fn launch_and_wait_for_background_job_updates_state() {
        let mut shell = test_shell();
        let child = ProcessCommand::new("sh")
            .args(["-c", "exit 7"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("exit 7".into(), vec![child]);
        let status = shell.wait_for_job(id).expect("wait");
        assert_eq!(status, 7);
        assert_eq!(shell.last_status, 7);
        assert!(shell.jobs.is_empty());
    }

    #[test]
    fn source_path_runs_script() {
        let mut shell = test_shell();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("meiksh-source-{unique}.sh"));
        fs::write(&path, "VALUE=42\n").expect("write");
        let status = shell.source_path(&path).expect("source");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("42"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_without_value_and_run_source_syntax_only_work() {
        let mut shell = test_shell();
        shell.env.insert("NAME".into(), "value".into());
        shell.export_var("NAME", None).expect("export");
        assert!(shell.exported.contains("NAME"));

        shell.options.syntax_check_only = true;
        let status = shell.run_source("<test>", "echo ok").expect("syntax only");
        assert_eq!(status, 0);
        assert_eq!(shell.last_status, 0);
    }

    #[test]
    fn reap_jobs_and_run_builtin_cover_flow_variants() {
        let mut shell = test_shell();
        let child = ProcessCommand::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("exit 0".into(), vec![child]);
        let mut finished = Vec::new();
        for _ in 0..20 {
            finished = shell.reap_jobs();
            if !finished.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(finished, vec![(id, 0)]);
        assert!(shell.jobs.is_empty());

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
    }

    #[test]
    fn reap_jobs_handles_try_wait_errors() {
        fn fake_try_wait(_child: &mut Child) -> io::Result<Option<ExitStatus>> {
            Err(io::Error::other("boom"))
        }

        let mut shell = test_shell();
        let child = ProcessCommand::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        let id = shell.launch_background_job("exit 0".into(), vec![child]);

        let finished = with_try_wait_for_test(fake_try_wait, || shell.reap_jobs());
        assert_eq!(finished, vec![(id, 1)]);
        assert!(shell.jobs.is_empty());
    }

    #[test]
    fn continue_job_and_source_path_error_when_missing() {
        let mut shell = test_shell();
        let error = shell.continue_job(99).expect_err("missing job");
        assert_eq!(error.message, "job 99: not found");

        let error = shell.wait_for_job(99).expect_err("missing job");
        assert_eq!(error.message, "job 99: not found");

        let error = shell.source_path(Path::new("/definitely/missing-meiksh-script")).expect_err("missing source");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn shell_helpers_cover_display_conversions_and_capture() {
        let parse_err = syntax::parse("echo 'unterminated").expect_err("parse");
        let shell_err: ShellError = parse_err.into();
        assert!(!shell_err.message.is_empty());

        let expand_err: ShellError = ExpandError { message: "expand".into() }.into();
        assert_eq!(expand_err.message, "expand");
        assert_eq!(format!("{}", shell_err), shell_err.message);

        let mut shell = test_shell();
        shell.env.insert("PATH".into(), "/usr/bin:/bin".into());
        shell.exported.insert("PATH".into());
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
    }

    #[test]
    fn parse_options_covers_dashdash_and_unknown_flags() {
        let options = parse_options(&["meiksh".into(), "--".into(), "arg1".into(), "arg2".into()])
            .expect("parse");
        assert_eq!(options.positional, vec!["arg1".to_string(), "arg2".to_string()]);

        let options = parse_options(&["meiksh".into(), "-z".into(), "script.sh".into()])
            .expect("parse");
        assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

        let options = parse_options(&["meiksh".into(), "-fC".into(), "+f".into(), "script.sh".into()])
            .expect("parse");
        assert!(!options.noglob);
        assert!(options.noclobber);
        assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));

        let options = parse_options(&["meiksh".into(), "-in".into(), "+n".into(), "script.sh".into()])
            .expect("parse");
        assert!(options.force_interactive);
        assert!(!options.syntax_check_only);
        assert_eq!(options.script_path, Some(PathBuf::from("script.sh")));
    }

    #[test]
    fn shell_run_covers_script_path_and_capture_error() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("meiksh-run-{unique}.sh"));
        fs::write(&path, "VALUE=77\n").expect("write");

        let mut shell = test_shell();
        shell.options.script_path = Some(path.clone());
        let status = shell.run().expect("run");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("77"));

        let error = shell.capture_output("missing-command").expect_err("capture error");
        assert!(!error.message.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn shell_run_command_string_and_capture_spawn_error_paths_work() {
        let mut shell = test_shell();
        shell.options.command_string = Some("VALUE=13".into());
        let status = shell.run().expect("run command string");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("13"));

        shell.current_exe = PathBuf::from("/definitely/missing-meiksh-binary");
        let error = shell.capture_output("printf hi").expect_err("spawn error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn print_jobs_covers_running_and_finished_paths() {
        let mut shell = test_shell();
        let finished_child = ProcessCommand::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        shell.launch_background_job("done".into(), vec![finished_child]);

        for _ in 0..20 {
            if !shell.reap_jobs().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let running_child = ProcessCommand::new("sh")
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        shell.launch_background_job("sleep".into(), vec![running_child]);
        shell.print_jobs();

        if let Some(id) = shell.jobs.first().map(|job| job.id) {
            let _ = shell.wait_for_job(id);
        }
    }

    #[test]
    fn execute_string_uses_current_alias_table() {
        let mut shell = test_shell();
        shell.execute_string("alias setok='export VALUE=ok'").expect("define alias");
        let status = shell.execute_string("setok").expect("run alias");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("VALUE").as_deref(), Some("ok"));

        shell.aliases.insert("cond".into(), "if".into());
        let status = shell
            .execute_string("cond true; then export BRANCH=hit; fi")
            .expect("run reserved-word alias");
        assert_eq!(status, 0);
        assert_eq!(shell.get_var("BRANCH").as_deref(), Some("hit"));
    }

    #[test]
    fn print_jobs_emits_finished_branch_when_job_is_done() {
        let mut shell = test_shell();
        let child = ProcessCommand::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn");
        shell.launch_background_job("done".into(), vec![child]);
        std::thread::sleep(std::time::Duration::from_millis(20));
        shell.print_jobs();
        assert!(shell.jobs.is_empty());
    }
}
