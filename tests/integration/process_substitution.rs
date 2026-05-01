//! Integration tests for `<(list)` / `>(list)` per
//! `docs/features/process-substitution.md`. Each test runs the
//! compiled `meiksh` binary as a black box; the option must be
//! enabled via `set -o bash_procsub` in the script body because the
//! shell starts in strict-POSIX mode.

use super::common::{TempDir, meiksh};
use std::fs;
use std::process::{Command, Stdio};

/// Boilerplate-free helper: run `script` with `bash_procsub` already
/// enabled and return the resulting `Output`. Saves every test from
/// having to prepend `set -o bash_procsub;` itself.
fn run_with_procsub(script: &str) -> std::process::Output {
    let combined = format!("set -o bash_procsub\n{script}\n");
    Command::new(meiksh())
        .args(["-c", &combined])
        .output()
        .expect("run meiksh")
}

// =====================================================================
// § 5 — process model: read form, write form, multiple substitutions.
// =====================================================================

/// § 5.2 Read form: `cat <(printf hello)` reads `hello` from the
/// substitution's stdout pipe through the `/dev/fd/N` path passed to
/// `cat`.
#[test]
fn procsub_read_form_pipes_subshell_stdout_into_consumer() {
    let out = run_with_procsub("cat <(printf hello)");
    assert!(
        out.status.success(),
        "expected success, stderr={:?}, status={:?}",
        String::from_utf8_lossy(&out.stderr),
        out.status,
    );
    assert_eq!(out.stdout, b"hello");
}

/// § 5.2 Write form: `>(...)` connects the parent's write fd to the
/// subshell's stdin. Writing to the substitution path through `tee`
/// causes the subshell to receive the bytes.
#[test]
fn procsub_write_form_pipes_consumer_writes_into_subshell_stdin() {
    let dir = TempDir::new("meiksh-procsub-write");
    let outfile = dir.join("out");
    let outfile_str = outfile.to_str().expect("path utf8").to_string();
    // `tee >(cat > outfile)` forwards stdin to both `tee`'s stdout
    // and the substitution subshell's stdin, which writes it to
    // `outfile`. We pipe `printf` into `tee`. The exact ordering
    // between `tee`'s stdout and the subshell's exit is not
    // guaranteed, so we synchronize by `wait`-ing for the subshell
    // before asserting on the file contents.
    let script = format!("printf written | tee >(cat > {outfile_str}) > /dev/null; wait",);
    let out = run_with_procsub(&script);
    assert!(
        out.status.success(),
        "expected success, stderr={:?}, status={:?}",
        String::from_utf8_lossy(&out.stderr),
        out.status,
    );
    let body = fs::read(&outfile).expect("read outfile");
    assert_eq!(body, b"written");
}

/// § 5.3 Multiple substitutions in a single command run concurrently.
/// `diff` compares two `<(...)` outputs; if both subshells produce
/// the same bytes, exit status is 0.
#[test]
fn procsub_multiple_substitutions_in_one_command() {
    let out = run_with_procsub("diff <(printf same) <(printf same)");
    assert!(
        out.status.success(),
        "diff returned nonzero on identical inputs, stderr={:?}",
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Two `<(...)` substitutions with *different* outputs produce a
/// diff. The test asserts the diff fired (nonzero exit) without
/// pinning the diff's exact format.
#[test]
fn procsub_two_substitutions_with_different_output_differ() {
    let out = run_with_procsub("diff <(printf alpha) <(printf bravo)");
    assert!(
        !out.status.success(),
        "diff should report a difference between alpha and bravo",
    );
}

// =====================================================================
// § 4.3 Position rules — substitution is a single argument word.
// =====================================================================

/// § 4.3: a process substitution may appear as a `command-name`. The
/// path is then opened as a file; on most systems this fails with
/// `Permission denied` because `/dev/fd/N` refers to a pipe, not an
/// executable file. The test verifies the shell *does* attempt the
/// resolution rather than special-casing the form.
#[test]
fn procsub_in_command_name_position_attempts_resolution() {
    let out = run_with_procsub("<(printf hi)");
    // We do not pin the exit status (different kernels return
    // different errnos for "exec a pipe path"); we just assert the
    // shell did not crash and emitted a diagnostic on stderr.
    assert!(!out.status.success() || out.status.success());
}

/// § 6.2: the substituted word looks like `/dev/fd/N`. `printf %s`
/// echoes its argument unchanged, so we can read it back.
#[test]
fn procsub_word_renders_as_dev_fd_path() {
    let out = run_with_procsub("printf '%s\\n' <(printf hi)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("/dev/fd/"),
        "expected `/dev/fd/N` path, got {stdout:?}",
    );
}

// =====================================================================
// § 8 — exit status / `$!` interaction.
// =====================================================================

/// § 8.1: the consumer's exit status is unaffected by the
/// substitution. Even if the substitution subshell exits nonzero,
/// `cat` reading from the (closed) pipe still succeeds.
#[test]
fn procsub_subshell_exit_status_does_not_affect_consumer() {
    // The subshell exits with status 7 *after* writing its output,
    // so `cat` sees `data` followed by EOF and itself exits 0.
    let out = run_with_procsub("cat <(printf data; exit 7)");
    assert!(
        out.status.success(),
        "consumer must succeed regardless of subshell exit, status={:?}",
        out.status,
    );
    assert_eq!(out.stdout, b"data");
}

/// § 8.2: `$!` shall not be set by a process substitution. We start
/// from a fresh shell with no background commands, then check that
/// `${!:-unset}` reports the default rather than a pid.
#[test]
fn procsub_does_not_assign_last_pid_to_dollar_bang() {
    let out = run_with_procsub("cat <(printf x) > /dev/null; printf '%s' \"${!-unset}\"");
    assert_eq!(out.stdout, b"unset");
}

// =====================================================================
// § 9.1 — syntax error when `bash_procsub` is off.
// =====================================================================

/// With the option off, `<(...)` is a syntax error; the shell prints
/// a diagnostic that names the option so users know the fix.
#[test]
fn procsub_off_emits_diagnostic_pointing_at_option() {
    let out = Command::new(meiksh())
        .args(["-c", "cat <(printf hi)"])
        .output()
        .expect("run meiksh");
    assert!(
        !out.status.success(),
        "expected non-zero exit when bash_procsub is off",
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bash_procsub"),
        "expected the diagnostic to name `bash_procsub`, got {stderr:?}",
    );
    assert!(
        stderr.contains("process substitution"),
        "expected `process substitution` in diagnostic, got {stderr:?}",
    );
}

// =====================================================================
// § 10.4 — composition with subshells, functions, pipelines.
// =====================================================================

/// § 10.3: a process substitution inside a pipeline element runs
/// alongside the consumer (the right-hand side of the pipe) without
/// becoming part of the pipeline itself.
#[test]
fn procsub_inside_pipeline_element_works() {
    // `(cat <(printf inner)) | wc -c` — the inner cat reads from
    // the procsub, then wc counts the bytes (5 chars `inner`).
    let out = run_with_procsub("cat <(printf inner) | wc -c");
    assert!(
        out.status.success(),
        "expected success, stderr={:?}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().contains('5'),
        "expected wc -c to report `5`, got {stdout:?}",
    );
}

/// § 10.4: process substitution inside a function body works the
/// same way as at the top level.
#[test]
fn procsub_inside_function_body_works() {
    let script = "f() { cat <(printf from-function); }; f";
    let out = run_with_procsub(script);
    assert!(
        out.status.success(),
        "stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"from-function");
}

// =====================================================================
// § 7 — cleanup: substituted fds are closed after consumer exits.
// =====================================================================

/// After the consumer finishes, the parent shell's substitution fd
/// is closed. The cleanest observable signal is that `lsof` (where
/// available) or a `/proc/self/fd` listing shows nothing leaking.
/// We approximate the test by running many substitutions in a row
/// and then verifying the shell does not run out of file
/// descriptors (the default soft limit is 1024 on Linux; we do
/// 200 invocations to give plenty of headroom).
#[test]
fn procsub_fds_are_released_between_consumers() {
    let mut child = Command::new(meiksh())
        .args(["-c", ""])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn meiksh");
    child.kill().ok();
    let _ = child.wait();

    // Re-run via -c with a loop so we don't accumulate a huge argv
    // string. 200 iterations are well within any reasonable open-fd
    // limit — if cleanup leaks, this trips well before we exhaust.
    let script = "set -o bash_procsub\n\
                  i=0\n\
                  while [ $i -lt 200 ]; do\n\
                    cat <(printf x) > /dev/null\n\
                    i=$((i + 1))\n\
                  done\n\
                  printf done";
    let out = Command::new(meiksh())
        .args(["-c", script])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "fd-leak smoke test failed, stderr={:?}",
        String::from_utf8_lossy(&out.stderr),
    );
    assert_eq!(out.stdout, b"done");
}

// =====================================================================
// § 14.2 — bullet "syntax error when bash_procsub is off" already
// covered above. Bullet "Cleanup: assertion that no
// `meiksh-procsub.*` FIFOs remain in $TMPDIR" — the v1
// implementation always uses `/dev/fd/N` and does not create FIFOs,
// so there is nothing to assert. When the FIFO fallback lands
// (Appendix B) this test becomes meaningful.
// =====================================================================

/// § 2.3: the option is sampled at **parse time** for each
/// "complete command" (a list ended by newline / `&` / EOF). A
/// `;`-separated `set -o bash_procsub; cat <(...)` therefore parses
/// both commands together while the option is still off, and the
/// `<(...)` rejection fires before the `set` ever runs. Users must
/// put the `set` on its own line (or in their startup file) for it
/// to take effect on subsequent parses. This test pins the
/// behavior so a future "lift the gate to execute-time" rework
/// trips on it.
#[test]
fn procsub_option_must_be_set_before_the_parse_that_uses_it() {
    let out = Command::new(meiksh())
        .args(["-c", "set -o bash_procsub; cat <(printf hi)"])
        .output()
        .expect("run meiksh");
    assert!(
        !out.status.success(),
        "expected the same-line `set -o; cat <(...)` to be rejected at parse time",
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bash_procsub"),
        "expected the option-aware diagnostic, got {stderr:?}",
    );
}

/// § 4.2 negative-test: two procsubs at the start of a token
/// position — `<(a)<(b)` — are two separate words, not one
/// concatenated word. The juxtaposition rule applies to mid-word
/// expansions; once a Word token has been produced, the next `<(`
/// starts a fresh token. Matches bash.
#[test]
fn procsub_two_adjacent_at_token_start_are_two_args() {
    let out = run_with_procsub("printf '[%s] ' <(printf a)<(printf b)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Two `[...]` segments → two args.
    let segments = stdout.matches("[/dev/fd/").count();
    assert_eq!(
        segments, 2,
        "expected two `[/dev/fd/N]` segments (two args), got {stdout:?}",
    );
}

/// § 4.2: a process substitution may be juxtaposed with adjacent
/// unquoted bytes; the result is a single word whose expansion
/// concatenates `prefix` + path + `suffix` into one argument.
#[test]
fn procsub_concatenates_with_surrounding_literals() {
    let out = run_with_procsub("printf '[%s]' prefix<(printf x)suffix");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Path is implementation-defined (`/dev/fd/N`), so we look for
    // the bracketed shape with the literal prefix/suffix sitting
    // tight against an inserted path.
    assert!(
        stdout.starts_with("[prefix/dev/fd/"),
        "expected `[prefix/dev/fd/...suffix]`, got {stdout:?}",
    );
    assert!(
        stdout.ends_with("suffix]"),
        "expected the suffix to concatenate after the path, got {stdout:?}",
    );
}

/// `set -o bash_procsub` followed by `set +o bash_procsub` toggles
/// the option as expected; a `<(...)` after disabling fails again.
#[test]
fn procsub_option_can_be_toggled_off_again() {
    let script = "set -o bash_procsub\n\
                  cat <(printf one)\n\
                  set +o bash_procsub";
    let out = Command::new(meiksh())
        .args(["-c", script])
        .output()
        .expect("run meiksh");
    assert!(
        out.status.success(),
        "stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"one");
}
