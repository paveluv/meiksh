//! Integration tests for `docs/features/ps1-prompt-extensions.md`.
//!
//! These tests drive meiksh as a subprocess with `-xc` so that a
//! single prompt-exposed path (PS4 rendering under `set -o xtrace`)
//! can be observed from outside the shell: the test-visible stream is
//! stderr, which receives the expanded PS4 value once per traced
//! command. Every assertion below sets `PS4` explicitly so that each
//! escape or compat-mode combination is isolated.
//!
//! The interactive (`PS1` / `PS2`) paths exercise the same decoder
//! through `interactive::prompt::expand_full_prompt` and are covered
//! by the unit tests colocated with the decoder in
//! `src/interactive/prompt_expand.rs`. The PTY-driven tests for
//! `\[...\]` cursor placement live in
//! `tests/integration/interactive_common/` and are exercised by the
//! emacs/vi suites.

use super::common::*;
use std::process::Command;

fn run_xtrace(ps4: &str, bash_compat: bool) -> String {
    let setup = if bash_compat {
        "set -o bash_compat\n"
    } else {
        ""
    };
    let script = format!("{setup}PS4={ps4}\nset -x\necho hi");
    let output = Command::new(meiksh())
        .args(["-c", &script])
        .output()
        .expect("run meiksh");
    assert!(
        output.status.success(),
        "meiksh exited unexpectedly: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn posix_mode_does_not_decode_backslash_escapes_in_ps4() {
    // Default is POSIX. Spec § 4: backslashes are literal bytes.
    let stderr = run_xtrace("'[\\u] '", false);
    assert!(
        stderr.contains("[\\u] echo hi"),
        "POSIX mode should emit literal \\u, got: {stderr}"
    );
}

#[test]
fn bash_compat_mode_decodes_basic_escapes_in_ps4() {
    // `\j` is deterministic (job count = 0 outside pipelines).
    let stderr = run_xtrace("'[\\j] '", true);
    assert!(
        stderr.contains("[0] echo hi"),
        "bash_compat should decode \\j, got: {stderr}"
    );
}

#[test]
fn bash_compat_mode_renders_dollar_escape() {
    // `\$` decodes to `$` for non-root. Our test runner is not root.
    let stderr = run_xtrace("'\\$ '", true);
    assert!(
        stderr.contains("$ echo hi"),
        "expected `$ echo hi`, got: {stderr}"
    );
}

#[test]
fn bash_compat_mode_emits_literal_for_unknown_escape() {
    // `\q` is not in the escape table (spec § 6.6).
    let stderr = run_xtrace("'[\\q] '", true);
    assert!(
        stderr.contains("[\\q] echo hi"),
        "unknown escape must round-trip as two bytes, got: {stderr}"
    );
}

#[test]
fn bash_compat_mode_decodes_octal_escapes() {
    // \101 == 'A', \60 == '0'.
    let stderr = run_xtrace(r"'\101\60 '", true);
    assert!(
        stderr.contains("A0 echo hi"),
        "octal decoding, got: {stderr}"
    );
}

#[test]
fn bash_compat_mode_emits_shell_name() {
    let stderr = run_xtrace("'<\\s> '", true);
    assert!(
        stderr.contains("<meiksh> echo hi") || stderr.contains("<sh> echo hi"),
        "\\s should expand to invocation basename, got: {stderr}"
    );
}

#[test]
fn bash_compat_discards_invisible_mask_in_ps4_output() {
    // Bytes inside \[...\] are NOT visible to the editor, but the
    // xtrace writer emits them verbatim per spec § 9.4.
    let stderr = run_xtrace(r"'\[X\]Y '", true);
    assert!(
        stderr.contains("XY echo hi"),
        "invisible-region bytes must still be emitted by xtrace, got: {stderr}"
    );
}

#[test]
fn set_o_lists_bash_compat_option() {
    let output = Command::new(meiksh())
        .args(["-c", "set -o"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bash_compat off"),
        "set -o should list bash_compat, got: {stdout}"
    );
}

#[test]
fn set_o_bash_compat_toggles_reported_state() {
    let output = Command::new(meiksh())
        .args(["-c", "set -o bash_compat; set +o | grep bash_compat"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set -o bash_compat"),
        "`set +o` should report bash_compat on, got: {stdout}"
    );
}

#[test]
fn bash_compat_backslash_bang_renders_history_number_in_ps4() {
    // `\!` runs through the escape pass (which runs for PS4), not
    // through the history pass (which doesn't). The history number
    // in a fresh -c invocation is 1.
    let stderr = run_xtrace(r"'<\! '", true);
    assert!(
        stderr.contains("<1 echo hi"),
        "expected history number 1, got: {stderr}"
    );
}

#[test]
fn ps4_skips_literal_bang_history_substitution() {
    // Per spec § 3.1 PS4 does NOT run the history pass, so a
    // literal `!` is emitted verbatim even in bash_compat mode.
    let stderr = run_xtrace("'<! '", true);
    assert!(
        stderr.contains("<! echo hi"),
        "PS4 must not substitute literal !, got: {stderr}"
    );
}

#[test]
fn posix_mode_keeps_literal_bang_in_prompt() {
    let stderr = run_xtrace("'<! '", false);
    assert!(
        stderr.contains("<! echo hi"),
        "POSIX mode preserves literal !, got: {stderr}"
    );
}
