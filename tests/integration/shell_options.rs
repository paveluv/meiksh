use super::common::{TempDir, meiksh, run_meiksh_with_nonblocking_stdin};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

// ── Syntax checking ──

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

// ── Invocation options ──

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

// ── Interactive shell ──

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
    assert!(
        stdout.contains("1"),
        "ENV file should set TEST_ENV_LOADED=1, got: {stdout}"
    );
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
fn interactive_shell_exports_meiksh_version_marker() {
    // Spec docs/features/startup-files.md § 5: the `MEIKSH_VERSION`
    // variable is set to the crate's SemVer string and exported before
    // any startup file is sourced, so a child process started inside
    // the interactive shell inherits it via `execve(2)`.
    let home = TempDir::new("meiksh-marker-home");
    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .env_remove("ENV")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printenv MEIKSH_VERSION\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = env!("CARGO_PKG_VERSION");
    assert_eq!(
        stdout.trim(),
        expected,
        "MEIKSH_VERSION should be exported to the child printenv call \
         as the crate SemVer string, got stdout: {stdout:?}"
    );
}

#[test]
fn interactive_shell_sources_home_profile() {
    // Spec docs/features/startup-files.md § 3.2.
    let home = TempDir::new("meiksh-home-profile");
    let profile = home.join(".profile");
    fs::write(&profile, "export FROM_HOME_PROFILE=home_ok\n").expect("write ~/.profile");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .env_remove("ENV")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printenv FROM_HOME_PROFILE\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("home_ok"),
        "~/.profile should have set FROM_HOME_PROFILE=home_ok, got stdout: {stdout}"
    );
}

#[test]
fn interactive_shell_sources_home_profile_before_env() {
    // Spec § 3: ordering is /etc/profile → ~/.profile → $ENV. The two
    // user files both assign the same variable; the final value is
    // whatever $ENV set last.
    let home = TempDir::new("meiksh-order-home");
    let profile = home.join(".profile");
    fs::write(&profile, "ORDER_VAR=home\nexport ORDER_VAR\n").expect("write ~/.profile");

    let env_file = home.join("env.sh");
    fs::write(&env_file, "ORDER_VAR=env\nexport ORDER_VAR\n").expect("write env file");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .env("ENV", &env_file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printenv ORDER_VAR\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("env"),
        "ORDER_VAR should be re-assigned by $ENV after ~/.profile, got stdout: {stdout}"
    );
}

#[test]
fn interactive_shell_home_profile_can_see_meiksh_version_marker() {
    // Spec § 5: MEIKSH_VERSION is established before ~/.profile runs
    // so that profile scripts can branch on shell identity. Without
    // the marker our `.profile` writes `FROM_PROFILE=empty`; with it,
    // it writes the marker's value (the crate SemVer string).
    let home = TempDir::new("meiksh-marker-profile");
    let profile = home.join(".profile");
    fs::write(
        &profile,
        "if [ -n \"${MEIKSH_VERSION:-}\" ]; then FROM_PROFILE=\"meiksh=${MEIKSH_VERSION}\"; else FROM_PROFILE=empty; fi\nexport FROM_PROFILE\n",
    )
    .expect("write ~/.profile");

    let output = Command::new(meiksh())
        .arg("-i")
        .env("HOME", home.path())
        .env_remove("ENV")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"printenv FROM_PROFILE\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh interactive");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("meiksh={}", env!("CARGO_PKG_VERSION"));
    assert!(
        stdout.contains(&expected),
        "~/.profile should observe MEIKSH_VERSION={}, got stdout: {stdout}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn non_interactive_shell_skips_startup_files() {
    // Spec § 2: non-interactive shells source no startup file. We
    // invoke `sh -c` with HOME/ENV pointing at loud files; neither
    // should run, so the command exits cleanly and prints only the
    // body of the -c string.
    let home = TempDir::new("meiksh-noninteractive");
    let profile = home.join(".profile");
    fs::write(&profile, "echo FROM_HOME_PROFILE_RAN\n").expect("write ~/.profile");
    let env_file = home.join("env.sh");
    fs::write(&env_file, "echo FROM_ENV_RAN\n").expect("write env file");

    let output = Command::new(meiksh())
        .args(["-c", "echo ok"])
        .env("HOME", home.path())
        .env("ENV", &env_file)
        .output()
        .expect("run meiksh -c");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        "ok",
        "non-interactive shell should source no startup files, got stdout: {stdout}"
    );
}

#[test]
fn interactive_shell_sources_env_file() {
    let root = TempDir::new("meiksh-env");
    let path = root.join("env.sh");
    let history = root.join("history.txt");
    fs::write(&path, "export TEST_ENV_LOADED=1\n").expect("write env file");

    let output = Command::new(meiksh())
        .arg("-i")
        // Sandbox HOME at the temp dir so the developer's real
        // `$HOME/.profile` does not leak into the test. On a typical
        // FreeBSD install ~/.profile contains `ENV=$HOME/.shrc;
        // export ENV`, which is sourced *before* the spec § 3.3
        // `$ENV` step and therefore overwrites the test-provided
        // `ENV=<tmp>/env.sh` — making this test silently fail when
        // run by a developer with a default-shipped home dir. The
        // temp dir has no `.profile`, so the sourcing chain
        // collapses to /etc/profile (no ENV override there) → noop
        // missing ~/.profile → our caller-provided $ENV.
        .env("HOME", root.path())
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

// ── xtrace ──

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

// ── Aliases ──

#[test]
fn aliases_defined_earlier_in_same_source_affect_later_commands() {
    let simple = Command::new(meiksh())
        .args(["-c", "alias say='printf ok'\nsay"])
        .output()
        .expect("run meiksh");
    assert!(simple.status.success());
    assert_eq!(String::from_utf8_lossy(&simple.stdout), "ok");

    let reserved = Command::new(meiksh())
        .args(["-c", "alias cond='if'\ncond true; then printf yes; fi"])
        .output()
        .expect("run meiksh");
    assert!(reserved.status.success());
    assert_eq!(String::from_utf8_lossy(&reserved.stdout), "yes");

    let group = Command::new(meiksh())
        .args(["-c", "alias say='printf group'\n{ say; }"])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "group");

    let function = Command::new(meiksh())
        .args(["-c", "alias say='printf fn'\nf() { say; }; f"])
        .output()
        .expect("run meiksh");
    assert!(function.status.success());
    assert_eq!(String::from_utf8_lossy(&function.stdout), "fn");

    let conditional = Command::new(meiksh())
        .args(["-c", "alias say='printf branch'\nif true; then say; fi"])
        .output()
        .expect("run meiksh");
    assert!(conditional.status.success());
    assert_eq!(String::from_utf8_lossy(&conditional.stdout), "branch");

    let heredoc_nested = Command::new(meiksh())
        .args(["-c", "alias say='cat'\nf() { say <<EOF\nhello\nEOF\n}; f"])
        .output()
        .expect("run meiksh");
    assert!(heredoc_nested.status.success());
    assert_eq!(String::from_utf8_lossy(&heredoc_nested.stdout), "hello\n");
}

// ── enoexec fallback ──

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
