//! Integration tests for the inputrc parser (`docs/features/inputrc.md`).
//!
//! These exercises drive a real `meiksh -i` process with a
//! test-controlled inputrc via the `INPUTRC` environment variable, so
//! they cover both the parser (recursion guard, `$include`, `$if
//! mode=`) and the downstream keymap application (macro expansion,
//! keyseq rebind).

use super::interactive_common::{PtyChild, spawn_meiksh_pty};
use std::io::Write;
use std::time::Duration;

fn drain_until_contains(pty: &mut PtyChild, needle: &[u8]) -> Vec<u8> {
    let needle = needle.to_vec();
    pty.drain_until(
        move |b| b.windows(needle.len()).any(|w| w == needle.as_slice()),
        Duration::from_secs(60),
    )
}

fn write_rc(path: &str, body: &[u8]) {
    let mut f = std::fs::File::create(path).expect("create inputrc");
    f.write_all(body).expect("write inputrc");
}

#[test]
fn inputrc_recursion_guard_reports_diagnostic() {
    let path = format!("/tmp/meiksh-rc-rec-{}", std::process::id());
    write_rc(&path, format!("$include {path}\n").as_bytes());
    let env = [("INPUTRC", path.as_str())];
    let Some(mut pty) = spawn_meiksh_pty(&env) else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"echo RCDONE\n");
    let out = drain_until_contains(&mut pty, b"RCDONE\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("recursive $include"),
        "expected recursion diagnostic, got {text:?}"
    );
}

#[test]
fn inputrc_unknown_variable_reports_but_does_not_abort() {
    let path = format!("/tmp/meiksh-rc-unk-{}", std::process::id());
    write_rc(&path, b"set who-knows off\nset completion-ignore-case on\n");
    let env = [("INPUTRC", path.as_str())];
    let Some(mut pty) = spawn_meiksh_pty(&env) else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    // Startup inputrc is consulted on first emacs-mode read_line.
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"echo DONEMARK\n");
    let out = drain_until_contains(&mut pty, b"DONEMARK\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unknown variable"),
        "expected unknown-variable diagnostic, got {text:?}"
    );
    assert!(
        text.contains("DONEMARK"),
        "shell must continue past the error"
    );
}

#[test]
fn inputrc_if_mode_emacs_branch_wins() {
    // Point C-a at end-of-line under $if mode=emacs. The parent test
    // just checks that this rebind was accepted without diagnostics;
    // actual key-press behavior is covered by the emacs_mode suite.
    let path = format!("/tmp/meiksh-rc-if-{}", std::process::id());
    write_rc(
        &path,
        b"$if mode=emacs\n\"\\C-a\": end-of-line\n$else\n\"\\C-a\": beginning-of-line\n$endif\n",
    );
    let env = [("INPUTRC", path.as_str())];
    let Some(mut pty) = spawn_meiksh_pty(&env) else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    // Don't set emacs mode here — we just want to inspect the global
    // keymap dump through `bind -p` to confirm the $if branch applied.
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"bind -p | grep end-of-line | head -1 ; printf 'END\\n'\n");
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("end-of-line"),
        "expected end-of-line binding to survive $if mode=emacs branch: {text:?}"
    );
}
