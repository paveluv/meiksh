use std::process::Command;
use std::os::unix::fs::PermissionsExt;
use std::{fs, time::{SystemTime, UNIX_EPOCH}};

fn meiksh() -> &'static str {
    env!("CARGO_BIN_EXE_meiksh")
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
fn handles_redirections() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("meiksh-redir-{unique}.txt"));
    let script = format!("printf hi > {}", path.display());

    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(&path).expect("read redirect target"), "hi");
    let _ = fs::remove_file(path);
}

#[test]
fn redirects_current_shell_builtins_and_compound_commands() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let builtin_path = std::env::temp_dir().join(format!("meiksh-builtin-redir-{unique}.txt"));
    let group_path = std::env::temp_dir().join(format!("meiksh-group-redir-{unique}.txt"));

    let builtin = Command::new(meiksh())
        .args(["-c", &format!("pwd > {}; printf ok", builtin_path.display())])
        .output()
        .expect("run meiksh");
    assert!(builtin.status.success());
    assert_eq!(String::from_utf8_lossy(&builtin.stdout), "ok");
    assert!(!fs::read_to_string(&builtin_path).expect("read builtin output").trim().is_empty());

    let group = Command::new(meiksh())
        .args(["-c", &format!("{{ printf inside; }} > {}; printf outside", group_path.display())])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "outside");
    assert_eq!(fs::read_to_string(&group_path).expect("read group output"), "inside");

    let pipeline = Command::new(meiksh())
        .args([
            "-c",
            &format!("{{ printf inside; }} > {} | wc -c", group_path.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(pipeline.status.success());
    assert_eq!(String::from_utf8_lossy(&pipeline.stdout).trim(), "0");
    assert_eq!(fs::read_to_string(&group_path).expect("read group output again"), "inside");

    let _ = fs::remove_file(builtin_path);
    let _ = fs::remove_file(group_path);
}

#[test]
fn handles_append_and_input_redirections() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let input = std::env::temp_dir().join(format!("meiksh-in-{unique}.txt"));
    let output = std::env::temp_dir().join(format!("meiksh-out-{unique}.txt"));
    fs::write(&input, "abc").expect("write input");

    let script = format!(
        "cat < {} > {}; printf def >> {}",
        input.display(),
        output.display(),
        output.display()
    );
    let status = Command::new(meiksh()).args(["-c", &script]).status().expect("run meiksh");
    assert!(status.success());
    assert_eq!(fs::read_to_string(&output).expect("read output"), "abcdef");
    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
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
        .args(["-c", "for item in a b; do printf $item; break; printf no; done"])
        .output()
        .expect("run meiksh");
    assert!(break_output.status.success());
    assert_eq!(String::from_utf8_lossy(&break_output.stdout), "a");

    let continue_output = Command::new(meiksh())
        .args(["-c", "for item in a b; do continue; printf no; done; printf ok"])
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
    assert!(String::from_utf8_lossy(&break_output.stderr).contains("break: only meaningful in a loop"));

    let return_output = Command::new(meiksh())
        .args(["-c", "return"])
        .output()
        .expect("run meiksh");
    assert!(!return_output.status.success());
    assert!(String::from_utf8_lossy(&return_output.stderr).contains("return: not in a function"));
}

#[test]
fn exec_builtin_replaces_process_in_subshell() {
    let output = Command::new(meiksh())
        .args(["-c", "exec /bin/echo hello"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "/bin/echo hello\n");
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

    let group = Command::new(meiksh())
        .args(["-c", "{ alias say='printf group'; say; }"])
        .output()
        .expect("run meiksh");
    assert!(group.status.success());
    assert_eq!(String::from_utf8_lossy(&group.stdout), "group");

    let function = Command::new(meiksh())
        .args(["-c", "f() { alias say='printf fn'; say; }; f"])
        .output()
        .expect("run meiksh");
    assert!(function.status.success());
    assert_eq!(String::from_utf8_lossy(&function.stdout), "fn");

    let conditional = Command::new(meiksh())
        .args(["-c", "if true; then alias say='printf branch'; say; fi"])
        .output()
        .expect("run meiksh");
    assert!(conditional.status.success());
    assert_eq!(String::from_utf8_lossy(&conditional.stdout), "branch");
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

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("meiksh-expand-spec-{unique}"));
    fs::create_dir(&dir).expect("create dir");
    fs::write(dir.join("a.txt"), "").expect("write a");
    fs::write(dir.join("b.txt"), "").expect("write b");
    fs::write(dir.join(".hidden.txt"), "").expect("write hidden");

    let glob = Command::new(meiksh())
        .current_dir(&dir)
        .args(["-c", "printf '%s|' *.txt \\*.txt .*\\.txt"])
        .output()
        .expect("run meiksh");
    assert!(glob.status.success());
    assert_eq!(String::from_utf8_lossy(&glob.stdout), "a.txt|b.txt|*.txt|.hidden.txt|");

    let noglob = Command::new(meiksh())
        .current_dir(&dir)
        .args(["-c", "set -f; printf '%s|' *.txt; set +f; printf '%s|' *.txt"])
        .output()
        .expect("run meiksh");
    assert!(noglob.status.success());
    assert_eq!(String::from_utf8_lossy(&noglob.stdout), "*.txt|a.txt|b.txt|");

    let shell_option = Command::new(meiksh())
        .current_dir(&dir)
        .args(["-f", "-c", "printf '%s|' *.txt"])
        .output()
        .expect("run meiksh");
    assert!(shell_option.status.success());
    assert_eq!(String::from_utf8_lossy(&shell_option.stdout), "*.txt|");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn falls_back_on_enoexec_scripts() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("meiksh-enoexec-{unique}"));
    fs::create_dir(&dir).expect("create dir");

    let slash_script = dir.join("slash-script");
    fs::write(&slash_script, "printf slash:$1").expect("write slash script");
    let mut permissions = fs::metadata(&slash_script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&slash_script, permissions).expect("chmod slash script");

    let slash_output = Command::new(meiksh())
        .args([
            "-c",
            &format!("{} arg", slash_script.display()),
        ])
        .output()
        .expect("run meiksh");
    assert!(slash_output.status.success());
    assert_eq!(String::from_utf8_lossy(&slash_output.stdout), "slash:arg");

    let path_script = dir.join("path-script");
    fs::write(&path_script, "cat").expect("write path script");
    let mut permissions = fs::metadata(&path_script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path_script, permissions).expect("chmod path script");

    let path_value = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default());
    let path_output = Command::new(meiksh())
        .env("PATH", path_value)
        .args(["-c", "printf piped | path-script"])
        .output()
        .expect("run meiksh");
    assert!(path_output.status.success());
    assert_eq!(String::from_utf8_lossy(&path_output.stdout), "piped");

    let _ = fs::remove_file(slash_script);
    let _ = fs::remove_file(path_script);
    let _ = fs::remove_dir(dir);
}

#[test]
fn handles_extended_redirection_matrix() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("meiksh-redir-matrix-{unique}"));
    fs::create_dir(&dir).expect("create dir");

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
    assert_eq!(String::from_utf8_lossy(&precedence_output.stdout).trim(), "0");
    assert_eq!(fs::read_to_string(&output).expect("read redirected output"), "hidden");

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
    let _ = fs::remove_file(append);
    let _ = fs::remove_file(rw);
    let _ = fs::remove_dir(dir);
}

#[test]
fn honors_noclobber_and_force_clobber() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("meiksh-noclobber-{unique}.txt"));
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

    let _ = fs::remove_file(path);
}

#[test]
fn background_and_or_returns_current_error() {
    let output = Command::new(meiksh())
        .args(["-c", "true && true &"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("background execution currently supports single pipelines")
    );
}

#[test]
fn interactive_shell_sources_env_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("meiksh-env-{unique}.sh"));
    let history = std::env::temp_dir().join(format!("meiksh-history-{unique}.txt"));
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
            child.stdin.as_mut().unwrap().write_all(b"printenv TEST_ENV_LOADED\nexit\n")?;
            child.wait_with_output()
        })
        .expect("run meiksh");

    assert!(output.status.success());
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(history);
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
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let marker = std::env::temp_dir().join(format!("meiksh-loop-{unique}.flag"));
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
    let _ = fs::remove_file(marker);
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
        .args(["-c", "for item; do LAST=$item; done; printf $LAST", "meiksh", "x", "y"])
        .output()
        .expect("run meiksh");
    assert!(positional.status.success());
    assert_eq!(String::from_utf8_lossy(&positional.stdout), "y");
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
        .args([
            "-c",
            "name=beta; case $name in *) printf yes ;; esac",
        ])
        .output()
        .expect("run meiksh");
    assert!(star.status.success());
    assert_eq!(String::from_utf8_lossy(&star.stdout), "yes");
}
