#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use libc;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn meiksh() -> &'static str {
    env!("CARGO_BIN_EXE_meiksh")
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_meiksh_with_stdin(script: &str, stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(meiksh())
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn meiksh");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("wait for meiksh")
}

fn run_meiksh_with_nonblocking_stdin(stdin: &[u8]) -> std::process::Output {
    let mut command = Command::new(meiksh());
    unsafe {
        command.pre_exec(|| {
            let flags = libc::fcntl(0, libc::F_GETFL, 0);
            if flags < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn meiksh with nonblocking stdin");
    let mut child_stdin = child.stdin.take().expect("stdin");
    std::thread::sleep(std::time::Duration::from_millis(100));
    child_stdin.write_all(stdin).expect("write delayed stdin");
    drop(child_stdin);
    child.wait_with_output().expect("wait for meiksh")
}

#[test]
fn syntax_check_accepts_valid_script() {
    let output = Command::new(meiksh())
        .arg("-n")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(b"echo ok\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh");
    assert!(output.status.success());
}

#[test]
fn syntax_check_rejects_reserved_word_misuse() {
    let function_name = Command::new(meiksh())
        .args(["-n", "-c", "if() { printf bad; }"])
        .output()
        .expect("run meiksh");
    assert!(!function_name.status.success());

    let bang_after_pipe = Command::new(meiksh())
        .args(["-n", "-c", "echo hi | ! cat"])
        .output()
        .expect("run meiksh");
    assert!(!bang_after_pipe.status.success());
}

#[test]
fn executes_simple_command_string() {
    let output = Command::new(meiksh())
        .args(["-c", "printf hi"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi");
}

#[test]
fn executes_pipeline() {
    let output = Command::new(meiksh())
        .args(["-c", "printf hi | wc -c"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2");
}

#[test]
fn export_visible_to_child() {
    let output = Command::new(meiksh())
        .args(["-c", "export FOO=bar; printenv FOO"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "bar");
}

#[test]
fn command_builtin_reports_and_executes_posix_like_lookups() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "printf() { echo bad; }; alias ll='printf alias'; command printf ok; command printf '\\n'; command -v export; command -V export; command -v ll; command -V if",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines[0], "ok");
    assert_eq!(lines[1], "export");
    assert!(lines[2].contains("export is a special built-in utility"));
    assert_eq!(lines[3], "ll='printf alias'");
    assert!(lines[4].contains("if is a reserved word"));
    assert!(!stdout.contains("bad"));
}

#[test]
fn export_readonly_unset_and_pwd_support_listing_and_options() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "export VALUE='a b' ONLY; readonly LOCK='x y' FLAG; f() { :; }; export -p; readonly -p; unset -f f; unset -v VALUE; command -V f; printf 'status=%s\\n' \"$?\"; pwd -L; pwd -P",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().collect();
    assert!(lines.contains(&"export ONLY"));
    assert!(lines.contains(&"export VALUE='a b'"));
    assert!(lines.contains(&"readonly FLAG"));
    assert!(lines.contains(&"readonly LOCK='x y'"));
    assert!(lines.contains(&"status=1"));
    assert_eq!(lines[lines.len() - 2], lines[lines.len() - 1]);
}

#[test]
fn read_builtin_assigns_variables_in_current_shell() {
    let output = run_meiksh_with_stdin(
        "read first second; STATUS=$?; printf %s \"$STATUS|$first|$second\"",
        b"alpha beta gamma\n",
    );
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "0|alpha|beta gamma"
    );

    let eof = run_meiksh_with_stdin(
        "read only; STATUS=$?; printf %s \"$STATUS|$only\"",
        b"tail-without-newline",
    );
    assert!(eof.status.success());
    assert_eq!(
        String::from_utf8_lossy(&eof.stdout),
        "1|tail-without-newline"
    );

    let raw = run_meiksh_with_stdin("read -r value; printf %s \"$value\"", b"one\\\\two\n");
    assert!(raw.status.success());
    assert_eq!(String::from_utf8_lossy(&raw.stdout), "one\\\\two");
}

#[test]
fn umask_and_times_builtins_follow_current_shell_state() {
    let root = TempDir::new("meiksh-umask");
    let path = root.join("test.txt");
    let script = format!("umask 077; : > {}; umask; umask -S", path.display());
    let umask_output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(umask_output.status.success());
    let umask_stdout = String::from_utf8_lossy(&umask_output.stdout);
    let lines: Vec<_> = umask_stdout.lines().collect();
    assert_eq!(lines, vec!["0077", "u=rwx,g=,o="]);
    assert_eq!(
        fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
        0o600
    );

    let times_output = Command::new(meiksh())
        .args(["-c", "times"])
        .output()
        .expect("run meiksh");
    assert!(times_output.status.success());
    let times_stdout = String::from_utf8_lossy(&times_output.stdout);
    let time_lines: Vec<_> = times_stdout.lines().collect();
    assert_eq!(time_lines.len(), 2);
    for line in time_lines {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 2);
        assert!(
            fields
                .iter()
                .all(|field| field.contains('m') && field.ends_with('s'))
        );
    }
}

#[test]
fn handles_redirections() {
    let root = TempDir::new("meiksh-redir");
    let path = root.join("out.txt");
    let script = format!("printf hi > {}", path.display());

    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&path).expect("read redirect target"),
        "hi"
    );
}

#[test]
fn redirects_current_shell_builtins_and_compound_commands() {
    let root = TempDir::new("meiksh-builtin-redir");
    let builtin_path = root.join("builtin.txt");
    let group_path = root.join("group.txt");

    let builtin = Command::new(meiksh())
        .args([
            "-c",
            &format!("pwd > {}; printf ok", builtin_path.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(builtin.status.success());
    assert_eq!(String::from_utf8_lossy(&builtin.stdout), "ok");
    assert!(
        !fs::read_to_string(&builtin_path)
            .expect("read builtin output")
            .trim()
            .is_empty()
    );

    let group = Command::new(meiksh())
        .args([
            "-c",
            &format!(
                "{{ printf inside; }} > {}; printf outside",
                group_path.display()
            ),
        ])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "outside");
    assert_eq!(
        fs::read_to_string(&group_path).expect("read group output"),
        "inside"
    );

    let pipeline = Command::new(meiksh())
        .args([
            "-c",
            &format!("{{ printf inside; }} > {} | wc -c", group_path.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(pipeline.status.success());
    assert_eq!(String::from_utf8_lossy(&pipeline.stdout).trim(), "0");
    assert_eq!(
        fs::read_to_string(&group_path).expect("read group output again"),
        "inside"
    );
}

#[test]
fn handles_append_and_input_redirections() {
    let root = TempDir::new("meiksh-append-input");
    let input = root.join("input.txt");
    let output = root.join("output.txt");
    fs::write(&input, "abc").expect("write input");

    let script = format!(
        "cat < {} > {}; printf def >> {}",
        input.display(),
        output.display(),
        output.display()
    );
    let status = Command::new(meiksh())
        .args(["-c", &script])
        .status()
        .expect("run meiksh");
    assert!(status.success());
    assert_eq!(fs::read_to_string(&output).expect("read output"), "abcdef");
}

#[test]
fn handles_background_wait() {
    let output = Command::new(meiksh())
        .args(["-c", "sleep 0.1 & wait"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}

#[test]
fn bg_sends_sigcont_to_background_job() {
    let output = Command::new(meiksh())
        .args(["-c", "sleep 0.01 & bg %1 2>/dev/null; wait"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}

#[test]
fn trap_wait_and_job_control_paths_cover_milestone_five() {
    let exit_trap = Command::new(meiksh())
        .args(["-c", "trap 'printf exit:$?' EXIT; false"])
        .output()
        .expect("run meiksh");
    assert_eq!(exit_trap.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&exit_trap.stdout), "exit:1");

    let signal_trap = Command::new(meiksh())
        .args(["-c", "trap 'printf INT:$?' INT; kill -INT $$; printf done"])
        .output()
        .expect("run meiksh");
    assert!(signal_trap.status.success());
    assert_eq!(String::from_utf8_lossy(&signal_trap.stdout), "INT:0done");

    let wait_pid = Command::new(meiksh())
        .args(["-c", "sleep 0.05 & pid=$!; wait \"$pid\"; printf :$?"])
        .output()
        .expect("run meiksh");
    assert!(wait_pid.status.success());
    assert!(String::from_utf8_lossy(&wait_pid.stdout).ends_with(":0"));

    let wait_unknown = Command::new(meiksh())
        .args(["-c", "wait 999999; printf %s $?"])
        .output()
        .expect("run meiksh");
    assert!(wait_unknown.status.success());
    assert_eq!(String::from_utf8_lossy(&wait_unknown.stdout), "127");

    let jobs_output = Command::new(meiksh())
        .args(["-c", "sleep 0.1 & jobs"])
        .output()
        .expect("run meiksh");
    assert!(jobs_output.status.success());
    let stdout = String::from_utf8_lossy(&jobs_output.stdout);
    assert!(stdout.contains("[1]"));
    assert!(stdout.contains("Running sleep 0.1"));
}

#[test]
fn unalias_and_dot_follow_milestone_six_paths() {
    let root = TempDir::new("meiksh-m6-spec");
    let path_dir = root.join("path");
    fs::create_dir_all(&path_dir).expect("mkdir path");

    let dot_script = path_dir.join("dot-script.sh");
    fs::write(&dot_script, "M6_SPEC_DOT=loaded\n").expect("write dot script");
    fs::set_permissions(&dot_script, fs::Permissions::from_mode(0o644)).expect("chmod dot script");

    let script = format!(
        "alias ll='printf no'; unalias -a; command -v ll >/dev/null 2>&1; printf 'unalias:%s\\n' $?; ORIGPATH=$PATH; PATH='{}'; . dot-script.sh; PATH=$ORIGPATH; printf 'dot:%s\\n' \"$M6_SPEC_DOT\"",
        path_dir.display(),
    );
    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().filter(|line| !line.is_empty()).collect();
    assert!(lines.contains(&"unalias:1"));
    assert!(lines.contains(&"dot:loaded"));
}

#[test]
fn cd_dash_and_jobs_p_follow_milestone_six_paths() {
    let root = TempDir::new("meiksh-m6-spec");
    let target = root.join("target");
    fs::create_dir_all(&target).expect("mkdir target");

    let original = std::env::current_dir().expect("cwd");
    let script = format!(
        "cd '{}'; cd - >/dev/null; printf 'pwd:%s\\nold:%s\\n' \"$PWD\" \"$OLDPWD\"; sleep 0.1 & jobs -p",
        target.display(),
    );
    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = stdout.lines().filter(|line| !line.is_empty()).collect();
    assert!(lines.contains(&format!("pwd:{}", original.display()).as_str()));
    assert!(lines.contains(&format!("old:{}", target.display()).as_str()));
    assert!(
        lines
            .last()
            .is_some_and(|line| line.chars().all(|ch| ch.is_ascii_digit()))
    );
}

#[test]
fn cd_uses_cdpath_and_reports_resolved_directory() {
    let root = TempDir::new("meiksh-cdpath-spec");
    let cdpath = root.join("cdpath");
    let target = cdpath.join("target");
    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(&target).expect("mkdir target");
    fs::create_dir_all(&elsewhere).expect("mkdir elsewhere");

    let output = Command::new(meiksh())
        .current_dir(&elsewhere)
        .args([
            "-c",
            "CDPATH='../cdpath'; cd target; printf '|pwd:%s|old:%s' \"$PWD\" \"$OLDPWD\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let canonical_target = target.canonicalize().expect("canonical target");
    let canonical_elsewhere = elsewhere.canonicalize().expect("canonical elsewhere");
    assert!(stdout.starts_with(&format!("{}\n", canonical_target.display())));
    assert!(stdout.contains(&format!("|pwd:{}|", canonical_target.display())));
    assert!(stdout.contains(&format!("|old:{}", canonical_elsewhere.display())));
}

#[test]
fn sh_s_option_sets_positionals_from_operands() {
    let output = Command::new(meiksh())
        .args(["-s", "alpha", "beta"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printf '%s|%s' \"$1\" \"$2\"\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh -s");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "alpha|beta");
}

#[test]
fn sh_c_command_name_sets_special_parameter_zero() {
    let output = Command::new(meiksh())
        .args(["-c", "printf %s \"$0\"", "cmd-name", "ignored-positional"])
        .output()
        .expect("run meiksh -c");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "cmd-name");
}

#[test]
fn sh_lone_dash_is_ignored_and_reads_stdin() {
    let output = Command::new(meiksh())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(b"printf ok\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh -");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
}

#[test]
fn sh_stdin_does_not_read_ahead_past_the_current_command() {
    let output = Command::new(meiksh())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"cat\necho after\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "echo after\n");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn sh_forces_blocking_reads_on_nonblocking_standard_input() {
    let output = run_meiksh_with_nonblocking_stdin(b"printf blocking\n");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "blocking");
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn sh_startup_option_subset_supports_allexport_nounset_verbose_named_o_and_dollar_dash() {
    let export_output = Command::new(meiksh())
        .args(["-a", "-c", "AUTO=works; printenv AUTO"])
        .output()
        .expect("run meiksh -a");
    assert!(export_output.status.success());
    assert_eq!(String::from_utf8_lossy(&export_output.stdout), "works\n");

    let dash_output = Command::new(meiksh())
        .args(["-a", "-C", "-u", "-v", "-c", "printf '%s' \"$-\""])
        .output()
        .expect("run meiksh dollar dash");
    assert!(dash_output.status.success());
    assert_eq!(String::from_utf8_lossy(&dash_output.stdout), "aCuvc");
    assert_eq!(
        String::from_utf8_lossy(&dash_output.stderr),
        "printf '%s' \"$-\""
    );

    let named_output = Command::new(meiksh())
        .args([
            "-o",
            "noglob",
            "-o",
            "nounset",
            "-o",
            "verbose",
            "-c",
            "printf '%s|%s' *.definitely_missing \"$-\"",
        ])
        .output()
        .expect("run meiksh -o noglob -o nounset -o verbose");
    assert!(named_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&named_output.stdout),
        "*.definitely_missing|fuvc"
    );
    assert_eq!(
        String::from_utf8_lossy(&named_output.stderr),
        "printf '%s|%s' *.definitely_missing \"$-\""
    );
}

#[test]
fn sh_nounset_fails_plain_unset_expansions_but_allows_defaulting_forms() {
    let unset_output = Command::new(meiksh())
        .args(["-u", "-c", "printf '%s' \"$MISSING\"; printf bad"])
        .output()
        .expect("run meiksh -u");
    assert_eq!(unset_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unset_output.stdout).is_empty());
    assert!(String::from_utf8_lossy(&unset_output.stderr).contains("MISSING: parameter not set"));

    let default_output = Command::new(meiksh())
        .args(["-u", "-c", "printf '%s' \"${MISSING-default}\""])
        .output()
        .expect("run meiksh -u default");
    assert!(default_output.status.success());
    assert_eq!(String::from_utf8_lossy(&default_output.stdout), "default");

    let set_builtin_output = Command::new(meiksh())
        .args([
            "-c",
            "set -u; set -v; printf '%s|%s' \"$-\" \"${MISSING-fallback}\"",
        ])
        .output()
        .expect("run set -u");
    assert!(set_builtin_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&set_builtin_output.stdout),
        "uvc|fallback"
    );
    assert!(String::from_utf8_lossy(&set_builtin_output.stderr).is_empty());
}

#[test]
fn sh_command_file_sets_special_parameter_zero_and_searches_path() {
    let root = TempDir::new("meiksh-sh-path");
    let dir = root.join("path");
    let elsewhere = root.join("cwd");
    fs::create_dir_all(&dir).expect("mkdir path dir");
    fs::create_dir_all(&elsewhere).expect("mkdir cwd dir");

    let script = dir.join("path-script");
    fs::write(&script, "printf %s \"$0\"").expect("write script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod script");
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = Command::new(meiksh())
        .current_dir(&elsewhere)
        .env("PATH", path)
        .arg("path-script")
        .output()
        .expect("run meiksh command_file");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "path-script");
}

#[test]
fn sh_command_file_missing_and_read_errors_have_distinct_exit_statuses() {
    let missing = Command::new(meiksh())
        .arg("/definitely/missing-meiksh-script")
        .output()
        .expect("run missing script");
    assert_eq!(missing.status.code(), Some(127));

    let root = TempDir::new("meiksh-sh-readerr");
    let read_error = Command::new(meiksh())
        .arg(root.path().display().to_string())
        .output()
        .expect("run directory script path");
    assert_eq!(read_error.status.code(), Some(128));
}

#[test]
fn sh_invalid_invocation_uses_usage_exit_status() {
    let invalid_option = Command::new(meiksh())
        .arg("-z")
        .output()
        .expect("run invalid option");
    assert_eq!(invalid_option.status.code(), Some(2));

    let missing_c_argument = Command::new(meiksh())
        .arg("-c")
        .output()
        .expect("run missing -c arg");
    assert_eq!(missing_c_argument.status.code(), Some(2));
}

#[test]
fn interactive_shell_expands_env_and_continues_after_error() {
    let home = TempDir::new("meiksh-m6-home");
    let env_file = home.join("env.sh");
    fs::write(&env_file, "export TEST_ENV_LOADED=1\n").expect("write env file");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .env("ENV", "${HOME}/env.sh")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"false\nprintenv TEST_ENV_LOADED\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1"), "ENV file should set TEST_ENV_LOADED=1, got: {stdout}");
}

#[test]
fn interactive_shell_uses_home_history_default() {
    let home = TempDir::new("meiksh-m6-home");
    let history = home.join(".sh_history");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printf ok\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let history_contents = fs::read_to_string(&history).expect("history contents");
    assert!(history_contents.contains("printf ok"));
}

#[test]
fn executes_shell_function() {
    let output = Command::new(meiksh())
        .args(["-c", "greet() { printf hello; }; greet"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello");
}

#[test]
fn control_flow_builtins_obey_function_and_loop_semantics() {
    let function = Command::new(meiksh())
        .args(["-c", "f() { printf hi; return 7; printf no; }; f"])
        .output()
        .expect("run meiksh");
    assert_eq!(function.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&function.stdout), "hi");

    let break_output = Command::new(meiksh())
        .args([
            "-c",
            "for item in a b; do printf $item; break; printf no; done",
        ])
        .output()
        .expect("run meiksh");
    assert!(break_output.status.success());
    assert_eq!(String::from_utf8_lossy(&break_output.stdout), "a");

    let continue_output = Command::new(meiksh())
        .args([
            "-c",
            "for item in a b; do continue; printf no; done; printf ok",
        ])
        .output()
        .expect("run meiksh");
    assert!(continue_output.status.success());
    assert_eq!(String::from_utf8_lossy(&continue_output.stdout), "ok");
}

#[test]
fn invalid_control_flow_builtins_fail_non_interactive_shells() {
    let break_output = Command::new(meiksh())
        .args(["-c", "break; printf no"])
        .output()
        .expect("run meiksh");
    assert!(!break_output.status.success());
    assert!(
        String::from_utf8_lossy(&break_output.stderr).contains("break: only meaningful in a loop")
    );

    let return_output = Command::new(meiksh())
        .args(["-c", "return"])
        .output()
        .expect("run meiksh");
    assert!(!return_output.status.success());
    assert!(String::from_utf8_lossy(&return_output.stderr).contains("return: not in a function"));
}

#[test]
fn ordinary_builtin_errors_do_not_exit_non_interactive_shells() {
    let fg_output = Command::new(meiksh())
        .args(["-c", "fg; printf after"])
        .output()
        .expect("run meiksh");
    assert!(fg_output.status.success());
    assert_eq!(String::from_utf8_lossy(&fg_output.stdout), "after");
    assert!(String::from_utf8_lossy(&fg_output.stderr).contains("fg: no job control"));

    let pwd_output = Command::new(meiksh())
        .args(["-c", "pwd </definitely/missing-input; printf after"])
        .output()
        .expect("run meiksh");
    assert!(pwd_output.status.success());
    assert_eq!(String::from_utf8_lossy(&pwd_output.stdout), "after");
    assert!(String::from_utf8_lossy(&pwd_output.stderr).contains("No such file"));
}

#[test]
fn special_builtin_redirection_errors_still_exit_non_interactive_shells() {
    let output = Command::new(meiksh())
        .args(["-c", "export </definitely/missing-input; printf after"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No such file"));
}

#[test]
fn exec_builtin_replaces_process_in_subshell() {
    let output = Command::new(meiksh())
        .args(["-c", "exec /bin/echo hello"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
}

#[test]
fn executes_subshell_and_group_commands() {
    let subshell = Command::new(meiksh())
        .args(["-c", "(exit 7)"])
        .status()
        .expect("run meiksh");
    assert_eq!(subshell.code(), Some(7));

    let group = Command::new(meiksh())
        .args(["-c", "{ VALUE=42; }; printf $VALUE"])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "42");

    let literal_closer = Command::new(meiksh())
        .args(["-c", "echo }"])
        .output()
        .expect("run meiksh");
    assert!(literal_closer.status.success());
    assert_eq!(String::from_utf8_lossy(&literal_closer.stdout), "}\n");

    let not_a_group = Command::new(meiksh())
        .args(["-c", "{printf bad; }"])
        .output()
        .expect("run meiksh");
    assert!(!not_a_group.status.success());
}

#[test]
fn negated_pipeline_flips_status() {
    let status = Command::new(meiksh())
        .args(["-c", "! true"])
        .status()
        .expect("run meiksh");
    assert_eq!(status.code(), Some(1));

    let literal = Command::new(meiksh())
        .args(["-c", "echo !"])
        .output()
        .expect("run meiksh");
    assert!(literal.status.success());
    assert_eq!(String::from_utf8_lossy(&literal.stdout), "!\n");

    let not_reserved = Command::new(meiksh())
        .args(["-c", "!true"])
        .output()
        .expect("run meiksh");
    assert!(!not_reserved.status.success());

    let linebreak_pipeline = Command::new(meiksh())
        .args(["-c", "printf hi |\n wc -c"])
        .output()
        .expect("run meiksh");
    assert!(linebreak_pipeline.status.success());
    assert_eq!(
        String::from_utf8_lossy(&linebreak_pipeline.stdout).trim(),
        "2"
    );

    let linebreak_and_or = Command::new(meiksh())
        .args(["-c", "false ||\n printf pass; true &&\n printf done"])
        .output()
        .expect("run meiksh");
    assert!(linebreak_and_or.status.success());
    assert_eq!(
        String::from_utf8_lossy(&linebreak_and_or.stdout),
        "passdone"
    );
}

#[test]
fn aliases_defined_earlier_in_same_source_affect_later_commands() {
    let simple = Command::new(meiksh())
        .args(["-c", "alias say='printf ok'; say"])
        .output()
        .expect("run meiksh");
    assert!(simple.status.success());
    assert_eq!(String::from_utf8_lossy(&simple.stdout), "ok");

    let reserved = Command::new(meiksh())
        .args(["-c", "alias cond='if'; cond true; then printf yes; fi"])
        .output()
        .expect("run meiksh");
    assert!(reserved.status.success());
    assert_eq!(String::from_utf8_lossy(&reserved.stdout), "yes");

    // Aliases defined inside compound commands are not visible to later
    // commands in the same compound command body, because the body was
    // already parsed before any of it executes (POSIX: aliases are
    // resolved at parse time, not execution time).  However, aliases
    // defined before a compound command ARE visible within it.
    let group = Command::new(meiksh())
        .args(["-c", "alias say='printf group'; { say; }"])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "group");

    let function = Command::new(meiksh())
        .args(["-c", "alias say='printf fn'; f() { say; }; f"])
        .output()
        .expect("run meiksh");
    assert!(function.status.success());
    assert_eq!(String::from_utf8_lossy(&function.stdout), "fn");

    let conditional = Command::new(meiksh())
        .args(["-c", "alias say='printf branch'; if true; then say; fi"])
        .output()
        .expect("run meiksh");
    assert!(conditional.status.success());
    assert_eq!(String::from_utf8_lossy(&conditional.stdout), "branch");

    let heredoc_nested = Command::new(meiksh())
        .args(["-c", "alias say='cat'; f() { say <<EOF\nhello\nEOF\n}; f"])
        .output()
        .expect("run meiksh");
    assert!(heredoc_nested.status.success());
    assert_eq!(String::from_utf8_lossy(&heredoc_nested.stdout), "hello\n");
}

#[test]
fn executes_here_documents() {
    let output = Command::new(meiksh())
        .args(["-c", "VALUE=world; cat <<EOF\nhello $VALUE\nEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello world\n");

    let quoted = Command::new(meiksh())
        .args(["-c", "VALUE=world; cat <<'EOF'\nhello $VALUE\nEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(quoted.status.success());
    assert_eq!(String::from_utf8_lossy(&quoted.stdout), "hello $VALUE\n");

    let stripped = Command::new(meiksh())
        .args(["-c", "cat <<-\tEOF\n\tstrip-me\n\tEOF\n"])
        .output()
        .expect("run meiksh");
    assert!(stripped.status.success());
    assert_eq!(String::from_utf8_lossy(&stripped.stdout), "strip-me\n");
}

#[test]
fn expands_parameters_and_pathnames_more_like_posix() {
    let positional = Command::new(meiksh())
        .args([
            "-c",
            "set -- a b c d e f g h i j; printf '%s|%s|%s' \"$10\" \"${10}\" \"${#10}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(positional.status.success());
    assert_eq!(String::from_utf8_lossy(&positional.stdout), "a0|j|1");

    let operators = Command::new(meiksh())
        .args([
            "-c",
            "unset X; printf '<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"; X=''; printf '|<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(operators.status.success());
    assert_eq!(
        String::from_utf8_lossy(&operators.stdout),
        "<word><word><><>|<><word><alt><>"
    );

    let trimming = Command::new(meiksh())
        .args([
            "-c",
            "PATHNAME='src/bin/main.rs'; DOTTED='alpha.beta.gamma'; printf '%s|%s|%s|%s|%s|%s|%s|%s|%s|%s' \"${PATHNAME#*/}\" \"${PATHNAME##*/}\" \"${PATHNAME%/*}\" \"${PATHNAME%%/*}\" \"${DOTTED#*.}\" \"${DOTTED##*.}\" \"${DOTTED%.*}\" \"${DOTTED%%.*}\" \"${DOTTED#\"*.\"}\" \"${PATHNAME#\"src/\"}\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(trimming.status.success());
    assert_eq!(
        String::from_utf8_lossy(&trimming.stdout),
        "bin/main.rs|main.rs|src/bin|src|beta.gamma|gamma|alpha.beta|alpha|alpha.beta.gamma|bin/main.rs"
    );

    let dir = TempDir::new("meiksh-expand-spec");
    fs::write(dir.join("a.txt"), "").expect("write a");
    fs::write(dir.join("b.txt"), "").expect("write b");
    fs::write(dir.join(".hidden.txt"), "").expect("write hidden");

    let glob = Command::new(meiksh())
        .current_dir(dir.path())
        .args(["-c", "printf '%s|' *.txt \\*.txt .*\\.txt"])
        .output()
        .expect("run meiksh");
    assert!(glob.status.success());
    assert_eq!(
        String::from_utf8_lossy(&glob.stdout),
        "a.txt|b.txt|*.txt|.hidden.txt|"
    );

    let noglob = Command::new(meiksh())
        .current_dir(dir.path())
        .args([
            "-c",
            "set -f; printf '%s|' *.txt; set +f; printf '%s|' *.txt",
        ])
        .output()
        .expect("run meiksh");
    assert!(noglob.status.success());
    assert_eq!(
        String::from_utf8_lossy(&noglob.stdout),
        "*.txt|a.txt|b.txt|"
    );

    let shell_option = Command::new(meiksh())
        .current_dir(dir.path())
        .args(["-f", "-c", "printf '%s|' *.txt"])
        .output()
        .expect("run meiksh");
    assert!(shell_option.status.success());
    assert_eq!(String::from_utf8_lossy(&shell_option.stdout), "*.txt|");
}

#[test]
fn dollar_single_quotes_follow_issue_eight_rules() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "printf '%s|%s|%s' $'a b' $'line\\nnext' \"$'literal'\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a b|line\nnext|$'literal'"
    );
}

#[test]
fn field_splitting_respects_ifs_defaults_and_star_joining() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "VALUE='a b'; unset IFS; printf '<%s>' $VALUE; IFS=; printf '|<%s>' $VALUE; set -- a b c; IFS=:; printf '|<%s><%s>' $* \"$*\"",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<a><b>|<a b>|<a><b>|<c><a:b:c>"
    );
}

#[test]
fn falls_back_on_enoexec_scripts() {
    let dir = TempDir::new("meiksh-enoexec");

    let slash_script = dir.join("slash-script");
    fs::write(&slash_script, "printf slash:$1").expect("write slash script");
    let mut permissions = fs::metadata(&slash_script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&slash_script, permissions).expect("chmod slash script");

    let slash_output = Command::new(meiksh())
        .args(["-c", &format!("{} arg", slash_script.display())])
        .output()
        .expect("run meiksh");
    assert!(slash_output.status.success());
    assert_eq!(String::from_utf8_lossy(&slash_output.stdout), "slash:arg");

    let path_script = dir.join("path-script");
    fs::write(&path_script, "cat").expect("write path script");
    let mut permissions = fs::metadata(&path_script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path_script, permissions).expect("chmod path script");

    let path_value = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let path_output = Command::new(meiksh())
        .env("PATH", path_value)
        .args(["-c", "printf piped | path-script"])
        .output()
        .expect("run meiksh");
    assert!(path_output.status.success());
    assert_eq!(String::from_utf8_lossy(&path_output.stdout), "piped");
}

#[test]
fn handles_extended_redirection_matrix() {
    let dir = TempDir::new("meiksh-redir-matrix");

    let input = dir.join("input.txt");
    let output = dir.join("output.txt");
    let append = dir.join("append.txt");
    let rw = dir.join("rw.txt");
    fs::write(&input, "from-input").expect("write input");
    fs::write(&rw, "from-rw").expect("write rw");

    let read_script = format!("cat 3<{} <&3", input.display());
    let read_output = Command::new(meiksh())
        .args(["-c", &read_script])
        .output()
        .expect("run meiksh");
    assert!(read_output.status.success());
    assert_eq!(String::from_utf8_lossy(&read_output.stdout), "from-input");

    let write_script = format!("printf file 3>{} >&3", output.display());
    let write_status = Command::new(meiksh())
        .args(["-c", &write_script])
        .status()
        .expect("run meiksh");
    assert!(write_status.success());
    assert_eq!(fs::read_to_string(&output).expect("read output"), "file");

    let append_script = format!("printf err 2>>{} >&2", append.display());
    let append_status = Command::new(meiksh())
        .args(["-c", &append_script])
        .status()
        .expect("run meiksh");
    assert!(append_status.success());
    assert_eq!(fs::read_to_string(&append).expect("read append"), "err");

    let read_write_script = format!("cat <>{}", rw.display());
    let rw_output = Command::new(meiksh())
        .args(["-c", &read_write_script])
        .output()
        .expect("run meiksh");
    assert!(rw_output.status.success());
    assert_eq!(String::from_utf8_lossy(&rw_output.stdout), "from-rw");

    let precedence_script = format!("printf hidden 1>{} | wc -c", output.display());
    let precedence_output = Command::new(meiksh())
        .args(["-c", &precedence_script])
        .output()
        .expect("run meiksh");
    assert!(precedence_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&precedence_output.stdout).trim(),
        "0"
    );
    assert_eq!(
        fs::read_to_string(&output).expect("read redirected output"),
        "hidden"
    );
}

#[test]
fn honors_noclobber_and_force_clobber() {
    let root = TempDir::new("meiksh-noclobber");
    let path = root.join("test.txt");
    fs::write(&path, "old").expect("write initial");

    let blocked = Command::new(meiksh())
        .args(["-c", &format!("set -C; printf new > {}", path.display())])
        .output()
        .expect("run meiksh");
    assert!(!blocked.status.success());
    assert_eq!(fs::read_to_string(&path).expect("read blocked"), "old");

    let forced = Command::new(meiksh())
        .args(["-c", &format!("set -C; printf new >| {}", path.display())])
        .output()
        .expect("run meiksh");
    assert!(forced.status.success());
    assert_eq!(fs::read_to_string(&path).expect("read forced"), "new");
}

#[test]
fn background_and_or_list_runs_asynchronously() {
    let output = Command::new(meiksh())
        .args(["-c", "true && true &"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}

#[test]
fn interactive_shell_sources_env_file() {
    let root = TempDir::new("meiksh-env");
    let path = root.join("env.sh");
    let history = root.join("history.txt");
    fs::write(&path, "export TEST_ENV_LOADED=1\n").expect("write env file");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("ENV", &path)
        .env("HISTFILE", &history)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printenv TEST_ENV_LOADED\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh");

    assert!(output.status.success());
}

#[test]
fn executes_if_elif_else() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "if false; then printf no; elif true; then printf yes; else printf bad; fi",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "yes");
}

#[test]
fn executes_while_and_until_loops() {
    let root = TempDir::new("meiksh-loop");
    let marker = root.join("marker.flag");
    fs::write(&marker, "present").expect("seed marker");

    let while_script = format!(
        "while test -f {}; do rm {}; VALUE=done; done; printf $VALUE",
        marker.display(),
        marker.display()
    );
    let while_output = Command::new(meiksh())
        .args(["-c", &while_script])
        .output()
        .expect("run meiksh");
    assert!(while_output.status.success());
    assert_eq!(String::from_utf8_lossy(&while_output.stdout), "done");

    let _ = fs::remove_file(&marker);
    let until_script = format!(
        "until test -f {}; do touch {}; VALUE=ready; done; printf $VALUE",
        marker.display(),
        marker.display()
    );
    let until_output = Command::new(meiksh())
        .args(["-c", &until_script])
        .output()
        .expect("run meiksh");
    assert!(until_output.status.success());
    assert_eq!(String::from_utf8_lossy(&until_output.stdout), "ready");
}

#[test]
fn executes_for_loops() {
    let explicit = Command::new(meiksh())
        .args(["-c", "for item in a b c; do LAST=$item; done; printf $LAST"])
        .output()
        .expect("run meiksh");
    assert!(explicit.status.success());
    assert_eq!(String::from_utf8_lossy(&explicit.stdout), "c");

    let positional = Command::new(meiksh())
        .args([
            "-c",
            "for item; do LAST=$item; done; printf $LAST",
            "meiksh",
            "x",
            "y",
        ])
        .output()
        .expect("run meiksh");
    assert!(positional.status.success());
    assert_eq!(String::from_utf8_lossy(&positional.stdout), "y");

    let linebreak_before_in = Command::new(meiksh())
        .args([
            "-c",
            "for item\nin alpha beta; do printf '%s|' \"$item\"; done",
        ])
        .output()
        .expect("run meiksh");
    assert!(linebreak_before_in.status.success());
    assert_eq!(
        String::from_utf8_lossy(&linebreak_before_in.stdout),
        "alpha|beta|"
    );

    let reserved_words_in_wordlist = Command::new(meiksh())
        .args(["-c", "for item in do done; do printf '%s|' \"$item\"; done"])
        .output()
        .expect("run meiksh");
    assert!(reserved_words_in_wordlist.status.success());
    assert_eq!(
        String::from_utf8_lossy(&reserved_words_in_wordlist.stdout),
        "do|done|"
    );
}

#[test]
fn executes_case_commands() {
    let exact = Command::new(meiksh())
        .args([
            "-c",
            "name=beta; case $name in alpha) printf no ;; beta|gamma) printf yes ;; esac",
        ])
        .output()
        .expect("run meiksh");
    assert!(exact.status.success());
    assert_eq!(String::from_utf8_lossy(&exact.stdout), "yes");

    let wildcard = Command::new(meiksh())
        .args([
            "-c",
            "name=report.txt; case $name in *.log) printf no ;; *.txt) printf yes ;; esac",
        ])
        .output()
        .expect("run meiksh");
    assert!(wildcard.status.success());
    assert_eq!(String::from_utf8_lossy(&wildcard.stdout), "yes");

    let star = Command::new(meiksh())
        .args(["-c", "name=beta; case $name in *) printf yes ;; esac"])
        .output()
        .expect("run meiksh");
    assert!(star.status.success());
    assert_eq!(String::from_utf8_lossy(&star.stdout), "yes");

    let linebreak_before_in = Command::new(meiksh())
        .args([
            "-c",
            "name=beta; case $name\nin\nalpha) printf no ;;\nbeta) printf yes ;;\nesac",
        ])
        .output()
        .expect("run meiksh");
    assert!(linebreak_before_in.status.success());
    assert_eq!(String::from_utf8_lossy(&linebreak_before_in.stdout), "yes");

    let empty_case = Command::new(meiksh())
        .args(["-c", "case value\nin\nesac; printf ok"])
        .output()
        .expect("run meiksh");
    assert!(empty_case.status.success());
    assert_eq!(String::from_utf8_lossy(&empty_case.stdout), "ok");
}

#[test]
fn pipeline_with_pty_exercises_terminal_foreground_control() {
    use std::os::unix::io::FromRawFd;
    let mut primary: i32 = -1;
    let mut secondary: i32 = -1;
    let ret = unsafe {
        libc::openpty(
            &mut primary,
            &mut secondary,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ret != 0 {
        return;
    }

    let secondary_fd = secondary;
    let output = unsafe {
        let mut cmd = Command::new(meiksh());
        cmd.args(["-c", "printf a | cat; printf ok"])
            .stdin(Stdio::from_raw_fd(secondary_fd))
            .stdout(Stdio::piped());
        cmd.pre_exec(move || {
            libc::setsid();
            libc::ioctl(secondary_fd, libc::TIOCSCTTY as _, 0);
            libc::dup2(secondary_fd, 2);
            Ok(())
        });
        cmd.output().expect("run meiksh")
    };

    unsafe {
        libc::close(primary);
        libc::close(secondary);
    }
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ok"),
        "expected 'ok' in stdout, got: {stdout}"
    );
}

#[test]
fn errexit_exits_on_failed_command() {
    let output = Command::new(meiksh())
        .args(["-ec", "false; echo unreachable"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("unreachable"),
        "should not reach echo after false with -e"
    );
}

#[test]
fn errexit_suppressed_in_if_condition() {
    let output = Command::new(meiksh())
        .args(["-ec", "if false; then echo then; fi; echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn errexit_suppressed_in_while_condition() {
    let output = Command::new(meiksh())
        .args(["-ec", "while false; do echo body; done; echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn errexit_suppressed_in_non_final_and_or() {
    let output = Command::new(meiksh())
        .args(["-ec", "false || echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn errexit_fires_on_final_and_or_command() {
    let output = Command::new(meiksh())
        .args(["-ec", "true && false; echo unreachable"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("unreachable"),
        "should not reach echo after && false with -e"
    );
}

#[test]
fn errexit_suppressed_in_negated_pipeline() {
    let output = Command::new(meiksh())
        .args(["-ec", "! true; echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn xtrace_outputs_trace_to_stderr() {
    let output = Command::new(meiksh())
        .args(["-xc", "echo hello"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("+ echo hello"),
        "expected xtrace output, got stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "hello");
}

#[test]
fn xtrace_uses_custom_ps4() {
    let output = Command::new(meiksh())
        .args(["-xc", "PS4='>> '; echo hello"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(">> echo hello"),
        "expected custom PS4 prefix, got stderr: {stderr}"
    );
}

#[test]
fn combined_c_flag_with_other_options() {
    let output = Command::new(meiksh())
        .args(["-ac", "echo $-"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().contains('a'),
        "expected 'a' in $- output, got: {stdout}"
    );
    assert!(
        stdout.trim().contains('c'),
        "expected 'c' in $- output, got: {stdout}"
    );
}

#[test]
fn set_e_and_set_x_work_at_runtime() {
    let output = Command::new(meiksh())
        .args(["-c", "set -x; echo traced"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("+ echo traced"),
        "expected xtrace output after set -x, got stderr: {stderr}"
    );

    let output = Command::new(meiksh())
        .args(["-c", "set -e; false; echo unreachable"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("unreachable"));
}

#[test]
fn dollar_dash_includes_new_option_flags() {
    let output = Command::new(meiksh())
        .args(["-ec", "echo $-"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().contains('e'),
        "expected 'e' in $- output, got: {stdout}"
    );
}

#[test]
fn dquote_backslash_preserves_non_special() {
    let output = Command::new(meiksh())
        .args(["-c", r#"printf '%s\n' "\a\z""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), r"\a\z");
}

#[test]
fn dquote_backslash_escapes_dollar_and_backslash() {
    let output = Command::new(meiksh())
        .args(["-c", r#"echo "\$HOME""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "$HOME");

    let output = Command::new(meiksh())
        .args(["-c", r#"echo "\\""#])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "\\");
}

#[test]
fn arithmetic_expansion_full_operators() {
    let cases: &[(&str, &str)] = &[
        ("echo $((3 + 4))", "7"),
        ("echo $((10 - 3))", "7"),
        ("echo $((3 * 4))", "12"),
        ("echo $((15 / 3))", "5"),
        ("echo $((17 % 5))", "2"),
        ("echo $((3 < 5))", "1"),
        ("echo $((5 < 3))", "0"),
        ("echo $((3 == 3))", "1"),
        ("echo $((3 != 5))", "1"),
        ("echo $((6 & 3))", "2"),
        ("echo $((6 | 3))", "7"),
        ("echo $((6 ^ 3))", "5"),
        ("echo $((~0))", "-1"),
        ("echo $((1 << 4))", "16"),
        ("echo $((16 >> 2))", "4"),
        ("echo $((1 && 1))", "1"),
        ("echo $((0 || 1))", "1"),
        ("echo $((!0))", "1"),
        ("echo $((1 ? 10 : 20))", "10"),
        ("echo $((0 ? 10 : 20))", "20"),
    ];
    for (cmd, expected) in cases {
        let output = Command::new(meiksh())
            .args(["-c", cmd])
            .output()
            .expect("run meiksh");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), *expected, "failed for: {cmd}");
    }
}

#[test]
fn arithmetic_variable_references() {
    let output = Command::new(meiksh())
        .args(["-c", "x=7; echo $((x + 3))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");

    let output = Command::new(meiksh())
        .args(["-c", "x=5; echo $(($x * 2))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "10");
}

#[test]
fn arithmetic_hex_and_octal() {
    let output = Command::new(meiksh())
        .args(["-c", "echo $((0xff))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "255");

    let output = Command::new(meiksh())
        .args(["-c", "echo $((010))"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "8");
}

#[test]
fn arithmetic_assignment_persists() {
    let output = Command::new(meiksh())
        .args(["-c", "x=1; y=$((x += 5)); echo $x $y"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "6 6");
}

#[test]
fn tilde_home_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ~/test"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.trim().starts_with('~'),
        "tilde should have been expanded, got: {stdout}"
    );
    assert!(
        stdout.trim().ends_with("/test"),
        "should end with /test, got: {stdout}"
    );
}

#[test]
fn tilde_user_expansion_via_getpwnam() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap();
    let output = Command::new(meiksh())
        .args(["-c", &format!("echo ~{user}")])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        !trimmed.starts_with('~'),
        "~{user} should have been expanded, got: {trimmed}"
    );
    assert!(
        trimmed.starts_with('/'),
        "should be an absolute path, got: {trimmed}"
    );
}

#[test]
fn tilde_unknown_user_preserved() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ~no_such_user_xyzzy_12345"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "~no_such_user_xyzzy_12345");
}

#[test]
fn tilde_in_assignment_after_colon() {
    let output = Command::new(meiksh())
        .args(["-c", "MYPATH=~/bin:~/lib; echo $MYPATH"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        !trimmed.contains('~'),
        "tildes should have been expanded, got: {trimmed}"
    );
    assert!(
        trimmed.contains(':'),
        "should have colon separator, got: {trimmed}"
    );
}

#[test]
fn subshell_resets_command_traps() {
    let output = Command::new(meiksh())
        .args(["-c", "trap 'echo PARENT' TERM; (trap)"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("PARENT"),
        "subshell should have reset command traps, got: {stdout}"
    );
}

#[test]
fn subshell_preserves_ignored_traps() {
    let output = Command::new(meiksh())
        .args(["-c", "trap '' TERM; (trap)"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("TERM"),
        "subshell should preserve ignored traps, got: {stdout}"
    );
}

#[test]
fn command_substitution_resets_command_traps() {
    let output = Command::new(meiksh())
        .args(["-c", "trap 'echo PARENT' TERM; echo $(trap)"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("PARENT"),
        "command substitution should have reset command traps, got: {stdout}"
    );
}

#[test]
fn direct_ast_execution_preserves_compound_commands() {
    let output = Command::new(meiksh())
        .args(["-c", "X=0; for i in 1 2 3; do X=$((X + i)); done; echo $X"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "6");

    let output = Command::new(meiksh())
        .args(["-c", "X=start; if true; then X=yes; else X=no; fi; echo $X"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "yes");

    let output = Command::new(meiksh())
        .args([
            "-c",
            "X=0; while [ $X -lt 3 ]; do X=$((X + 1)); done; echo $X",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");

    let output = Command::new(meiksh())
        .args(["-c", "case hello in he*) echo matched;; *) echo no;; esac"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "matched");
}

#[test]
fn brace_group_executes_in_current_environment() {
    let output = Command::new(meiksh())
        .args(["-c", "X=before; { X=after; }; echo $X"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "after");
}

#[test]
fn subshell_changes_do_not_affect_parent() {
    let output = Command::new(meiksh())
        .args(["-c", "X=parent; (X=child); echo $X"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "parent");
}

#[test]
fn cd_logical_and_physical_modes() {
    let output = Command::new(meiksh())
        .args(["-c", "cd -L / && pwd"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "/");

    let output = Command::new(meiksh())
        .args(["-c", "cd -P / && pwd -P"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "/");

    let output = Command::new(meiksh())
        .args(["-c", "cd -LP / && echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");

    let output = Command::new(meiksh())
        .args(["-c", "cd -PL / && echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "ok");
}

#[test]
fn cd_logical_resolves_dotdot() {
    let output = Command::new(meiksh())
        .args(["-c", "cd /tmp && cd .. && echo $PWD"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "/");
}

#[test]
fn trap_supports_broader_signal_names() {
    let output = Command::new(meiksh())
        .args(["-c", "trap 'echo caught' USR1 USR2 PIPE; trap -p USR1"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("USR1"), "should show USR1 trap: {stdout}");
    assert!(
        stdout.contains("echo caught"),
        "should show action: {stdout}"
    );
}

#[test]
fn trap_accepts_sig_prefix() {
    let output = Command::new(meiksh())
        .args(["-c", "trap 'echo yes' SIGTERM; trap -p TERM"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TERM"), "should show TERM trap: {stdout}");
}

#[test]
fn read_without_variable_reads_into_reply() {
    let output = Command::new(meiksh())
        .args(["-c", "read <<EOF\nhello\nEOF\necho $REPLY"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn umask_accepts_symbolic_s_perm() {
    let output = Command::new(meiksh())
        .args(["-c", "umask u+s; echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
}

#[test]
fn umask_accepts_symbolic_x_uppercase_perm() {
    let output = Command::new(meiksh())
        .args(["-c", "umask u+X; echo ok"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
}

#[test]
fn exec_with_redirection_only_applies_to_shell() {
    let tmp = TempDir::new("exec-redir");
    let outfile = tmp.join("out.txt");
    let script = format!(
        "exec > '{}'; echo redirected",
        outfile.display()
    );
    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let contents = fs::read_to_string(&outfile).expect("read output file");
    assert_eq!(contents.trim(), "redirected");
}

#[test]
fn exec_with_double_dash_passes_arguments() {
    let output = Command::new(meiksh())
        .args(["-c", "exec -- /bin/echo hello"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn backslash_newline_continuation_in_tokenizer() {
    let output = Command::new(meiksh())
        .args(["-c", "echo hel\\\nlo"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn assignment_with_nested_parameter_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "y=${x:-hello}; echo $y"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn if_with_empty_condition_is_syntax_error() {
    let output = Command::new(meiksh())
        .args(["-c", "if then fi"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn while_with_empty_condition_is_syntax_error() {
    let output = Command::new(meiksh())
        .args(["-c", "while do done"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn until_with_empty_condition_is_syntax_error() {
    let output = Command::new(meiksh())
        .args(["-c", "until do done"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn export_with_tilde_prefix_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "HOME=/fakehome; export V=~/bin; echo $V"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "/fakehome/bin"
    );
}

#[test]
fn export_with_known_tilde_user_expansion() {
    let output = Command::new(meiksh())
        .args(["-c", "export V=~root/bin; echo $V"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().ends_with("/bin"), "expected ~root/bin to expand, got: {stdout}");
    assert!(!stdout.contains('~'), "tilde should have been expanded");
}

#[test]
fn export_with_unknown_tilde_user_preserved() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "export V=~no_such_user_xyzzy_999/bin; echo $V",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "~no_such_user_xyzzy_999/bin"
    );
}

#[test]
fn kill_background_job_via_process_group() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            "sleep 60 & pid=$!; kill $pid; wait $pid 2>/dev/null; echo done",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "done");
}

#[test]
fn capture_output_preserves_non_utf8_bytes() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"v=$(printf 'A\377B\200C'); printf '%s' "$v" | od -An -t x1 | tr -d ' \n'"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "41ff428043"
    );
}

#[test]
fn pwd_initialized_from_getcwd_when_env_invalid() {
    let output = Command::new(meiksh())
        .args(["-c", "echo $PWD"])
        .env_remove("PWD")
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let pwd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(pwd.starts_with('/'), "PWD should be absolute, got: {pwd}");
}

#[test]
fn pwd_corrected_when_env_contains_dotdot() {
    let output = Command::new(meiksh())
        .args(["-c", "echo $PWD"])
        .env("PWD", "/tmp/../tmp")
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let pwd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!pwd.contains(".."), "PWD should not contain .., got: {pwd}");
}

#[test]
fn character_class_pattern_matching_uses_locale() {
    let output = Command::new(meiksh())
        .args(["-c", "case a in ([[:alpha:]]) echo yes;; (*) echo no;; esac"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "yes");
}

#[test]
fn parameter_default_with_at_fields() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"set a b c; for x in ${unset:-"$@"}; do echo "($x)"; done"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "(a)\n(b)\n(c)"
    );

    let output2 = Command::new(meiksh())
        .args([
            "-c",
            r#"set --; echo "${unset:-"$@"}""#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output2.status.success());
    assert_eq!(String::from_utf8_lossy(&output2.stdout).trim(), "");
}

#[test]
fn quoted_null_adjacent_to_empty_at_produces_one_field() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"set --; for x in ''"$@"; do echo "[$x]"; done"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");

    let output2 = Command::new(meiksh())
        .args([
            "-c",
            r#"set --; for x in 'pfx'"$@"; do echo "[$x]"; done"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output2.status.success());
    assert_eq!(String::from_utf8_lossy(&output2.stdout).trim(), "[pfx]");
}

#[test]
fn invalid_parameter_expansion_reports_error() {
    let output = Command::new(meiksh())
        .args(["-c", "echo ${%bad}"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
}

#[test]
fn parameter_pattern_removal_operators() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"f=archive.tar.gz; echo "${f%.*}" "${f%%.*}" "${f#*.}" "${f##*.}""#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "archive.tar archive tar.gz gz"
    );
}

#[test]
fn string_to_bytes_round_trips_non_ascii() {
    let output = Command::new(meiksh())
        .args([
            "-c",
            r#"v=$(printf '\351'); printf '%s' "$v" | od -An -t x1 | tr -d ' \n'"#,
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "e9");
}
