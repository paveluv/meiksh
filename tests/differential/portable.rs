use std::process::Command;

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

#[test]
fn matches_system_sh_on_portable_cases() {
    let cases = [
        "printf ok",
        "X=1; export X; printenv X",
        "printf abc | wc -c",
        "false || printf pass",
        "true && printf pass",
        "greet() { printf hi; }; greet",
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
        "unset X; printf '<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"; X=''; printf '|<%s><%s><%s><%s>' \"${X-word}\" \"${X:-word}\" \"${X+alt}\" \"${X:+alt}\"",
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
