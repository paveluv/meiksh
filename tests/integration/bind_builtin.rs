//! Integration tests for the `bind` builtin.
//!
//! These tests drive `meiksh -i` through the shared PTY harness and
//! cover the option matrix enumerated by `docs/features/emacs-editing-
//! mode.md` § 10 and the implementation plan step `d5-bind-tests`:
//!
//! * `bind -l`        — lists bindable function names.
//! * `bind -p`        — dumps bindings in inputrc format.
//! * `bind -r`        — removes a binding.
//! * `bind -f FILE`   — loads bindings from an inputrc file.
//! * `bind -x '...'`  — binds a key sequence to a shell command.
//! * `bind <line>`    — single-argument form applies one inputrc line.
//! * Unknown options  — exit status 2.
//!
//! To avoid cargo's parallel test harness stressing the host's PTY
//! budget, every scenario is exercised through a single long-running
//! `meiksh -i` session whose output is inspected for unique sentinels.

use super::interactive_common::{PtyChild, spawn_meiksh_pty};
use std::time::Duration;

fn spawn_or_skip() -> Option<PtyChild> {
    spawn_meiksh_pty(&[])
}

fn drain_until_contains(pty: &mut PtyChild, needle: &[u8]) -> Vec<u8> {
    let needle = needle.to_vec();
    pty.drain_until(
        move |b| b.windows(needle.len()).any(|w| w == needle.as_slice()),
        Duration::from_secs(5),
    )
}

/// Scan `bytes` for the first well-formed `<prefix><digits><terminator>`
/// occurrence and return the signed integer between `<prefix>` and
/// `<terminator>`. Returns `None` when no complete occurrence is
/// present.
///
/// The prefix and terminator together are what makes status tagging
/// echo-safe: the terminal echoes the raw command text (which
/// contains `>>TAG=%d<`), so any drain that matches only on the
/// prefix `>>TAG=` would race the actual `printf` output. Requiring
/// that `<prefix>` be followed by ASCII digits and then `<terminator>`
/// is a pattern the echoed literal `%d` can never satisfy.
fn match_status_tag(bytes: &[u8], prefix: &[u8], terminator: u8) -> Option<i32> {
    for start in 0..bytes.len().saturating_sub(prefix.len()) {
        if &bytes[start..start + prefix.len()] != prefix {
            continue;
        }
        let mut j = start + prefix.len();
        let digit_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > digit_start && bytes.get(j) == Some(&terminator) {
            let digits = &bytes[digit_start..j];
            if let Ok(s) = std::str::from_utf8(digits) {
                if let Ok(n) = s.parse::<i32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Parse `>>TAG=<digits><` from command output.
fn parse_status_tag(out: &[u8], tag: &str) -> Option<i32> {
    let prefix = format!(">>{tag}=");
    match_status_tag(out, prefix.as_bytes(), b'<')
}

fn send_cmd_with_tag(pty: &mut PtyChild, tag: &str, cmd: &str) -> Vec<u8> {
    let line = format!("{cmd} ; __r=$? ; printf '>>{tag}=%d<\\n' \"$__r\"\n");
    pty.send(line.as_bytes());
    let prefix: Vec<u8> = format!(">>{tag}=").into_bytes();
    pty.drain_until(
        move |buf| match_status_tag(buf, &prefix, b'<').is_some(),
        Duration::from_secs(5),
    )
}

/// A single PTY session that exercises every `bind` scenario. Each
/// step uses a unique sentinel so we can locate its status in the
/// accumulated transcript.
#[test]
fn bind_builtin_covers_all_options() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };

    // The `bind` builtin is independent of whether the emacs line
    // editor is currently active (it mutates the globally shared
    // keymap). Turning emacs mode OFF avoids the per-keystroke redraw
    // that makes long command lines painfully slow to drive through
    // the PTY. Now that emacs is the default editing mode
    // (`docs/features/emacs-editing-mode.md` § 2.5), the opt-out has
    // to happen explicitly. Wait for the initial prompt first so the
    // raw-mode teardown from `set +o emacs` cannot race the startup
    // banner — otherwise we would see the `set +o emacs` line itself
    // redrawn character-by-character through the editor.
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set +o emacs\n");
    let _ = drain_until_contains(&mut pty, b"$ ");

    // `bind -l` must enumerate at least the three canonical function
    // names used throughout the plan.
    let out = send_cmd_with_tag(
        &mut pty,
        "BL",
        "bind -l | grep -cE '^(self-insert|beginning-of-line|accept-line)$'",
    );
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\r\n3\r\n") || text.contains("\n3\r\n") || text.contains("\n3\n"),
        "expected exactly 3 canonical function names in bind -l: {text:?}"
    );
    assert_eq!(parse_status_tag(&out, "BL"), Some(0));

    // `bind -p` must emit at least one inputrc-format line.
    let out = send_cmd_with_tag(
        &mut pty,
        "BP",
        "bind -p | grep -cE '^\"[^\"]+\"[[:space:]]*:'",
    );
    assert_eq!(
        parse_status_tag(&out, "BP"),
        Some(0),
        "BP output: {:?}",
        String::from_utf8_lossy(&out)
    );
    // The `grep -c` count must precede the sentinel `>>BP=` on its
    // own line and be strictly positive.
    let text = String::from_utf8_lossy(&out);
    let count_line = text
        .lines()
        .rev()
        .find(|l| l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty());
    let count = count_line.and_then(|l| l.parse::<u32>().ok()).unwrap_or(0);
    assert!(
        count > 0,
        "expected at least one inputrc-format line from bind -p, got count={count} in {text:?}"
    );

    // Single-argument form.
    let out = send_cmd_with_tag(&mut pty, "BS", "bind '\"\\C-xa\": accept-line'");
    assert_eq!(parse_status_tag(&out, "BS"), Some(0));

    // `bind -r` removes the key bound above.
    let out = send_cmd_with_tag(&mut pty, "BR", "bind -r '\\C-xa'");
    assert_eq!(parse_status_tag(&out, "BR"), Some(0));

    // `bind -r` on an unbound key is nonzero.
    let out = send_cmd_with_tag(&mut pty, "BRM", "bind -r '\\C-xZ'");
    assert!(
        matches!(parse_status_tag(&out, "BRM"), Some(n) if n != 0),
        "expected nonzero status for missing binding: {:?}",
        String::from_utf8_lossy(&out)
    );

    // `bind -f FILE` loads an inputrc file written from the test host.
    let path = format!("/tmp/meiksh-bind-f-{}", std::process::id());
    std::fs::write(&path, b"\"\\C-xq\": accept-line\n").expect("write rc");
    let cmd = format!("bind -f {path}");
    let out = send_cmd_with_tag(&mut pty, "BF", &cmd);
    let _ = std::fs::remove_file(&path);
    assert_eq!(parse_status_tag(&out, "BF"), Some(0));

    // `bind -x` installs a keyseq -> shell-command binding.
    let out = send_cmd_with_tag(&mut pty, "BX", "bind -x '\"\\C-xy\": echo SHELLCMD'");
    assert_eq!(parse_status_tag(&out, "BX"), Some(0));

    // Unknown option -> status 2.
    let out = send_cmd_with_tag(&mut pty, "BU", "bind -Z");
    assert_eq!(parse_status_tag(&out, "BU"), Some(2));

    // Editline positional form with caret notation: FreeBSD `~/.shrc`
    // convention — must install a binding on the rxvt/xterm
    // up-arrow sequence and report status 0. Covers the primary user-
    // reported regression.
    let out = send_cmd_with_tag(&mut pty, "BEL1", "bind ^[[A ed-search-prev-history");
    assert_eq!(
        parse_status_tag(&out, "BEL1"),
        Some(0),
        "BEL1 output: {:?}",
        String::from_utf8_lossy(&out)
    );

    // Confirm the installed sequence shows up in `bind -p`.
    let out = send_cmd_with_tag(
        &mut pty,
        "BELD1",
        "bind -p | grep -cE '^\"\\\\e\\[A\"[[:space:]]*:[[:space:]]*history-search-backward$'",
    );
    assert_eq!(parse_status_tag(&out, "BELD1"), Some(0));

    // Editline positional form with backslash notation and quoted arg.
    let out = send_cmd_with_tag(&mut pty, "BEL2", "bind '\\e[1;5C' em-next-word");
    assert_eq!(parse_status_tag(&out, "BEL2"), Some(0));

    // Editline positional form with an unsupported function name
    // (mode-switcher, non-goal in the spec) must return status 1.
    let out = send_cmd_with_tag(&mut pty, "BELU", "bind ^[qz vi-cmd-mode");
    assert_eq!(
        parse_status_tag(&out, "BELU"),
        Some(1),
        "BELU output: {:?}",
        String::from_utf8_lossy(&out)
    );

    // Bash-style multi-arg readline form: multiple `keyseq:function`
    // strings on a single invocation. Status 0 per bash compat.
    let out = send_cmd_with_tag(
        &mut pty,
        "BMA",
        "bind '\"\\C-xm\": accept-line' '\"\\C-xn\": beginning-of-line'",
    );
    assert_eq!(parse_status_tag(&out, "BMA"), Some(0));
    let out = send_cmd_with_tag(
        &mut pty,
        "BMAD",
        "bind -p | grep -cE '^\"\\\\C-xm\"[[:space:]]*:[[:space:]]*accept-line$|\
         ^\"\\\\C-xn\"[[:space:]]*:[[:space:]]*beginning-of-line$'",
    );
    assert_eq!(parse_status_tag(&out, "BMAD"), Some(0));

    let _ = pty.exit_and_wait();
}
