use super::interactive_common::{PtyChild, spawn_meiksh_pty};
use std::time::Duration;

// Integration tests that drive `meiksh -i` over a real PTY. The shell
// only activates its vi-mode line editor, terminal-mode wrappers
// (`tcgetattr` / `tcsetattr`), and the locale wrappers used by
// rendering (`encode_char`, `to_upper`, `to_lower`, `char_width`) when
// stdin is an interactive terminal, so these paths cannot be reached
// from the standard `-c` tests in the rest of this suite.
//
// All PTY / raw-terminal plumbing lives in `tests/integration/sys.rs`
// and is exposed through the safe `interactive_common` harness, so
// tests here carry zero `unsafe` and zero `libc`.

/// Vi-mode line editor: typing `hElLo`, pressing Esc to leave insert
/// mode, `0` to go to the start of the line, and `5~` to toggle the
/// case of the next five characters flips the word to `HeLlO` before
/// submitting. That single sequence exercises `sys::locale::decode_char`,
/// `classify_char`, `to_upper`, `to_lower`, `encode_char`, and
/// `char_width` for the redraw, plus the production `tcgetattr` /
/// `tcsetattr` wrappers used to put the PTY into raw mode.
///
/// The flipped word `HeLlO` is not a builtin or program, so the shell
/// writes a `not found` diagnostic that echoes the token verbatim;
/// that echo is what we pin down in the assertion. If the case-flip
/// ever regressed we would see the original `hElLo` (or some other
/// permutation) in the error line instead.
#[test]
fn vi_mode_tilde_toggle_over_pty_flips_case_before_submit() {
    let Some(mut pty): Option<PtyChild> = spawn_meiksh_pty(&[]) else {
        // Host has no PTY support — nothing to assert on in that case.
        return;
    };

    pty.send(b"set -o vi\n");
    // Insert `hElLo`, Esc → normal mode, `0` → beginning of line,
    // `5~` → toggle five characters, Enter → submit.
    pty.send(b"hElLo\x1b05~\n");
    let output = pty.drain_until(
        |b| b.windows(5).any(|w| w == b"HeLlO"),
        Duration::from_secs(5),
    );
    let _ = pty.exit_and_wait();

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("HeLlO"),
        "expected case-flipped token `HeLlO` in PTY transcript, got: {text:?}",
    );
}
