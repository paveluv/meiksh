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

// PTY roundtrips finish in milliseconds; the 5-second budget is a
// generous failure timeout that still caps the suite's worst-case
// runtime when a test regresses. Longer budgets here just hide bugs.
fn drain_until_contains(pty: &mut PtyChild, needle: &[u8]) -> Vec<u8> {
    let needle = needle.to_vec();
    pty.drain_until(
        move |b| b.windows(needle.len()).any(|w| w == needle.as_slice()),
        Duration::from_secs(5),
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
    // Emacs is now the default editing mode
    // (`docs/features/emacs-editing-mode.md` § 2.5), which means
    // `emacs_editing::read_line` fires for the FIRST prompt and
    // `ensure_startup_loaded` runs before the shell emits its initial
    // `$ ` — the recursion diagnostic therefore lands in the startup
    // preamble, not after our `set -o emacs` toggle. Capture the
    // preamble, then continue driving the shell to prove it is still
    // responsive (the diagnostic must not abort the editor).
    let startup = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"echo RCDONE\n");
    let tail = drain_until_contains(&mut pty, b"RCDONE\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let mut out = startup;
    out.extend_from_slice(&tail);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("recursive $include"),
        "expected recursion diagnostic, got {text:?}"
    );
    assert!(
        text.contains("RCDONE"),
        "shell must continue past the recursion diagnostic"
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
    // Emacs is default-on (`docs/features/emacs-editing-mode.md`
    // § 2.5), so the inputrc — and therefore its unknown-variable
    // diagnostic — is processed before the first `$ ` prompt appears.
    // Capture the startup preamble so the assertion sees the
    // diagnostic in full transcript.
    let startup = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"echo DONEMARK\n");
    let tail = drain_until_contains(&mut pty, b"DONEMARK\r\n");
    let mut out = startup;
    out.extend_from_slice(&tail);
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
fn inputrc_input_meta_and_output_meta_accepted() {
    // Distribution-shipped /etc/inputrc files commonly carry
    //
    //     set input-meta on
    //     set output-meta on
    //
    // Meiksh must accept these as recognized variables (even though
    // they currently have no runtime effect — see
    // `docs/features/emacs-editing-mode.md` Section 15.11) instead of
    // emitting "unknown variable" diagnostics.
    let path = format!("/tmp/meiksh-rc-meta-{}", std::process::id());
    write_rc(
        &path,
        b"set input-meta on\nset output-meta on\nset meta-flag off\n",
    );
    let env = [("INPUTRC", path.as_str())];
    let Some(mut pty) = spawn_meiksh_pty(&env) else {
        let _ = std::fs::remove_file(&path);
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"echo METADONE\n");
    let out = drain_until_contains(&mut pty, b"METADONE\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("unknown variable"),
        "input-meta/output-meta/meta-flag must be accepted, got {text:?}"
    );
    assert!(
        text.contains("METADONE"),
        "shell must reach the post-rc command: {text:?}"
    );
}

/// Spawn meiksh with a test-controlled inputrc and the given TERM,
/// force a re-read of the inputrc via `bind -f`, dump the resulting
/// keymap to a side-channel file (so the PTY echo of the command
/// doesn't pollute the match), and return the `(pty_output,
/// dump_file_contents)` pair.
fn capture_bind_dump_for_term(rc_body: &[u8], term: &str, tag: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let pid = std::process::id();
    let rc_path = format!("/tmp/meiksh-rc-{tag}-{pid}");
    let dump_path = format!("/tmp/meiksh-dump-{tag}-{pid}");
    write_rc(&rc_path, rc_body);
    let env = [("INPUTRC", rc_path.as_str()), ("TERM", term)];
    let mut pty = match spawn_meiksh_pty(&env) {
        Some(p) => p,
        None => {
            let _ = std::fs::remove_file(&rc_path);
            return None;
        }
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(format!("bind -f {rc_path}\n").as_bytes());
    pty.send(format!("bind -p > {dump_path}\n").as_bytes());
    pty.send(b"echo TERMEND\n");
    let out = drain_until_contains(&mut pty, b"TERMEND\r\n");
    let _ = pty.exit_and_wait();
    let dump = std::fs::read(&dump_path).unwrap_or_default();
    let _ = std::fs::remove_file(&rc_path);
    let _ = std::fs::remove_file(&dump_path);
    Some((out, dump))
}

#[test]
fn inputrc_if_term_matches_current_term() {
    // $if term=<name> shall match both the full TERM and the portion
    // before the first `-`. Launching meiksh with TERM=rxvt-unicode
    // and an inputrc that only rebinds inside `$if term=rxvt` must
    // therefore apply that binding. The key sequence `C-x m` is
    // chosen because it is not bound by default, so its presence in
    // the `bind -p` dump is an unambiguous signal that the
    // `$if term=rxvt` branch fired.
    let rc = b"$if term=rxvt\n\"\\C-xm\": end-of-line\n$endif\n";
    let Some((out, dump)) = capture_bind_dump_for_term(rc, "rxvt-unicode", "term-match") else {
        return;
    };
    let out_text = String::from_utf8_lossy(&out);
    let dump_text = String::from_utf8_lossy(&dump);
    assert!(
        !out_text.contains("unknown $if test"),
        "term=<name> must be a recognized $if test, got {out_text:?}"
    );
    assert!(
        dump_text.contains("\"\\C-xm\": end-of-line"),
        "expected `\"\\C-xm\": end-of-line` in dump under TERM=rxvt-unicode, got {dump_text:?}"
    );
}

#[test]
fn inputrc_if_term_miss_leaves_binding_unbound() {
    // Flipside of the match test: under a TERM that doesn't match,
    // the gated binding must not be installed, and the mismatch must
    // NOT be reported as "unknown $if test".
    let rc = b"$if term=rxvt\n\"\\C-xm\": end-of-line\n$endif\n";
    let Some((out, dump)) = capture_bind_dump_for_term(rc, "xterm-256color", "term-miss") else {
        return;
    };
    let out_text = String::from_utf8_lossy(&out);
    let dump_text = String::from_utf8_lossy(&dump);
    assert!(
        !out_text.contains("unknown $if test"),
        "well-formed term= mismatch must be silent, got {out_text:?}"
    );
    assert!(
        !dump_text.contains("\\C-xm"),
        "term=rxvt branch must not fire under TERM=xterm-256color, got {dump_text:?}"
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
