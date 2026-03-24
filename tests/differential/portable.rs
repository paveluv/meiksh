use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io::Write};

fn meiksh() -> &'static str {
    env!("CARGO_BIN_EXE_meiksh")
}

fn run(shell: &str, script: &str) -> (i32, String, String) {
    let output = Command::new(shell)
        .args(["-c", script])
        .output()
        .expect("run shell");
    (
        output.status.code().unwrap_or(128),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_with_args_and_stdin(shell: &str, args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut child = Command::new(shell)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run shell");
    child.stdin.take().expect("stdin").write_all(stdin).expect("write stdin");
    let output = child.wait_with_output().expect("wait shell");
    (
        output.status.code().unwrap_or(128),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn matches_system_sh_on_portable_cases() {
    let cases = [
        "printf ok",
        "X=1; export X; printenv X",
        "printf abc | wc -c",
        "printf abc |\n wc -c",
        "false || printf pass",
        "false ||\n printf pass",
        "true && printf pass",
        "true &&\n printf pass",
        "greet() { printf hi; }; greet",
        "{ printf ok; }",
        "echo }",
        "if false; then printf no; elif true; then printf yes; else printf bad; fi",
        "while false; do printf no; done; printf ok",
        "until true; do printf no; done; printf ok",
        "for item in a b c; do LAST=$item; done; printf $LAST",
        "name=beta; case $name in alpha) printf no ;; beta|gamma) printf yes ;; esac",
        "name=report.txt; case $name in *.log) printf no ;; *.txt) printf yes ;; esac",
        "f() { printf hi; return 7; printf no; }; f",
        "for item in a b; do printf $item; break; printf no; done",
        "for item in a b; do continue; printf no; done; printf ok",
        "{ printf inside; } >/dev/null; printf outside",
        "pwd >/dev/null; printf outside",
        "set -- a b c d e f g h i j; printf '%s|%s|%s' \"$10\" \"${10}\" \"${#10}\"",
        "VALUE='a b'; unset IFS; printf '<%s>' $VALUE; IFS=; printf '|<%s>' $VALUE; set -- a b c; IFS=:; printf '|<%s><%s>' $* \"$*\"",
        "unset X; printf '<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"; X=''; printf '|<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"",
        "alias ll='printf nope'; unalias -a; alias ll >/dev/null 2>&1; printf :$?",
        "trap 'printf exit:$?' EXIT; false",
        "trap 'printf INT:$?' INT; kill -INT $$; printf done",
        "wait 999999; printf %s $?",
        "VALUE=world; cat <<EOF\nhello $VALUE\nEOF\n",
        "VALUE=world; cat <<'EOF'\nhello $VALUE\nEOF\n",
        "cat <<-\tEOF\n\tstrip-me\n\tEOF\n",
    ];

    for script in cases {
        let mesh_result = run(meiksh(), script);
        let sh_result = run("sh", script);
        assert_eq!(mesh_result.0, sh_result.0, "status mismatch for {script}");
        assert_eq!(mesh_result.1, sh_result.1, "stdout mismatch for {script}");
    }
}

#[test]
fn matches_system_sh_on_s_option_case() {
    let meiksh_s = run_with_args_and_stdin(meiksh(), &["-s", "alpha", "beta"], b"printf '%s|%s' \"$1\" \"$2\"\n");
    let sh_s = run_with_args_and_stdin("sh", &["-s", "alpha", "beta"], b"printf '%s|%s' \"$1\" \"$2\"\n");
    assert_eq!(meiksh_s, sh_s, "-s positional behavior mismatch");
}

#[test]
fn matches_system_sh_on_c_command_name_case() {
    let meiksh_c = run_with_args_and_stdin(meiksh(), &["-c", "printf %s \"$0\"", "cmd-name"], b"");
    let sh_c = run_with_args_and_stdin("sh", &["-c", "printf %s \"$0\"", "cmd-name"], b"");
    assert_eq!(meiksh_c, sh_c, "-c command_name behavior mismatch");
}

#[test]
fn matches_system_sh_on_lone_dash_case() {
    let meiksh_dash = run_with_args_and_stdin(meiksh(), &["-"], b"printf ok\n");
    let sh_dash = run_with_args_and_stdin("sh", &["-"], b"printf ok\n");
    assert_eq!(meiksh_dash, sh_dash, "lone dash behavior mismatch");
}

#[test]
fn matches_system_sh_on_noninteractive_stdin_no_read_ahead_case() {
    let meiksh_stream = run_with_args_and_stdin(meiksh(), &[], b"cat\necho after\n");
    let sh_stream = run_with_args_and_stdin("sh", &[], b"cat\necho after\n");
    assert_eq!(meiksh_stream, sh_stream, "non-interactive stdin no-read-ahead mismatch");
}

#[test]
fn matches_system_sh_on_allexport_startup_case() {
    let meiksh_export = run(meiksh(), "set -a; AUTO=works; printenv AUTO");
    let sh_export = run("sh", "set -a; AUTO=works; printenv AUTO");
    assert_eq!(meiksh_export, sh_export, "set -a allexport behavior mismatch");

    let meiksh_startup = run_with_args_and_stdin(meiksh(), &["-a", "-c", "AUTO=works; printenv AUTO"], b"");
    let sh_startup = run_with_args_and_stdin("sh", &["-a", "-c", "AUTO=works; printenv AUTO"], b"");
    assert_eq!(meiksh_startup, sh_startup, "startup -a behavior mismatch");
}

#[test]
fn matches_system_sh_on_named_noglob_startup_case() {
    let meiksh_noglob = run_with_args_and_stdin(meiksh(), &["-o", "noglob", "-c", "printf '%s' *.definitely_missing"], b"");
    let sh_noglob = run_with_args_and_stdin("sh", &["-o", "noglob", "-c", "printf '%s' *.definitely_missing"], b"");
    assert_eq!(meiksh_noglob, sh_noglob, "startup -o noglob behavior mismatch");
}

#[test]
fn matches_system_sh_on_cd_dash_case() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("meiksh-diff-cd-{unique}"));
    let target = root.join("target");
    fs::create_dir_all(&target).expect("mkdir target");
    let script = format!("cd '{}'; cd - >/dev/null; printf '%s|%s' \"$PWD\" \"$OLDPWD\"", target.display());
    let meiksh_cd = run(meiksh(), &script);
    let sh_cd = run("sh", &script);
    assert_eq!(meiksh_cd, sh_cd, "cd - behavior mismatch");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn matches_system_sh_on_cdpath_case() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("meiksh-diff-cdpath-{unique}"));
    let cdpath = root.join("cdpath");
    let target = cdpath.join("target");
    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(&target).expect("mkdir target");
    fs::create_dir_all(&elsewhere).expect("mkdir elsewhere");
    let script = "CDPATH='../cdpath'; cd target; printf '|%s|%s' \"$PWD\" \"$OLDPWD\"";

    let meiksh_cd = Command::new(meiksh())
        .current_dir(&elsewhere)
        .args(["-c", script])
        .output()
        .expect("run meiksh");
    let sh_cd = Command::new("sh")
        .current_dir(&elsewhere)
        .args(["-c", script])
        .output()
        .expect("run sh");
    assert_eq!(
        (
            meiksh_cd.status.code().unwrap_or(128),
            String::from_utf8_lossy(&meiksh_cd.stdout).into_owned(),
            String::from_utf8_lossy(&meiksh_cd.stderr).into_owned(),
        ),
        (
            sh_cd.status.code().unwrap_or(128),
            String::from_utf8_lossy(&sh_cd.stdout).into_owned(),
            String::from_utf8_lossy(&sh_cd.stderr).into_owned(),
        ),
        "cdpath behavior mismatch"
    );
    let _ = fs::remove_dir_all(root);
}
