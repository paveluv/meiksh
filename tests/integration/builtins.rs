use super::common::*;
use libc;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::Command;

// ── export, readonly, unset, pwd ──

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
    assert_eq!(lines[3], "alias ll='printf alias'");
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

// ── read builtin ──

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
fn read_without_variable_reads_into_reply() {
    let output = Command::new(meiksh())
        .args(["-c", "read <<EOF\nhello\nEOF\necho $REPLY"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

// ── umask and times ──

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

// ── exec builtin ──

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
fn exec_with_redirection_only_applies_to_shell() {
    let tmp = TempDir::new("exec-redir");
    let outfile = tmp.join("out.txt");
    let script = format!("exec > '{}'; echo redirected", outfile.display());
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

// ── trap builtin ──

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
fn ignored_on_entry_signal_reported_by_trap() {
    let out = unsafe {
        Command::new(meiksh())
            .args(["-c", "trap -p USR1"])
            .pre_exec(|| {
                libc::signal(libc::SIGUSR1, libc::SIG_IGN);
                Ok(())
            })
            .output()
            .expect("run")
    };
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("trap -- '' USR1"),
        "expected ignored trap, got: {stdout}"
    );
}

// ── subshell trap behavior ──

#[test]
fn subshell_resets_command_traps() {
    let output = Command::new(meiksh())
        .args(["-c", "trap 'echo PARENT' TERM; (trap)"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PARENT"),
        "subshell trap (no operands) should show parent traps, got: {stdout}"
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
        stdout.contains("PARENT"),
        "command substitution trap (no operands) should show parent traps, got: {stdout}"
    );
}

// ── cd builtin ──

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
fn cd_unset_home_fails() {
    let out = Command::new(meiksh())
        .args(["-c", "unset HOME; cd 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

// ── pwd ──

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

// ── alias / unalias ──

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
fn unalias_diagnostic_for_unknown() {
    let out = Command::new(meiksh())
        .args(["-c", "unalias no_such_alias"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not found"));
}

#[test]
fn command_v_alias_prefix() {
    let out = Command::new(meiksh())
        .args(["-c", "alias greet='echo hi'; command -v greet"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "alias greet='echo hi'"
    );
}

// ── wait ──

#[test]
fn wait_second_returns_127_and_diagnostic() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "sleep 0 &\np=$!\nwait $p\nwait $p 2>/dev/null\nprintf '%s\\n' $?",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "127");

    let out = Command::new(meiksh())
        .args(["-c", "wait 999999"])
        .output()
        .expect("run");
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a child"));
}

// ── kill ──

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

// ── bg ──

#[test]
fn bg_sends_sigcont_to_background_job() {
    let output = Command::new(meiksh())
        .args(["-c", "sleep 0.01 & bg %1 2>/dev/null; wait"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}

// ── type builtin ──

#[test]
fn type_builtin_finds_builtins_and_externals() {
    let out = Command::new(meiksh())
        .args(["-c", "type cd"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("cd"));

    let out = Command::new(meiksh())
        .args(["-c", "type no_such_thing_at_all"])
        .output()
        .expect("run");
    assert!(!out.status.success());
}

// ── hash builtin ──

#[test]
fn hash_builtin_caches_and_clears() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "hash ls 2>/dev/null\nhash | grep -q ls && echo found || echo empty\nhash -r\nhash | grep -q ls && echo found || echo cleared",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("found"), "hash: {stdout}");
    assert!(stdout.contains("cleared"), "hash -r: {stdout}");
}

#[test]
fn hash_skips_builtins_and_functions() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "myfunc() { :; }\nhash cd myfunc 2>/dev/null\nhash\necho done",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "done");
}

#[test]
fn hash_path_change_clears_cache() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            "hash ls 2>/dev/null\nPATH=\"$PATH\"\nhash | grep -q ls && echo still || echo gone",
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("gone"));
}

#[test]
fn path_cache_hit_resolves_command() {
    let out = Command::new(meiksh())
        .args(["-c", "hash ls 2>/dev/null\nls /dev/null"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("/dev/null"));
}

#[test]
fn hash_not_found_error() {
    let out = Command::new(meiksh())
        .args(["-c", "hash no_such_binary_ever_xyzzy 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

// ── ulimit builtin ──

#[test]
fn ulimit_get_and_set() {
    for opt in ['c', 'd', 'f', 'n', 's', 't', 'v'] {
        let out = Command::new(meiksh())
            .args(["-c", &format!("ulimit -{opt}")])
            .output()
            .expect("run");
        assert!(out.status.success(), "ulimit -{opt} failed");
    }

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -a"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).lines().count() >= 7);

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -Hf"])
        .output()
        .expect("run");
    assert!(out.status.success());

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -Sf"])
        .output()
        .expect("run");
    assert!(out.status.success());

    let out = Command::new(meiksh())
        .args(["-c", "cur=$(ulimit -Sf)\nulimit -Sf \"$cur\"\necho $?"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0");

    let _out = Command::new(meiksh())
        .args(["-c", "ulimit -f unlimited"])
        .output()
        .expect("run");

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -Sc 0; echo $?"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0");

    let out = Command::new(meiksh())
        .args(["-c", "ulimit bad_value"])
        .output()
        .expect("run");
    assert!(!out.status.success());

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -z"])
        .output()
        .expect("run");
    assert!(!out.status.success());

    let out = Command::new(meiksh())
        .args(["-c", "ulimit -Hn 1 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

// ── fc builtin (interactive mode) ──

#[test]
fn fc_reexec_mode() {
    let out = run_interactive(b"echo original\nfc -s\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.matches("original").count(), 2, "fc -s: {stdout}");

    let out = run_interactive(b"echo old_word\nfc -s old_word=new_word\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("old_word") && stdout.contains("new_word"),
        "fc -s sub: {stdout}"
    );

    let out = run_interactive(b"echo first\necho second\nfc -s -2\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.matches("first").count(), 2, "fc -s -2: {stdout}");

    let out = run_interactive(b"echo alpha\necho beta\nfc -s echo\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("beta"), "fc -s echo: {stdout}");

    let out = run_interactive(b"echo hi\nfc -s zzz_none\nexit\n");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no command found"), "fc -s bad: {stderr}");
}

#[test]
fn fc_list_mode() {
    let out = run_interactive(b"echo aaa\necho bbb\necho ccc\nfc -l\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo aaa"), "fc -l: {stdout}");

    let out = run_interactive(b"echo aaa\necho bbb\nfc -ln\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo aaa"), "fc -ln: {stdout}");

    let out = run_interactive(b"echo aaa\necho bbb\nfc -lr\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo"), "fc -lr: {stdout}");

    let out = run_interactive(b"echo aaa\necho bbb\nfc -lnr\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo aaa"), "fc -lnr: {stdout}");
}

#[test]
fn fc_edit_mode() {
    let out = run_interactive(b"echo before_edit\nFCEDIT=cat\nfc\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("before_edit"), "fc FCEDIT: {stdout}");

    let out = run_interactive(b"echo before_edit2\nfc -e cat\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("before_edit2"), "fc -e: {stdout}");

    let out = run_interactive(b"echo hi\nfc -ecat\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo hi"), "fc -ecat: {stdout}");

    let out = run_interactive(b"echo target_cmd\nfc -e cat 1\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("target_cmd"), "fc -e cat 1: {stdout}");

    let out = run_interactive(b"echo test_edit\nfc -e false\nexit\n");
    let _ = out;
}

#[test]
fn fc_edit_mode_empty_result() {
    let editor_script = "/tmp/meiksh_test_empty_editor.sh";
    fs::write(editor_script, "#!/bin/sh\n: > \"$1\"\n").expect("write");
    fs::set_permissions(editor_script, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = run_interactive(
        format!("echo original\nfc -e {editor_script}\necho done\nexit\n").as_bytes(),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("done"), "fc empty: {stdout}");
    let _ = fs::remove_file(editor_script);
}

#[test]
fn fc_option_parsing() {
    let out = run_interactive(b"echo hello\nfc -l -- -1 -1\nexit\n");
    assert!(out.status.success());

    let out = Command::new(meiksh())
        .args(["-c", "fc -z 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");

    let out = Command::new(meiksh())
        .args(["-c", "fc -e 2>/dev/null; echo $?"])
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");

    let out = Command::new(meiksh())
        .args(["-c", "fc -l 2>/dev/null"])
        .output()
        .expect("run");
    assert!(out.status.success());
}

#[test]
fn add_history_edge_cases() {
    let out = run_interactive(b"HISTSIZE=2\necho a\necho b\necho c\nfc -l\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("echo a\n"), "evicted: {stdout}");
}

#[test]
fn fc_list_one_operand() {
    let out = run_interactive(b"echo first\necho second\necho third\nfc -l 2\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("echo second"), "fc -l 2: {stdout}");
}

#[test]
fn fc_list_reversed_range() {
    let out = run_interactive(b"echo alpha\necho beta\necho gamma\nfc -l 3 1\nexit\n");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("echo alpha") && stdout.contains("echo gamma"),
        "fc -l 3 1: {stdout}"
    );
}

// ── getopts builtin ──

#[test]
fn getopts_parses_positional_params_and_explicit_params() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            r#"
OPTIND=1; set -- -a -b; r=
while getopts ab name; do r="${r}${name}"; done
printf '%s\n' "$r"
OPTIND=1; r=
while getopts xy name -x -y; do r="${r}${name}"; done
printf '%s:%s\n' "$r" "$OPTIND"
"#,
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ab\nxy:3");
}

#[test]
fn getopts_option_with_separate_and_attached_arg() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            r#"
OPTIND=1; set -- -f value
getopts f: name
printf 'name=%s optarg=%s optind=%s\n' "$name" "$OPTARG" "$OPTIND"
OPTIND=1; set -- -fvalue
getopts f: name
printf 'name=%s optarg=%s optind=%s\n' "$name" "$OPTARG" "$OPTIND"
"#,
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=f optarg=value optind=3\nname=f optarg=value optind=2"
    );
}

#[test]
fn getopts_unsets_optarg_for_options_without_argument() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            r#"OPTIND=1; OPTARG=stale; set -- -a; getopts a name; printf '%s\n' "${OPTARG-unset}""#,
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "unset");
}

#[test]
fn getopts_invalid_option_normal_and_silent_mode() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -z; getopts ab name 2>/dev/null; printf 'name=%s optarg=%s\n' "$name" "${OPTARG-unset}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=? optarg=unset"
    );

    let out2 = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -z; getopts :ab name 2>/dev/null; printf 'name=%s optarg=%s\n' "$name" "$OPTARG""#])
        .output()
        .expect("run");
    assert!(out2.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out2.stdout).trim(),
        "name=? optarg=z"
    );
}

#[test]
fn getopts_missing_argument_normal_and_silent_mode() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -f; getopts f: name 2>/dev/null; printf 'name=%s optarg=%s\n' "$name" "${OPTARG-unset}""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=? optarg=unset"
    );

    let out2 = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -f; getopts :f: name 2>/dev/null; printf 'name=%s optarg=%s\n' "$name" "$OPTARG""#])
        .output()
        .expect("run");
    assert!(out2.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out2.stdout).trim(),
        "name=: optarg=f"
    );
}

#[test]
fn getopts_double_dash_ends_options() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -a -- -b; getopts a name; getopts a name; printf 'name=%s optind=%s status=%s\n' "$name" "$OPTIND" "$?""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=? optind=3 status=1"
    );
}

#[test]
fn getopts_non_option_operand_ends_options() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -a operand; getopts a name; getopts a name; printf 'name=%s optind=%s status=%s\n' "$name" "$OPTIND" "$?""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=? optind=2 status=1"
    );
}

#[test]
fn getopts_empty_param_list_reports_end() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set --; getopts a name; printf 'name=%s optind=%s status=%s\n' "$name" "$OPTIND" "$?""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=? optind=1 status=1"
    );
}

#[test]
fn getopts_combined_short_options() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -abc; r=; while getopts abc name; do r="${r}${name}"; done; printf '%s\n' "$r""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "abc");
}

#[test]
fn getopts_grouped_with_trailing_argument() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -abf file.txt; r=; a=; while getopts abf: name; do r="${r}${name}"; [ "$name" = f ] && a=$OPTARG; done; printf '%s:%s\n' "$r" "$a""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "abf:file.txt");
}

#[test]
fn getopts_invalid_option_within_group_resumes() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -za; getopts ab name 2>/dev/null; getopts ab name 2>/dev/null; printf 'name=%s status=%s\n' "$name" "$?""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=a status=0"
    );
}

#[test]
fn getopts_resumes_after_invalid_option() {
    let out = Command::new(meiksh())
        .args(["-c", r#"OPTIND=1; set -- -z -a; getopts ab name 2>/dev/null; getopts ab name 2>/dev/null; printf 'name=%s status=%s\n' "$name" "$?""#])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "name=a status=0"
    );
}

#[test]
fn getopts_readonly_errors() {
    for script in &[
        "readonly name; OPTIND=1; set -- -a; getopts a name 2>/dev/null",
        "OPTIND=1; readonly OPTIND; set -- -a; getopts a name 2>/dev/null",
        "OPTIND=1; readonly OPTARG; set -- -f val; getopts f: name 2>/dev/null",
    ] {
        let out = Command::new(meiksh())
            .args(["-c", script])
            .output()
            .expect("run");
        assert!(
            out.status.code().unwrap_or(0) > 1,
            "expected >1 for: {script}, got {}",
            out.status.code().unwrap_or(0)
        );
    }
}

#[test]
fn getopts_usage_error() {
    let out = Command::new(meiksh())
        .args(["-c", "getopts ab 2>/dev/null"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn getopts_optind_initialized_to_one() {
    let out = Command::new(meiksh())
        .args(["-c", "printf '%s\\n' \"$OPTIND\""])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

#[test]
fn getopts_reset_optind_allows_reparsing() {
    let out = Command::new(meiksh())
        .args([
            "-c",
            r#"
OPTIND=1; set -- -a -b; r=
while getopts ab name; do r="${r}${name}"; done
OPTIND=1; set -- -x -y
while getopts xy name; do r="${r}${name}"; done
printf '%s\n' "$r"
"#,
        ])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "abxy");
}

// ── builtin error behavior ──

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
