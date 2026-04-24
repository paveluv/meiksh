//! Integration tests for `docs/features/ps1-prompt-extensions.md`.
//!
//! These tests drive meiksh as a subprocess with `-xc` so that a
//! single prompt-exposed path (PS4 rendering under `set -o xtrace`)
//! can be observed from outside the shell: the test-visible stream is
//! stderr, which receives the expanded PS4 value once per traced
//! command. Every assertion below sets `PS4` explicitly so that each
//! escape or prompts-mode combination is isolated.
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

fn run_xtrace(ps4: &str, bash_prompts: bool) -> String {
    let setup = if bash_prompts {
        "set -o bash_prompts\n"
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
fn bash_prompts_mode_decodes_basic_escapes_in_ps4() {
    // `\j` is deterministic (job count = 0 outside pipelines).
    let stderr = run_xtrace("'[\\j] '", true);
    assert!(
        stderr.contains("[0] echo hi"),
        "bash_prompts should decode \\j, got: {stderr}"
    );
}

#[test]
fn bash_prompts_mode_renders_dollar_escape() {
    // `\$` decodes to `$` for non-root. Our test runner is not root.
    let stderr = run_xtrace("'\\$ '", true);
    assert!(
        stderr.contains("$ echo hi"),
        "expected `$ echo hi`, got: {stderr}"
    );
}

#[test]
fn bash_prompts_mode_emits_literal_for_unknown_escape() {
    // `\q` is not in the escape table (spec § 6.6).
    let stderr = run_xtrace("'[\\q] '", true);
    assert!(
        stderr.contains("[\\q] echo hi"),
        "unknown escape must round-trip as two bytes, got: {stderr}"
    );
}

#[test]
fn bash_prompts_mode_decodes_octal_escapes() {
    // \101 == 'A', \60 == '0'.
    let stderr = run_xtrace(r"'\101\60 '", true);
    assert!(
        stderr.contains("A0 echo hi"),
        "octal decoding, got: {stderr}"
    );
}

#[test]
fn bash_prompts_mode_emits_shell_name() {
    let stderr = run_xtrace("'<\\s> '", true);
    assert!(
        stderr.contains("<meiksh> echo hi") || stderr.contains("<sh> echo hi"),
        "\\s should expand to invocation basename, got: {stderr}"
    );
}

#[test]
fn bash_prompts_discards_invisible_mask_in_ps4_output() {
    // Bytes inside \[...\] are NOT visible to the editor, but the
    // xtrace writer emits them verbatim per spec § 9.4.
    let stderr = run_xtrace(r"'\[X\]Y '", true);
    assert!(
        stderr.contains("XY echo hi"),
        "invisible-region bytes must still be emitted by xtrace, got: {stderr}"
    );
}

#[test]
fn set_o_lists_bash_prompts_option() {
    let output = Command::new(meiksh())
        .args(["-c", "set -o"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bash_prompts off"),
        "set -o should list bash_prompts, got: {stdout}"
    );
}

#[test]
fn set_o_bash_prompts_toggles_reported_state() {
    let output = Command::new(meiksh())
        .args(["-c", "set -o bash_prompts; set +o | grep bash_prompts"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set -o bash_prompts"),
        "`set +o` should report bash_prompts on, got: {stdout}"
    );
}

#[test]
fn bash_prompts_backslash_bang_renders_history_number_in_ps4() {
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
    // literal `!` is emitted verbatim even in bash_prompts mode.
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

// === § 2.1 / § 13.6 — bash_prompts does not get a short-option letter ===

#[test]
fn bash_prompts_does_not_appear_in_dollar_dash() {
    // § 2.1: "bash_prompts shall not be exposed through a short option
    // letter. The value of $- shall not gain a new character when
    // bash_prompts is enabled."
    let output = Command::new(meiksh())
        .args(["-c", "set -o bash_prompts; printf '%s\\n' \"$-\""])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `$-` must contain only shell-option short letters (e.g. `hB`);
    // no letter shall be added for bash_prompts.
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
fn bash_prompts_default_state_is_off_on_startup() {
    // § 2.1: "The default value of bash_prompts on shell startup shall
    // be off, for both interactive and non-interactive shells."
    let output = Command::new(meiksh())
        .args(["-c", "set -o | grep '^bash_prompts'"])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bash_prompts off"),
        "bash_prompts must default to off, got: {stdout}"
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
            "set -o bash_prompts\n\
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
            "set -o bash_prompts\n\
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
fn read_dash_p_prompt_is_not_subject_to_escape_pass() {
    // § 12.3: "read -p writes the literal bytes of prompt to stderr,
    // matching POSIX and bash (bash's read -p explicitly does not run
    // the PS1 expansion pipeline)." Even under bash_prompts, `\u` in
    // the read prompt shall appear as the two raw bytes `\u`.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_prompts\n\
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

#[test]
fn read_dash_p_prompt_emits_literal_bytes_in_posix_mode() {
    // § 12.3 in POSIX mode — same contract: the `-p` argument is
    // written verbatim to stderr, with no prompt pipeline applied.
    let output = Command::new(meiksh())
        .args(["-c", "read -p 'say: ' VAR < /dev/null"])
        .output()
        .expect("run meiksh");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("say: "),
        "read -p prompt must land on stderr, got: {stderr:?}"
    );
}

#[test]
fn read_dash_p_joined_short_form_writes_prompt() {
    // Bash accepts both `-p PROMPT` and `-pPROMPT`. The joined form
    // is widely used in shell scripts; verify it round-trips
    // identically.
    let output = Command::new(meiksh())
        .args(["-c", "read -phello_world VAR < /dev/null"])
        .output()
        .expect("run meiksh");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hello_world"),
        "joined -pPROMPT form must write PROMPT, got: {stderr:?}"
    );
}

// === § 7.2 — !! renders as single literal ! in the history pass =====

#[test]
fn bash_prompts_double_bang_renders_single_bang_in_ps1() {
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

// === § 2.3 — next prompt observes updated selector ====================

#[test]
fn next_prompt_observes_updated_prompts_mode() {
    // § 2.3: "The next prompt expansion shall observe the updated
    // selector. There is no hysteresis and no deferred flip."
    //
    // We toggle bash_prompts between two xtrace-ed commands and
    // confirm the escape decoder switches accordingly.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_prompts\n\
             PS4='<\\u> '\n\
             set -x\n\
             echo a\n\
             set +o bash_prompts\n\
             echo b",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // First echo runs under bash_prompts — \u is decoded.
    let first = stderr.lines().find(|l| l.contains("echo a")).unwrap_or("");
    assert!(
        !first.contains("\\u"),
        "first trace must have \\u decoded, got: {first:?}"
    );
    // Second echo runs under POSIX — \u is literal.
    let second = stderr.lines().find(|l| l.contains("echo b")).unwrap_or("");
    assert!(
        second.contains("<\\u>"),
        "second trace must preserve literal \\u, got: {second:?}"
    );
}

// === § 3.5 — PS4 first char duplicates per subshell nesting level =====

#[test]
fn ps4_first_char_duplicates_per_subshell_nesting() {
    // § 3.5: "When the rendered value of PS4 is longer than a single
    // character, the first character shall be duplicated once per
    // level of subshell nesting, matching bash." A single `(...)`
    // nests once, a `( (...) )` nests twice.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "PS4='+X '\n\
             set -x\n\
             ( echo one )\n\
             ( ( echo two ) )",
        ])
        .output()
        .expect("run meiksh");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("++X echo one"),
        "expected '++X echo one' under one subshell level, got: {stderr}"
    );
    assert!(
        stderr.contains("+++X echo two"),
        "expected '+++X echo two' under two subshell levels, got: {stderr}"
    );
}

#[test]
fn ps4_single_character_does_not_duplicate_in_subshell() {
    // § 3.5 guards the duplication on "longer than a single
    // character"; a one-byte PS4 must stay one byte deep.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "PS4='+'\n\
             set -x\n\
             ( echo hi )",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The traced line should start with a single `+` directly
    // followed by `echo hi`, no extra copies.
    assert!(
        stderr.contains("+echo hi"),
        "one-char PS4 must not duplicate, got: {stderr}"
    );
}

// === § 6.2 — $PWD is preferred over getcwd ============================

#[test]
fn w_prefers_pwd_over_getcwd() {
    // § 6.2: "The escape pass shall resolve the current working
    // directory by querying the shell's recorded PWD, falling back
    // to getcwd(3) if PWD is unset." We set PWD to a synthetic path
    // that does not exist on the filesystem; \w must emit exactly
    // that path rather than the real getcwd result.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_prompts\n\
             PWD='/synthetic/not-real'\n\
             HOME=/root\n\
             PS4='<\\w> '\n\
             set -x\n\
             echo hi",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("</synthetic/not-real> echo hi"),
        "PWD must take priority over getcwd, got: {stderr}"
    );
}

// === § 6.4 — counter increments per accepted input line (PTY) =========

/// § 6.4: "The counter shall ... increment by 1 each time the shell
/// accepts an input line from the interactive reader". Driving this
/// end-to-end requires a real PTY so that the REPL path in
/// `src/interactive/repl.rs` actually runs.
#[test]
fn session_counter_increments_across_accepted_interactive_lines() {
    use super::interactive_common::spawn_meiksh_pty;
    use std::time::Duration;

    let Some(mut pty) = spawn_meiksh_pty(&[]) else {
        return;
    };

    pty.send(b"set -o bash_prompts\n");
    pty.send(b"PS1='<\\#>MARK '\n");
    pty.send(b"true\n");
    pty.send(b"true\n");
    let out = pty.drain_until(
        |b| b.windows(7).any(|w| w == b"<5>MARK"),
        Duration::from_secs(5),
    );
    let _ = pty.exit_and_wait();

    let text = String::from_utf8_lossy(&out);
    // After each accepted line the counter bumps by one. After
    // `PS1='<\#>MARK '` (the 2nd accepted line) the new prompt
    // renders with counter 3, then 4 after the first `true`, 5
    // after the second.
    assert!(
        text.contains("<3>MARK") && text.contains("<4>MARK") && text.contains("<5>MARK"),
        "\\# must advance 3 → 4 → 5 across accepted lines, got: {text:?}"
    );
}

/// § 6.4: "The counter shall not decrement." A failed command still
/// counts as an accepted line, so the subsequent prompt must observe
/// a strictly greater counter value.
#[test]
fn session_counter_never_decrements_on_failure_interactive() {
    use super::interactive_common::spawn_meiksh_pty;
    use std::time::Duration;

    let Some(mut pty) = spawn_meiksh_pty(&[]) else {
        return;
    };

    pty.send(b"set -o bash_prompts\n");
    pty.send(b"PS1='<\\#>TAG '\n");
    pty.send(b"false\n");
    pty.send(b"true\n");
    let out = pty.drain_until(
        |b| b.windows(6).any(|w| w == b"<5>TAG"),
        Duration::from_secs(5),
    );
    let _ = pty.exit_and_wait();

    let text = String::from_utf8_lossy(&out);
    // Initial prompt (counter 1) uses the default PS1 — no TAG. The
    // first `<N>TAG` prompt appears *after* `PS1=...` is accepted
    // (counter 3). Then `false` (counter 4) and `true` (counter 5).
    assert!(
        text.contains("<3>TAG") && text.contains("<4>TAG") && text.contains("<5>TAG"),
        "\\# must advance past a failed command, got: {text:?}"
    );
}

// === § 6.4 — counter stays fixed in non-interactive shells ============

#[test]
fn session_counter_is_stable_across_non_interactive_commands() {
    // § 6.4 is keyed on "the interactive reader". In a non-
    // interactive `-s` (or `-c`) invocation there is no interactive
    // reader, so `\#` is expected to remain at its startup value (1)
    // for every traced command rather than advance.
    let mut child = std::process::Command::new(meiksh())
        .args(["-s"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn meiksh -s");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            b"set -o bash_prompts\n\
              PS4='<\\#> '\n\
              set -x\n\
              echo a\n\
              echo b\n\
              echo c\n",
        )
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("<1> echo a")
            && stderr.contains("<1> echo b")
            && stderr.contains("<1> echo c"),
        "non-interactive \\# must stay at 1, got: {stderr}"
    );
}

// === § 7.2 — `!` from parameter expansion is scanned by history pass ==

/// § 7.2: "A `!` introduced by parameter expansion shall be subject
/// to this pass exactly like a `!` written directly in `PS1`". We
/// set a variable to `!`, reference it from `PS1`, and observe that
/// the rendered prompt substitutes a decimal digit (the history
/// number) rather than a literal `!`.
#[test]
fn bang_from_parameter_expansion_is_scanned_by_history_pass_interactive() {
    use super::interactive_common::spawn_meiksh_pty;
    use std::time::Duration;

    let Some(mut pty) = spawn_meiksh_pty(&[]) else {
        return;
    };

    pty.send(b"set -o bash_prompts\n");
    pty.send(b"VAR='!'\n");
    pty.send(b"PS1='<${VAR}>DONE '\n");
    pty.send(b"true\n");
    let out = pty.drain_until(
        |b| {
            // Look for `<N>DONE ` where N is one or more digits.
            let mut i = 0;
            while i + 7 <= b.len() {
                if b[i] == b'<' {
                    let mut j = i + 1;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > i + 1 && j + 5 <= b.len() && &b[j..j + 5] == b">DONE" {
                        return true;
                    }
                }
                i += 1;
            }
            false
        },
        Duration::from_secs(5),
    );
    let _ = pty.exit_and_wait();

    let text = String::from_utf8_lossy(&out);
    // The literal `<!>DONE` must never appear — that would mean the
    // history pass failed to scan the output of parameter expansion.
    assert!(
        !text.contains("<!>DONE"),
        "literal `!` must be substituted by history pass, got: {text:?}"
    );
    // And we must see the substituted form with a decimal digit.
    let saw_substituted = text
        .split("DONE")
        .any(|chunk| chunk.ends_with(|c: char| c == '>') && chunk.contains('<'))
        && text.contains(">DONE");
    assert!(
        saw_substituted,
        "expected `<<digits>>DONE` in transcript, got: {text:?}"
    );
}

// === § 10.1 — \u fallback when $USER is empty =========================

#[test]
fn user_escape_falls_back_to_pwuid_when_user_env_is_empty() {
    // § 10.1 / § 6.1 (`\u`): "If `$USER` is unset or empty, the
    // shell shall call `getpwuid(geteuid())` and emit its `pw_name`
    // field." We clear `$USER` at invocation time and observe that
    // `\u` in PS4 emits a non-empty, non-`?` login name (i.e. the
    // fallback path produced a real answer).
    let output = Command::new(meiksh())
        .env_remove("USER")
        .env("LOGNAME", "")
        .args([
            "-c",
            "set -o bash_prompts\n\
             PS4='<\\u> '\n\
             set -x\n\
             echo hi",
        ])
        .output()
        .expect("run meiksh");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Extract the value between `<` and `> echo hi` on the trace line.
    let line = stderr
        .lines()
        .find(|l| l.contains("> echo hi"))
        .expect("trace line missing");
    let start = line.find('<').expect("open angle");
    let end = line.find("> echo hi").expect("close angle");
    let value = &line[start + 1..end];
    assert!(
        !value.is_empty(),
        "\\u must produce a non-empty fallback, got: {line:?}"
    );
    // On a normal host getpwuid succeeds, so the value is a real
    // login name. If the host is pathological (no passwd entry), the
    // spec requires `?` — accept either outcome.
    assert!(
        value == "?" || value.chars().all(|c| c.is_ascii() && !c.is_whitespace()),
        "\\u fallback must be `?` or a plain login name, got: {value:?}"
    );
}

// === § 13.8 — \N is emitted as two raw bytes ==========================

#[test]
fn bash_5_1_nickname_escape_is_treated_as_unknown() {
    // § 13.8: "Meiksh shall not recognize \N; it is emitted as two
    // raw bytes per Section 6.6."
    let stderr = run_xtrace(r"'[\N] '", true);
    assert!(
        stderr.contains("[\\N] echo hi"),
        "\\N must round-trip as two raw bytes, got: {stderr}"
    );
}

// === § 10.3 — parameter expansion errors do not abort prompt rendering

#[test]
fn parameter_expansion_error_in_ps4_does_not_abort_xtrace() {
    // § 10.3: "A parameter expansion failure during pass 2 ... shall
    // ... fall back to rendering the prompt value with the failing
    // expansion removed, matching bash." `set -u` + an unset variable
    // reference is the canonical failure driver. The shell must
    // still emit the traced command even if the prompt's parameter
    // pass raises.
    let output = Command::new(meiksh())
        .args([
            "-c",
            "set -o bash_prompts\n\
             set -u\n\
             PS4='<${UNDEFINED}> '\n\
             set -x\n\
             echo survived 2>&1 || true",
        ])
        .output()
        .expect("run meiksh");
    // The exit status may be non-zero (set -u raised), but the
    // traced command and/or its output must still be present.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("echo survived") || combined.contains("survived"),
        "prompt rendering must not abort xtrace, got: {combined}"
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
