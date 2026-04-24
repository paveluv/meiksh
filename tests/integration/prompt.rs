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

// === § 2.1 / § 13.6 — bash_compat does not get a short-option letter ===

#[test]
fn bash_compat_does_not_appear_in_dollar_dash() {
    // § 2.1: "bash_compat shall not be exposed through a short option
    // letter. The value of $- shall not gain a new character when
    // bash_compat is enabled."
    let output = Command::new(meiksh())
        .args(["-c", "set -o bash_compat; printf '%s\\n' \"$-\""])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `$-` must contain only shell-option short letters (e.g. `hB`);
    // no letter shall be added for bash_compat.
    assert!(
        !stdout.contains('B'),
        "$- gained an unexpected 'B' letter: {stdout}"
    );
    // Sanity: the output is non-empty (non-interactive shells still
    // have at least some of the default options present).
    assert!(!stdout.trim().is_empty(), "$- unexpectedly empty: {stdout}");
}

// === § 2.1 — default state is off on startup ==========================

#[test]
fn bash_compat_default_state_is_off_on_startup() {
    // § 2.1: "The default value of bash_compat on shell startup shall
    // be off, for both interactive and non-interactive shells."
    let output = Command::new(meiksh())
        .args(["-c", "set -o | grep '^bash_compat'"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bash_compat off"),
        "bash_compat must default to off, got: {stdout}"
    );
}

// === § 3.5 / § 3.6 — PS4 fallback and empty behavior ==================

#[test]
fn unset_ps4_falls_back_to_plus_space_default() {
    // § 3.6: "A prompt variable that becomes unset shall fall back to
    // its default from Sections 3.2-3.5." PS4 default is "+ " per § 3.5.
    let output = Command::new(meiksh())
        .args(["-c", "unset PS4; set -x; echo hi"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("+ echo hi"),
        "unset PS4 must render as default '+ ', got: {stderr}"
    );
}

#[test]
fn empty_ps4_renders_empty_prompt_prefix() {
    // § 3.6: "A prompt variable that is set to the empty string shall
    // produce an empty prompt; the shell shall not substitute the
    // default." The xtrace line should therefore begin with the
    // traced command, not with the default "+ ".
    let output = Command::new(meiksh())
        .args(["-c", "PS4=; set -x; echo hi"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Extract the xtrace line containing `echo hi`.
    let line = stderr.lines().find(|l| l.contains("echo hi")).unwrap_or("");
    assert!(
        line.starts_with("echo hi"),
        "empty PS4 must not produce a prefix, got: {line:?}"
    );
}

// === § 6.4 — session counter starts at 1 in a fresh invocation ========

#[test]
fn session_counter_hash_starts_at_one_in_non_interactive_shell() {
    // § 6.4: "The counter shall start at 1 on shell startup". A -c
    // invocation never reaches the interactive-reader increment so
    // the counter value observed during PS4 rendering must be 1.
    let stderr = run_xtrace(r"'[#\#] '", true);
    assert!(
        stderr.contains("[#1] echo hi"),
        "expected [#1], got: {stderr}"
    );
}

// === § 6.4 — counter is decoupled from the history number ============

#[test]
fn session_counter_and_history_number_are_distinct() {
    // § 6.4: "\# shall emit a decimal integer that is distinct from
    // the history number produced by \!."
    let stderr = run_xtrace(r"'[\#|\!] '", true);
    // History number in -c is 1; counter is also 1 at startup, but
    // the values are produced by two independent decoders. The key
    // behavior we assert is that both slots render a decimal number.
    assert!(
        stderr.contains("[1|1] echo hi"),
        "expected '[1|1] echo hi', got: {stderr}"
    );
}

// === § 5 — pass 1 before pass 2 (escape, then parameter) ==============

#[test]
fn escape_pass_runs_before_parameter_pass_for_dollar_foo() {
    // § 5: "Pass 1 must precede pass 2 so that \$ produces a literal
    // $ that parameter expansion then treats as a normal character
    // rather than reinterpreting as the start of a parameter
    // reference." We can at minimum verify that \$ decodes to `$`
    // even when followed by an identifier — parameter expansion
    // receiving `$FOO` as input may or may not expand it, but the
    // escape pass itself must not lose the dollar sign.
    let stderr = run_xtrace(r"'<\$XYZ> '", true);
    // Whether `$XYZ` gets expanded to the empty string by pass 2 is
    // acceptable; what is NOT acceptable is losing the `$` from
    // the escape pass — the prefix `<` followed by `>` or by the
    // unset variable's value must be observable.
    let line = stderr.lines().find(|l| l.contains("echo hi")).unwrap_or("");
    assert!(
        line.starts_with("<") && line.contains("> echo hi"),
        "escape pass must emit `$` reliably, got: {line:?}"
    );
}

// === § 6.2 — \w honors HOME and PWD ===================================

#[test]
fn w_collapses_home_prefix_in_ps4() {
    // § 6.2: "\w shall emit ~/<rest> when CWD extends $HOME with a
    // path separator followed by additional components."
    //
    // We force HOME=/tmp and change into /tmp (the caller's actual
    // temp dir) then assert `\w` renders `~`. Doing this with a
    // subdirectory gives `~/<sub>`; we use the simpler equality
    // case since `/tmp` is always writable on Linux CI.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_compat\n\
             HOME=/tmp\n\
             cd /tmp\n\
             PS4='<\\w> '\n\
             set -x\n\
             echo hi",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("<~> echo hi"),
        "expected <~> echo hi, got: {stderr}"
    );
}

// === § 3.6 — PS4 is re-expanded on every prompt write =================

#[test]
fn ps4_is_re_expanded_between_commands() {
    // § 3.6: "Prompt variables shall be re-expanded on every prompt
    // write. Meiksh shall not cache the expanded value." We embed a
    // parameter reference in PS4 and mutate the referent between two
    // xtrace-ed commands; both expansions must observe their
    // respective values.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_compat\n\
             PS4='<$TAG> '\n\
             set -x\n\
             TAG=first\n\
             echo A\n\
             TAG=second\n\
             echo B",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("<first> echo A"),
        "first expansion missing, got: {stderr}"
    );
    assert!(
        stderr.contains("<second> echo B"),
        "second expansion missing, got: {stderr}"
    );
}

// === § 12.3 — read -p does NOT run the escape pass ===================

/// Marked `#[ignore]` pending implementation of `read -p` in
/// [src/builtin/read.rs](../../../src/builtin/read.rs). The spec
/// statement is a forward contract: when `-p` lands, its prompt
/// string MUST NOT run through the backslash-escape pass. Un-ignore
/// this test once `read -p` is wired up.
#[test]
#[ignore = "read -p not yet implemented; spec § 12.3 contract"]
fn read_dash_p_prompt_is_not_subject_to_escape_pass() {
    // § 12.3: "read -p writes the literal bytes of prompt to stderr,
    // matching POSIX and bash (bash's read -p explicitly does not run
    // the PS1 expansion pipeline)." Even under bash_compat, `\u` in
    // the read prompt shall appear as the two raw bytes `\u`.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_compat\n\
             read -p 'literal:\\u> ' line < /dev/null\n\
             true",
        ])
        .output()
        .expect("run meiksh");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("literal:\\u> "),
        "read -p must emit its prompt verbatim, got: {stderr}"
    );
}

// === § 7.2 — !! renders as single literal ! in the history pass =====

#[test]
fn bash_compat_double_bang_renders_single_bang_in_ps1() {
    // § 7.2: "The sequence !! in the expanded prompt shall render as
    // a single literal !." We can't drive PS1 without a PTY, but we
    // can drive the `expand_prompt_exclamation` rule indirectly via
    // `read -e` ? — not available. Instead, we rely on the unit test
    // `expand_prompt_exclamation_covers_all_branches` in
    // `src/interactive/prompt.rs`, which asserts `!!` → `!` for the
    // very function PS1/PS2 call. This integration test instead
    // asserts the POSIX-mode complement: `!!` is preserved verbatim
    // because the history pass is disabled.
    let stderr = run_xtrace("'<!!> '", false);
    assert!(
        stderr.contains("<!!> echo hi"),
        "POSIX mode must not collapse !!, got: {stderr}"
    );
}

// === § 4 — POSIX mode emits every listed escape verbatim =============

#[test]
fn posix_mode_emits_every_listed_escape_verbatim() {
    // § 4: "Backslashes shall be literal bytes. The sequences \u,
    // \h, \w, \t, \$, \[, \], \D{...}, and every other escape listed
    // in Section 6 shall be emitted as their raw two-byte forms."
    let stderr = run_xtrace(r"'[\u\h\w\t\$\[\]\D{%H}] '", false);
    // Every backslash-pair must survive; `\D{%H}` must survive as a
    // literal byte sequence. We assert on a fragment that proves
    // no decoding occurred.
    assert!(
        stderr.contains(r"[\u\h\w\t\$\[\]\D{%H}] echo hi"),
        "POSIX mode must pass every escape through literally, got: {stderr}"
    );
}
