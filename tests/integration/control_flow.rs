use super::common::*;
use libc;
use std::fs;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

// ── if/elif/else ──

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
fn if_with_empty_condition_is_syntax_error() {
    let output = Command::new(meiksh())
        .args(["-c", "if then fi"])
        .output()
        .expect("run meiksh");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
}

// ── while/until ──

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

// ── for loops ──

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

// ── case commands ──

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

// ── Functions ──

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

// ── Subshell and brace group ──

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
fn case_pattern_that_is_a_reserved_word_name_still_matches() {
    // POSIX 2.10.1: reserved-word recognition does not apply to the word
    // after `case`, nor to case patterns. Patterns spelled with reserved
    // word names (e.g. `if`, `then`, `else`) must match ordinary-word
    // equality.
    for source in [
        "case if in if) echo ok;; esac",
        "case then in if|then) echo ok;; esac",
        "case foo in if) echo no;; *) echo ok;; esac",
    ] {
        let output = Command::new(meiksh())
            .args(["-c", source])
            .output()
            .expect("run meiksh");
        assert!(
            output.status.success(),
            "source={source:?} status={:?}",
            output.status
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "ok",
            "source={source:?}"
        );
    }
}

// ── Pipeline and and/or ──

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
fn background_and_or_list_runs_asynchronously() {
    let output = Command::new(meiksh())
        .args(["-c", "true && true &"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
}

// ── errexit ──

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

// ── PTY-based foreground control ──

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
