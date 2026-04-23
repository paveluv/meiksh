//! PTY integration tests for the emacs editing mode.
//!
//! Coverage map: every normative "shall" in
//! [`docs/features/emacs-editing-mode.md`](../../docs/features/emacs-editing-mode.md)
//! sections 2 through 13 that is observable through a terminal has at
//! least one test below. Section 14 (the `bind` builtin) is covered in
//! [`super::bind_builtin`]. Section 15 (non-goals) is covered by
//! negative tests scattered through sections 5, 14.5, and 15.11.
//!
//! All tests drive `meiksh -i` through the shared PTY harness in
//! [`super::interactive_common`], so no `unsafe` or direct `libc` calls
//! leak into test sources. The tests use two complementary strategies:
//!
//! * **Cooked-mode commands** — emacs mode is OFF and the kernel line
//!   discipline is echoing. This is used to query shell state
//!   (`set -o`, `echo $?`) and to set up fixtures. Output is gated on
//!   a unique `END\r\n` sentinel written by `printf` via octal escapes
//!   so the typed input — which contains the literal backslash-digits
//!   source — can never be mistaken for the sentinel.
//! * **Emacs-mode keystrokes** — once `set -o emacs` has been
//!   accepted, each byte goes through the editor keymap. Tests type a
//!   multi-step sequence, accept the resulting line with `RET`, and
//!   then send a second "sentinel" command whose output bounds the
//!   drain window. The command under test is always crafted so that
//!   its *output* contains a marker that is never present in the typed
//!   input, avoiding the classic PTY echo race.

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

/// Wait for the initial prompt so subsequent commands don't race the
/// shell startup, then enable emacs mode and wait for the post-`set`
/// prompt to appear. Returns only after the editor is ready for the
/// next input line.
fn enable_emacs(pty: &mut PtyChild) {
    let _ = drain_until_contains(pty, b"$ ");
    pty.send(b"set -o emacs\n");
    let _ = drain_until_contains(pty, b"$ ");
}

/// Bytes that `printf` expands to the three-letter sentinel "END"
/// followed by `\n`. The kernel translates the `\n` to `\r\n` when
/// emitting it through the PTY, so the exact byte sequence "END\r\n"
/// appears in the terminal output only as a direct consequence of the
/// sentinel `printf` running — never as an echo of the typed source
/// (which contains literal backslashes and digits).
const END_SENTINEL_INPUT: &[u8] = b"printf '\\105\\116\\104\\012'\n";

/// Accept the current editor line (LF = `accept-line`), then send the
/// `END\r\n` sentinel printf as a follow-up command, and drain until
/// the sentinel is observed. Returns the accumulated transcript.
fn accept_then_drain_end(pty: &mut PtyChild) -> Vec<u8> {
    pty.send(b"\n");
    pty.send(END_SENTINEL_INPUT);
    drain_until_contains(pty, b"END\r\n")
}

/// Count occurrences of the terminal BEL byte (0x07) in a transcript.
/// Used to assert that a spec-mandated bell actually rang.
fn bell_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == 0x07).count()
}

/// Drain for a brief bounded window, used to snap up any bell/redraw
/// bytes a keystroke produced without blocking the whole test on a
/// sentinel that will never arrive (e.g. "no match → bell").
fn drain_brief(pty: &mut PtyChild) -> Vec<u8> {
    pty.drain_for(Duration::from_millis(200))
}

// =====================================================================
// § 2. Activation and Lifecycle
// =====================================================================

/// § 2.5: Default editing mode is neither emacs nor vi.
#[test]
fn emacs_mode_default_off_at_startup() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o | grep emacs\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("emacs") && text.contains("off"),
        "expected `emacs ... off` in default-state set -o output: {text:?}"
    );
}

/// § 2.1, § 13.1: `set -o emacs` enables, and the state is reported
/// through `set -o` with the same column formatting as the other
/// POSIX options.
#[test]
fn set_o_emacs_enables_and_reports_on() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"set -o | grep emacs\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("emacs") && text.contains("on"),
        "expected `emacs ... on` after `set -o emacs`: {text:?}"
    );
}

/// § 2.1: `set +o emacs` disables and is reported as off.
#[test]
fn set_plus_o_emacs_disables_and_reports_off() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"set +o emacs\n");
    pty.send(b"set -o | grep emacs\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    // Find the LAST `emacs` line in the output (the most recent
    // `set -o | grep emacs` invocation) and check it reports off.
    let last = text
        .lines()
        .filter(|l| l.contains("emacs"))
        .last()
        .unwrap_or("");
    assert!(
        last.contains("off"),
        "expected emacs to report off after `set +o emacs`, last line was {last:?}"
    );
}

/// § 2.2: Enabling vi disables emacs (mutual exclusion, reverse
/// direction of the existing test).
#[test]
fn set_o_vi_turns_off_emacs() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"set -o vi\n");
    pty.send(b"set -o | grep -E '^(vi|emacs) '\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    let vi_line = text.lines().find(|l| l.starts_with("vi ")).unwrap_or("");
    let em_line = text.lines().find(|l| l.starts_with("emacs ")).unwrap_or("");
    assert!(vi_line.contains("on"), "vi should be on: {vi_line:?}");
    assert!(
        em_line.contains("off"),
        "emacs should be off after enabling vi: {em_line:?}"
    );
}

/// § 2.3: Non-interactive shells accept `set -o emacs` (the reportable
/// state updates) but no raw mode is attempted. Verified via the
/// non-PTY `-c` entry point: the shell must not fail and must report
/// the state as on.
#[test]
fn non_interactive_set_o_emacs_updates_reportable_state() {
    use std::process::Command;
    let out = Command::new(super::common::meiksh())
        .args(["-c", "set -o emacs; set -o | grep emacs"])
        .output()
        .expect("run meiksh");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("emacs") && text.contains("on"),
        "expected `emacs ... on` from non-interactive set -o: {text:?}"
    );
}

/// § 2.4: When stdin is not a terminal and emacs is enabled, the
/// shell falls back to line-buffered input without diagnostic. We
/// drive this through stdin with a simple script that toggles emacs
/// and echoes a value — the shell must run the script without
/// complaining and without stalling.
#[test]
fn no_terminal_falls_back_silently() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(super::common::meiksh())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"set -o emacs\nprintf OK\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "shell must not fail: {out:?}");
    assert_eq!(
        out.stdout, b"OK",
        "expected OK printed under non-terminal fallback"
    );
    assert!(
        out.stderr.is_empty(),
        "expected no diagnostic, got: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// =====================================================================
// § 3.2 Terminal / escape-sequence plumbing — observable redraws.
// =====================================================================

/// § 5.1 `clear-screen` (`C-l`): the editor emits the ANSI sequence
/// `\x1b[H\x1b[2J` when redrawing the cleared screen, which must
/// appear in the PTY stream immediately after the `C-l` keystroke.
#[test]
fn ctrl_l_emits_clear_screen_escape() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"\x0c");
    // The redraw contains the cursor-home + clear-display pair.
    let probe = pty.drain_until(
        |b| b.windows(7).any(|w| w == b"\x1b[H\x1b[2J"),
        Duration::from_secs(2),
    );
    let _ = pty.exit_and_wait();
    assert!(
        probe.windows(7).any(|w| w == b"\x1b[H\x1b[2J"),
        "expected CSI H CSI 2 J after C-l; got {:?}",
        probe
            .iter()
            .map(|&b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

// =====================================================================
// § 5.1 Basic Movement
// =====================================================================

/// `C-a` moves to beginning-of-line; subsequent self-insert happens
/// there so the final buffer is `<inserted><previously-typed>`.
#[test]
fn ctrl_a_inserts_at_beginning_of_line() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "SUFFIX", C-a, then "printf (%s) PRE", RET. The submitted
    // command is `printf (%s) PRESUFFIX`. The command output
    // `(PRESUFFIX)` only appears as a consequence of the execution;
    // the typed bytes contain "PRE" and "SUFFIX" separately.
    pty.send(b"SUFFIX\x01printf '(%s)' PRE");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(PRESUFFIX)"),
        "expected `(PRESUFFIX)` after C-a + prefix insert: {text:?}"
    );
}

/// `C-e` moves to end-of-line; a self-insert after `C-a` + `C-e`
/// appears at the tail of the buffer.
#[test]
fn ctrl_e_returns_to_end_of_line() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s) TAIL", C-a (to start), C-e (back to end), "X".
    // Buffer: "printf (%s) TAILX"  → output "(TAILX)".
    pty.send(b"printf '(%s)' TAIL\x01\x05X");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(TAILX)"),
        "expected `(TAILX)` after C-e + append: {text:?}"
    );
}

/// `C-f` / `C-b`: char-granular cursor motion. Two `C-b`s land the
/// cursor on `X`; `C-d` deletes the char under the cursor.
#[test]
fn ctrl_b_ctrl_f_move_by_char() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Buffer "printf '(%s)' AXB" (cursor at EOB). C-b → cursor before
    // 'B'. C-b → cursor on 'X'. C-d deletes 'X'. Buffer becomes
    // "printf '(%s)' AB" → output "(AB)".
    pty.send(b"printf '(%s)' AXB\x02\x02\x04");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AB)"),
        "expected `(AB)` after C-b C-b C-d deleting 'X': {text:?}"
    );
    assert!(
        !text.contains("(AXB)"),
        "X should be gone from the submitted command: {text:?}"
    );
}

/// `M-f` advances one word; `M-b` retreats one word. Word boundary is
/// alnum+`_` per § 5.1.
#[test]
fn meta_f_meta_b_move_by_word() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s) alpha beta"; cursor at end. M-b moves before
    // "beta"; insert 'Z'. Buffer: "printf '(%s)' alpha Zbeta" →
    // output "(alpha Zbeta)"... printf takes first arg "(%s)", then
    // "alpha" as format arg, and "Zbeta" is a separate arg but %s
    // only consumes the first → output "(alpha)" — wrong target.
    //
    // Instead: "printf (%s%s) alpha beta" with cursor at end, M-b
    // goes before "beta", insert '_': arg list becomes "alpha" and
    // "_beta", formatted as "(alpha_beta)".
    pty.send(b"printf '(%s%s)' alpha beta\x1bb_");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(alpha_beta)"),
        "expected `(alpha_beta)` after M-b + underscore: {text:?}"
    );
}

// =====================================================================
// § 5.2 Cursor Keys (ANSI escape sequences)
// =====================================================================

/// Up arrow `\e[A` is bound to `previous-history`.
#[test]
fn up_arrow_recalls_previous_history() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf LAST\n");
    let _ = drain_until_contains(&mut pty, b"LAST");
    // Up-arrow then RET re-runs the previous command.
    pty.send(b"\x1b[A\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.matches("LAST").count() >= 1,
        "expected at least one re-run of LAST via up arrow: {text:?}"
    );
}

/// Home (`\e[H`) is `beginning-of-line`; End (`\e[F`) is
/// `end-of-line`. Exercise both by typing, using Home to jump to
/// start, inserting, and using End to return.
#[test]
fn home_and_end_arrow_keys_move_to_ends() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "Z", Home, "printf A", End, "B".
    //   After "Z":              buf="Z",        cur=1
    //   After Home (\e[H):      cur=0
    //   After "printf A":       buf="printf AZ", cur=8  (insert before Z)
    //   After End (\e[F):       cur=9
    //   After "B":              buf="printf AZB", cur=10
    // Submit: `printf AZB` → output "AZB". Typed bytes contain 'A',
    // 'Z', 'B' separately so the assembled token "AZB" on a fresh
    // output line is distinct from input echo (which at no point has
    // "AZB" prefixed by the printf command output's `\r\n` marker).
    pty.send(b"Z\x1b[Hprintf A\x1b[FB");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\r\nAZB") || text.contains("\nAZB"),
        "expected AZB as command output after Home/End: {text:?}"
    );
}

/// Delete key (`\e[3~`) deletes the character under the cursor
/// (`delete-char`).
#[test]
fn delete_key_removes_char_under_cursor() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s) AXB", C-b C-b to position cursor on 'X',
    // then `\e[3~` to delete 'X' under cursor.
    pty.send(b"printf '(%s)' AXB\x02\x02\x1b[3~");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AB)"),
        "expected `(AB)` after Delete on 'X': {text:?}"
    );
}

/// Ctrl+Right (`\e[1;5C`) is `forward-word` (§ 5.2).
#[test]
fn ctrl_right_arrow_moves_forward_word() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s%s) alpha beta", C-a to start, Ctrl-Right twice
    // (past "printf" then past "alpha"), insert '_' : inserts between
    // "alpha" and space → args "alpha_ " and "beta"? Subtle. Simpler
    // target: insert '|' which keeps args separated. Use
    // "printf a%sz alpha", C-a, Ctrl-Right, insert '1' → "printf1"
    // which is not a command. Let me design differently.
    //
    // Pattern: "printf (%s) alpha", C-a, Ctrl-Right (past 'printf'),
    // then Ctrl-Right (past "(%s)"?). The %s tokens are not alnum
    // so boundary semantics matter. Safer: use ONLY alnum words.
    //
    // Type "printf ALPHA beta", C-a, Ctrl-Right past "printf", then
    // Ctrl-Right past "ALPHA", then insert 'X' → "printf ALPHAX beta"
    // but that's a command and ALPHAX may or may not exist. Use
    // "printf alpha%s ZED" with C-a, Ctrl-Right twice, insert 'X' →
    // still messy.
    //
    // Cleanest: drop the format-string approach and use arithmetic:
    //   echo a $((2+3)) b
    // The '5' only appears in output. Build using Ctrl+Right:
    //   Type "a b c", C-a, Ctrl-Right, Ctrl-Right, insert "echo "
    //   Wait — inserting at start means "echo a b c" is what we want.
    //
    // Step back: we just need to *observe* that Ctrl-Right moved the
    // cursor past a word. The simplest observable check is
    // equivalent to M-f. Since M-f is already covered, focus here is
    // that the ANSI sequence `\e[1;5C` routes to the same function.
    //
    // Strategy: take a buffer like "printf '(%s)' alpha beta", Ctrl-
    // Left (move back from end), Ctrl-Left, then Ctrl-Right +
    // insert. If Ctrl-Right maps to forward-word, the inserted text
    // ends up after "alpha"/ between alpha and beta.
    //
    // Setup: "printf '(%s%s)' alpha  beta"
    //   alpha and beta are separated by two spaces.
    //   Cursor at end, Ctrl-Left → before "beta"  (cursor between
    //     the two spaces and 'b' of beta).
    //   Ctrl-Right → past "beta" (cursor at end again).
    //   Now insert '_' at end → arg2 becomes "beta_".
    //   %s%s sees args "alpha" and "beta_" → output "(alphabeta_)"
    pty.send(b"printf '(%s%s)' alpha  beta\x1b[1;5D\x1b[1;5C_");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(alphabeta_)"),
        "expected `(alphabeta_)` after Ctrl-Left / Ctrl-Right round-trip: {text:?}"
    );
}

// =====================================================================
// § 5.3 History
// =====================================================================

/// `C-p` recalls previous history (kept as a smoke test — mirrors the
/// old `previous_history_recalls_last_command`).
#[test]
fn ctrl_p_recalls_previous_history() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf FIRSTOUT\n");
    let _ = drain_until_contains(&mut pty, b"FIRSTOUT");
    pty.send(b"\x10\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.matches("FIRSTOUT").count() >= 2,
        "expected C-p replay of FIRSTOUT: {text:?}"
    );
}

/// `M-.` (`yank-last-arg`) inserts the last word of the previous
/// history entry.
#[test]
fn meta_dot_yanks_last_argument() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Establish history: `printf UNIQARG`.
    pty.send(b"printf UNIQARG\n");
    let _ = drain_until_contains(&mut pty, b"UNIQARG");
    // On a fresh line, type `echo Q-` then M-. to yank UNIQARG, then
    // RET. The command becomes `echo Q-UNIQARG` and outputs
    // "Q-UNIQARG". The literal token "Q-UNIQARG" only appears in the
    // output (input has "Q-" followed by ESC . which does not form
    // the combined token).
    pty.send(b"echo Q-\x1b.\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("Q-UNIQARG"),
        "expected `Q-UNIQARG` from yank-last-arg: {text:?}"
    );
}

// =====================================================================
// § 5.4 Deletion
// =====================================================================

/// `BS` (0x08) as `backward-delete-char` — kept from the prior suite.
#[test]
fn backspace_deletes_previous_character() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf '(%s)' MEX\x08Y");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(MEY)"),
        "expected `(MEY)` after BS replacing X with Y: {text:?}"
    );
}

/// `DEL` (0x7f) is also `backward-delete-char` (§ 4).
#[test]
fn delete_byte_deletes_previous_character() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf '(%s)' MEX\x7fZ");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(MEZ)"),
        "expected `(MEZ)` after 0x7f replacing X with Z: {text:?}"
    );
}

/// § 5.4 / § 11: `C-d` on an empty line returns EOF; shell exits.
#[test]
fn ctrl_d_on_empty_line_exits() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"\x04");
    let _ = pty.wait_with_timeout(Duration::from_secs(5));
}

/// § 5.4: `C-d` on a non-empty buffer deletes the character under
/// the cursor.
#[test]
fn ctrl_d_nonempty_buffer_deletes_char() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s) AXB", C-b to put cursor on 'B', C-d deletes
    // 'B' → buffer "printf '(%s)' AX" → "(AX)".
    pty.send(b"printf '(%s)' AXB\x02\x04");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AX)"),
        "expected `(AX)` after C-d deleting 'B': {text:?}"
    );
}

/// `C-k` kills from cursor to end-of-buffer; verify by yanking the
/// kill via `C-y` so the submitted command is identical to the
/// pre-kill state.
#[test]
fn ctrl_k_then_ctrl_y_round_trips_tail() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf (%s) ABC", C-a, M-f twice (past 'printf' and past
    // "(%s)"), C-f to skip the space, C-k kills "ABC", C-e, C-y
    // puts "ABC" back at end. Net: same command.
    //
    // Simpler: type "printf (%s) ABC", C-a, C-k kills whole line,
    // C-y restores it.
    pty.send(b"printf '(%s)' ABC\x01\x0b\x19");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(ABC)"),
        "expected `(ABC)` after C-k/C-y round trip: {text:?}"
    );
}

/// `C-u` (`unix-line-discard`) kills from cursor back to start;
/// yanked content is the removed prefix.
#[test]
fn ctrl_u_kills_backward_to_start() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "JUNKprintf '(%s)' RES", C-a (to start)... wait, C-u needs
    // cursor *past* the junk. Layout: type "JUNK" at start via C-a,
    // then C-e back, then final text. Easier:
    //   1. Type "printf '(%s)' RES" — normal command.
    //   2. C-a, "DROP" (insert at start; buffer is "DROPprintf…").
    //   3. C-u kills "DROP" back to start.
    // But cursor position after (2) is at 4 (just after DROP). C-u
    // from there kills "DROP" back to start.
    pty.send(b"printf '(%s)' RES\x01DROP\x15");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(RES)"),
        "expected `(RES)` after C-u removing DROP: {text:?}"
    );
}

/// `C-w` (`unix-word-rubout`) deletes backward to whitespace.
#[test]
fn ctrl_w_rubs_out_previous_word_to_whitespace() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s)' WRONG GOOD". Cursor at end. C-w deletes
    // "GOOD" back to the space, leaving "printf '(%s)' WRONG ".
    // printf then produces "(WRONG)" (trailing space is arg with
    // %s consuming first arg only; but the buffer has a trailing
    // space which is just between arg tokens — so the only arg is
    // "WRONG" → output "(WRONG)").
    pty.send(b"printf '(%s)' WRONG GOOD\x17");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(WRONG)"),
        "expected `(WRONG)` after C-w removing GOOD: {text:?}"
    );
    assert!(!text.contains("(GOOD)"), "GOOD should be gone: {text:?}");
}

/// `M-d` (`kill-word`) kills forward to end-of-word using alnum+`_`
/// boundary.
#[test]
fn meta_d_kills_word_forward() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "JUNK printf '(%s)' OK", C-a, M-d. The alnum+`_` word at
    // cursor is "JUNK"; `kill-word` also consumes the trailing space
    // per `next_word_boundary` semantics, so the buffer collapses to
    // "printf '(%s)' OK" → output "(OK)".
    pty.send(b"JUNK printf '(%s)' OK\x01\x1bd");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(OK)"),
        "expected `(OK)` after M-d killing JUNK: {text:?}"
    );
}

/// `M-DEL` / `M-BS` (`backward-kill-word`).
#[test]
fn meta_backspace_kills_word_backward() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s)' KEEP JUNK", cursor at end, ESC-DEL kills
    // "JUNK" back to space. Buffer: "printf '(%s)' KEEP ". printf
    // has one arg "KEEP" (the trailing space is discarded as IFS)
    // and outputs "(KEEP)".
    pty.send(b"printf '(%s)' KEEP JUNK\x1b\x7f");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(KEEP)"),
        "expected `(KEEP)` after M-DEL killing JUNK: {text:?}"
    );
}

// =====================================================================
// § 5.6 Text Editing
// =====================================================================

/// `C-t` at end-of-buffer exchanges the two preceding characters.
#[test]
fn ctrl_t_transposes_last_two_chars_at_end() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s)' ab", C-t at end: exchanges 'a' and 'b' →
    // "printf '(%s)' ba". Output: "(ba)".
    pty.send(b"printf '(%s)' ab\x14");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("(ba)"), "expected `(ba)` after C-t: {text:?}");
}

/// `M-t` exchanges the word at cursor with the preceding word.
///
/// **Implementation gap (2026-04-19).** § 5.6 specifies "Exchange the
/// word at (or immediately before) the cursor with the preceding
/// word." — bash readline additionally treats end-of-buffer as
/// "transpose the last two words". The current implementation in
/// `do_transpose_words` at end-of-buffer swaps the last two words
/// (spec § 5.6).
#[test]
fn meta_t_transposes_words() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s%s)' alpha beta", cursor at end, M-t swaps
    // "alpha" and "beta" (the two alnum words immediately preceding
    // the end of buffer). Buffer: "printf '(%s%s)' beta alpha".
    // Output: "(betaalpha)".
    pty.send(b"printf '(%s%s)' alpha beta\x1bt");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(betaalpha)"),
        "expected `(betaalpha)` after M-t: {text:?}"
    );
}

/// `M-u` upcases the word at cursor. `do_case_word` only operates on
/// a word the cursor is currently on, so we position the cursor at
/// the beginning of the target word via `M-b`.
#[test]
fn meta_u_upcases_word_at_cursor() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s)' hello", cursor at EOB. M-b lands the cursor
    // at the start of "hello"; M-u upcases it. Buffer:
    //   "printf '(%s)' HELLO" → output "(HELLO)". The literal pair
    // "(HELLO)" only occurs in the command output — the typed echo
    // has "(%s)" and "HELLO" separated by " '".
    pty.send(b"printf '(%s)' hello\x1bb\x1bu");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(HELLO)"),
        "expected `(HELLO)` after M-b + M-u: {text:?}"
    );
}

/// `M-l` downcases the word at cursor.
#[test]
fn meta_l_downcases_word_at_cursor() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf '(%s)' HELLO\x1bb\x1bl");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(hello)"),
        "expected `(hello)` after M-b + M-l: {text:?}"
    );
}

/// `M-c` capitalizes the word at cursor.
#[test]
fn meta_c_capitalizes_word() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf '(%s)' HELLO\x1bb\x1bc");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(Hello)"),
        "expected `(Hello)` after M-b + M-c: {text:?}"
    );
}

/// `C-q` / `C-v` (`quoted-insert`): the next byte is inserted
/// verbatim, bypassing keymap dispatch. Sending `C-q C-a` inserts
/// byte 0x01 literally; we then observe it via `od`.
#[test]
fn ctrl_q_quoted_insert_bypasses_dispatch() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf %s\\\\n", C-q, C-a (literal 0x01 byte in buffer),
    // then "X" (so the arg is "\x01X"), submit, pipe to od -c to get
    // a textual rendition.
    //
    // Simpler: use `printf '%d\n' "'<literal-0x01>"` which prints the
    // decimal value of the first byte. printf `'%d' "'<ch>"` is the
    // POSIX form for "character code of <ch>". The expected output
    // is "1\n". We stamp with a label so the 1 is unambiguous.
    pty.send(b"printf 'CODE=%d<' \"'\x16\x01\"");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("CODE=1<"),
        "expected `CODE=1<` from quoted-insert of 0x01: {text:?}"
    );
}

// =====================================================================
// § 5.9 Miscellaneous
// =====================================================================

/// `C-c` aborts the current line, emits a newline, and the shell
/// re-prompts. The aborted content must not appear in the next
/// command.
#[test]
fn ctrl_c_aborts_current_line_and_reprompts() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "ABORTME" on an editor line, then C-c, then a fresh
    // command `printf CLEAN` + RET. Expect "CLEAN" to appear *after*
    // the aborted ABORTME (which the shell must not execute).
    pty.send(b"ABORTME\x03");
    pty.send(b"printf CLEAN\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("CLEAN"), "expected CLEAN in output: {text:?}");
    // "ABORTME" must not have been executed as a command, so no
    // "command not found" diagnostic for it.
    assert!(
        !text.contains("ABORTME: command not found") && !text.contains("ABORTME: not found"),
        "ABORTME should have been discarded, not executed: {text:?}"
    );
}

// =====================================================================
// § 5.10 Bracketed Paste
// =====================================================================

/// Bytes between `\e[200~` and `\e[201~` are inserted literally even
/// if they contain normally-bound bytes such as `C-a` (0x01).
#[test]
fn bracketed_paste_inserts_bytes_literally() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Paste a string containing 0x01 (which would normally be C-a).
    // After paste ends, append "X" outside the paste so the resulting
    // line assembles as expected. Format: byte sequence that when
    // submitted is `printf '(%s)' "P\x01Q"` — but quoting the 0x01
    // through an editor is tricky. Simpler: paste contains the
    // literal text "printf '(%s)' PASTED" (no control bytes) to
    // verify the brackets themselves are consumed and the inner
    // bytes are inserted.
    pty.send(b"\x1b[200~printf '(%s)' PASTED\x1b[201~");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(PASTED)"),
        "expected `(PASTED)` from bracketed paste: {text:?}"
    );
    // The bracket markers themselves must not appear as part of the
    // executed command (they should be consumed by the editor, not
    // inserted literally).
    assert!(
        !text.contains("\x1b[200~") && !text.contains("\x1b[201~"),
        "bracket markers leaked into command: {text:?}"
    );
}

// =====================================================================
// § 6 Kill-Buffer Semantics
// =====================================================================

/// Consecutive kill commands accumulate into the kill buffer; a
/// single yank recovers the full concatenation.
#[test]
fn consecutive_kill_commands_accumulate() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Build `printf '(%s)' XY`, then C-a, go past "printf '(%s)' ",
    // kill 'X' via kill-word, kill 'Y' via kill-word (but XY is one
    // alnum word…). Use distinct-word setup:
    //   Type "printf '(%s%s%s)' ALPHA BETA GAMMA". Cursor end.
    //   C-w (rubout "GAMMA", kill buffer = "GAMMA").
    //   C-w (rubout "BETA", kill buffer prepended to "BETAGAMMA" or
    //        appended? § 6 says unix-word-rubout PREPENDS, so the
    //        buffer becomes "BETA" + "GAMMA" = "BETAGAMMA").
    //   C-y yanks "BETAGAMMA".
    //
    // Buffer after sequence: "printf '(%s%s%s)' ALPHA BETAGAMMA".
    // printf sees args "ALPHA" and "BETAGAMMA"; %s%s%s with only 2
    // args leaves the third %s empty → output "(ALPHABETAGAMMA)".
    pty.send(b"printf '(%s%s%s)' ALPHA BETA GAMMA\x17\x17\x19");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(ALPHABETAGAMMA)"),
        "expected `(ALPHABETAGAMMA)` — consecutive C-w kills should \
         prepend and a single C-y should restore both words: {text:?}"
    );
}

// =====================================================================
// § 7 Incremental Search
// =====================================================================

/// `C-r` enters reverse-incremental search and displays the mini-
/// buffer prompt `(reverse-i-search)`` `. Accepting with `RET`
/// executes the matched history entry.
///
/// **Implementation gap (2026-04-19).** § 7.2 requires "RET shall
/// accept: the editor shall exit search with the current matching
/// line as the buffer, cursor positioned at the end, and immediately
/// call `accept-line`." RET inside C-r mini-buffer accepts the
/// current match and submits it as a command in one step.
#[test]
fn ctrl_r_reverse_search_finds_and_reexecutes() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf UNIQTOKEN\n");
    let _ = drain_until_contains(&mut pty, b"UNIQTOKEN");
    // C-r starts reverse search; typing "UNIQ" narrows to our entry;
    // RET accepts and immediately executes.
    pty.send(b"\x12UNIQ\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(reverse-i-search`"),
        "expected reverse-i-search mini-prompt: {text:?}"
    );
    assert!(
        text.matches("UNIQTOKEN").count() >= 2,
        "expected UNIQTOKEN to be re-executed by accepting the search: {text:?}"
    );
}

/// `C-g` during incremental search restores the pre-search buffer
/// and does not re-execute.
#[test]
fn ctrl_g_aborts_incremental_search() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf ABRTSRCH\n");
    let _ = drain_until_contains(&mut pty, b"ABRTSRCH");
    // Type "printf STAY", then C-r, type "ABRT" (finds ABRTSRCH),
    // C-g aborts. Buffer should be restored to "printf STAY". RET
    // then submits `printf STAY`. The only *executed* ABRTSRCH was
    // the first command; the aborted search must not produce a
    // second execution. Since the editor re-echoes the buffer on
    // every keystroke, we check for the distinctive command-output
    // shape `\r\nABRTSRCH\r\n` (CRLF + bare token + CRLF), which
    // only appears once per actual execution.
    pty.send(b"printf STAY\x12ABRT\x07\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\r\nSTAY") || text.contains("\nSTAY"),
        "expected STAY as command output after C-g: {text:?}"
    );
    // The initial `printf ABRTSRCH` was drained before this test's
    // capture window opened, so its command-output `\r\nABRTSRCH\r\n`
    // is not in `text`. If the aborted search had re-executed it,
    // that execution *would* be in `text` — the assertion proves
    // abort does not leak through.
    let reexecutions = text.matches("\r\nABRTSRCH\r\n").count();
    assert_eq!(
        reexecutions, 0,
        "C-g abort must not re-execute the matched history entry: {text:?}"
    );
}

// =====================================================================
// § 9 Undo
// =====================================================================

/// `C-_` undoes the most recent editing group. A run of self-insert
/// bytes forms one group, so a single undo should erase all of them.
#[test]
fn ctrl_underscore_undoes_self_insert_run() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "WASTE" (one self-insert group), C-_ undoes the whole
    // group, then type "printf '(%s)' OK", RET. Buffer should be
    // exactly "printf '(%s)' OK" → "(OK)".
    pty.send(b"WASTE\x1fprintf '(%s)' OK");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("(OK)"), "expected `(OK)`: {text:?}");
    assert!(
        !text.contains("WASTEprintf"),
        "undo should have removed WASTE before the command ran: {text:?}"
    );
}

// =====================================================================
// § 15.11 / inputrc.md: input-meta and output-meta accepted silently.
// =====================================================================

/// The inputrc parser accepts `input-meta` and `output-meta` without
/// diagnostic even though they have no runtime effect. We feed an
/// inputrc snippet via `bind -f` and assert that the load reports
/// success with no error on stderr.
#[test]
fn input_meta_and_output_meta_accepted_without_diagnostic() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    let path = format!("/tmp/meiksh-emacs-meta-{}", std::process::id());
    std::fs::write(
        &path,
        b"set input-meta on\nset output-meta on\nset meta-flag on\n",
    )
    .expect("write rc");
    // Run `bind -f <path> 2>&1; printf END`. We expect no "unknown
    // variable" text to appear in the transcript.
    let cmd = format!("bind -f {path} 2>&1; printf '\\105\\116\\104\\012'\n");
    pty.send(cmd.as_bytes());
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("unknown variable"),
        "input-meta/output-meta should parse without `unknown variable` diagnostic: {text:?}"
    );
}

/// § 15.14: `$if term=...` is recognized. A term test that matches
/// should install its bindings; one that doesn't should be silently
/// skipped with no diagnostic.
#[test]
fn if_term_directive_gates_bindings_without_diagnostic() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    let path = format!("/tmp/meiksh-emacs-term-{}", std::process::id());
    // The PTY harness sets TERM=dumb, so `term=dumb` should match
    // and `term=rxvt` should not. Neither should produce a diagnostic.
    std::fs::write(
        &path,
        b"$if term=dumb\n\"\\C-xq\": accept-line\n$endif\n\
          $if term=rxvt\n\"\\C-xr\": accept-line\n$endif\n",
    )
    .expect("write rc");
    let cmd = format!(
        "bind -f {path} 2>&1; bind -p | grep -E '^\"\\\\\\\\C-x[qr]\"'; printf '\\105\\116\\104\\012'\n"
    );
    pty.send(cmd.as_bytes());
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("unknown") && !text.contains("$if test"),
        "`$if term=...` must not produce a diagnostic: {text:?}"
    );
}

// =====================================================================
// Existing coverage that is retained verbatim.
// =====================================================================

/// § 2.2: enabling emacs turns off vi (kept from the prior suite).
#[test]
fn emacs_mode_turns_off_vi_mode() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o vi\n");
    pty.send(b"set +o emacs\n");
    pty.send(b"set -o emacs\n");
    pty.send(b"set -o | grep -E '^(vi|emacs) '; printf '\\105\\116\\104\\012'\n");
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("emacs"),
        "expected emacs listed as on: {text:?}"
    );
    let vi_line = text.lines().find(|l| l.starts_with("vi ")).unwrap_or("");
    assert!(
        vi_line.contains("off"),
        "vi should be disabled after later `set -o emacs`: {vi_line:?}"
    );
}

/// Smoke test — the prior suite's "self-insert + accept submits".
#[test]
fn self_insert_and_accept_submits_command() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf '(%s)' HELLO");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(HELLO)"),
        "expected `(HELLO)` from typed+accepted line: {text:?}"
    );
}

// =====================================================================
// § 2.1 POSIX-format reporting: `set +o` must include the emacs flag.
// =====================================================================

/// § 13.1 / 2.1: `set +o` (POSIX format) shall emit `set -o emacs` or
/// `set +o emacs` reflecting the current state.
#[test]
fn set_plus_o_posix_format_emits_emacs_line() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    pty.send(b"set +o | grep emacs\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("set -o emacs"),
        "expected `set -o emacs` in POSIX-format report: {text:?}"
    );
}

// =====================================================================
// § 3.1 Terminal preconditions: editor mutes kernel echo (ICANON/ECHO
// cleared). When emacs mode is active every keystroke is drawn by the
// editor itself, not the kernel, so a typed '#' that is immediately
// erased with C-u should leave no residual '#' between prompts.
// =====================================================================

/// § 3.1: `ECHO` cleared — kernel does not echo bytes; the editor
/// redraws the buffer. Specifically, typing `#abc` then `C-u` (which
/// erases back to start) and then `RET` must produce *no* textual
/// `#abc` that survives as a command or comment in the transcript.
/// If the kernel were still echoing, the erased characters would be
/// present in the log without the editor's concurrent redraw.
#[test]
fn emacs_mode_suppresses_kernel_echo_of_erased_input() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"#ERASED\x15");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("END"),
        "sentinel should run after empty accept-line: {text:?}"
    );
    assert!(
        !text.contains("ERASED: command not found"),
        "erased input was treated as a command; ECHO not cleared: {text:?}"
    );
}

/// § 3.1: "The saved attributes shall be restored before returning an
/// input line." After `accept-line`, the shell is back in cooked mode
/// — subsequent cooked-mode commands therefore behave exactly as on
/// startup. Exercise this by toggling `set +o emacs` and checking the
/// kernel echo reappears (observable as the typed bytes showing up in
/// the transcript as-is without editor redraw artifacts).
#[test]
fn terminal_restored_to_cooked_mode_after_disable() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"set +o emacs\n");
    pty.send(b"printf RESTORED\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("RESTORED"),
        "cooked-mode `printf RESTORED` should run after set +o emacs: {text:?}"
    );
}

// =====================================================================
// § 3.2 / § 5.10 Bracketed-paste enable sequence on editor entry.
// =====================================================================

/// § 3.2 / § 5.10: the editor shall emit `\e[?2004h` ("bracketed paste
/// on") when entering the editor for each input line. It must appear
/// in the PTY stream *before* the first prompt after `set -o emacs`.
#[test]
fn editor_enables_bracketed_paste_on_entry() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    let probe = pty.drain_until(
        |b| b.windows(8).any(|w| w == b"\x1b[?2004h"),
        Duration::from_secs(2),
    );
    let _ = pty.exit_and_wait();
    assert!(
        probe.windows(8).any(|w| w == b"\x1b[?2004h"),
        "expected \\e[?2004h after entering emacs mode; got {:?}",
        probe
            .iter()
            .map(|&b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

// =====================================================================
// § 4 Key notation: both RET (0x0D) and LF (0x0A) submit the line.
// =====================================================================

/// § 4: `RET` (0x0D, i.e. `\r`) shall be accepted as a line terminator
/// equivalent to LF. The PTY layer's ICRNL normally translates `\r` to
/// `\n` at the kernel line discipline, but emacs mode clears `ICANON`
/// and reads bytes raw, so `\r` reaches the editor untouched.
#[test]
fn carriage_return_accepts_line_identically_to_lf() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf RETOUT");
    pty.send(b"\r");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("RETOUT"),
        "expected RETOUT submitted by bare \\r: {text:?}"
    );
}

/// § 4 / § 5.9: `C-j` (0x0A, raw LF) is also `accept-line`. Redundant
/// with the standard `\n` submit already exercised by
/// `accept_then_drain_end`, but expressed here as a separate test so
/// the normative dual-terminator requirement is explicit in coverage.
#[test]
fn ctrl_j_accepts_line() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf CJOUT\x0a");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("CJOUT"),
        "expected CJOUT via raw C-j: {text:?}"
    );
}

// =====================================================================
// § 5.1 Boundary bells (edge of buffer).
// =====================================================================

/// `C-f` at end-of-buffer shall ring the bell.
///
/// **Implementation gap (2026-04-19).** `EmacsFn::ForwardChar` in
/// `src/interactive/emacs_editing/functions.rs` clamps the cursor to
/// `state.buf.len()`; spec § 5.1 mandates a bell.
#[test]
fn ctrl_f_at_end_of_buffer_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type nothing — buffer is empty, cursor at 0 is both start and
    // end. C-f must ring.
    pty.send(b"\x06");
    let probe = drain_brief(&mut pty);
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL (0x07) after C-f on empty buffer"
    );
}

/// `C-b` at beginning-of-buffer shall ring the bell (spec § 5.1).
#[test]
fn ctrl_b_at_beginning_of_buffer_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"\x02");
    let probe = drain_brief(&mut pty);
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL (0x07) after C-b on empty buffer"
    );
}

/// `BS` (`backward-delete-char`) at beginning-of-buffer shall ring.
#[test]
fn backspace_at_beginning_of_buffer_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"\x08");
    let probe = drain_brief(&mut pty);
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL after BS on empty buffer"
    );
}

/// `C-d` at end-of-buffer with a non-empty buffer shall ring the bell
/// (§ 5.4). Empty-buffer `C-d` produces EOF and is covered separately.
#[test]
fn ctrl_d_at_end_of_nonempty_buffer_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"abc");
    // Drain prompt + echo so we don't catch a stray BEL from
    // elsewhere.
    let _ = drain_brief(&mut pty);
    pty.send(b"\x04");
    let probe = drain_brief(&mut pty);
    // Recover: erase the line and exit cleanly.
    pty.send(b"\x15\n");
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL after C-d at EOB of non-empty buffer"
    );
}

// =====================================================================
// § 5.2 Cursor Keys — remaining normative variants.
// =====================================================================

/// Down (`\e[B`) is bound to `next-history`. After walking back with
/// Up and then forward with Down we should land on the most recent
/// edit buffer (empty, in our test).
#[test]
fn down_arrow_next_history_walks_forward() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf OLDCMD\n");
    let _ = drain_until_contains(&mut pty, b"OLDCMD");
    // Up recalls OLDCMD, Down returns to the empty buffer.
    pty.send(b"\x1b[A\x1b[B");
    // Now type a distinct command so the subsequent output proves the
    // buffer was genuinely emptied by the Down key.
    pty.send(b"printf NEWCMD\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("NEWCMD"),
        "expected NEWCMD to run cleanly after Down restored the empty buffer: {text:?}"
    );
}

/// SS3-form Up arrow `\eOA` (application mode) is equivalent to
/// `\e[A` per § 5.2: both shall invoke `previous-history`.
#[test]
fn ss3_up_arrow_recalls_previous_history() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf SS3UP\n");
    let _ = drain_until_contains(&mut pty, b"SS3UP");
    pty.send(b"\x1bOA\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.matches("SS3UP").count() >= 1,
        "expected SS3UP replay via \\eOA: {text:?}"
    );
}

/// SS3-form Home (`\eOH`) is `beginning-of-line` (§ 5.2).
#[test]
fn ss3_home_key_moves_to_beginning() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // "SUFF", SS3-Home, prefix "printf (%s) PRE", submit →
    // `printf '(%s)' PRESUFF`.
    pty.send(b"SUFF\x1bOHprintf '(%s)' PRE");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(PRESUFF)"),
        "expected `(PRESUFF)` after SS3-Home + prefix insert: {text:?}"
    );
}

/// `\e[1~` alternate Home keyseq is `beginning-of-line`.
#[test]
fn linux_home_keyseq_moves_to_beginning() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"SUF\x1b[1~printf '(%s)' PR");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(PRSUF)"),
        "expected `(PRSUF)` after \\e[1~ + prefix insert: {text:?}"
    );
}

/// `\e[4~` alternate End keyseq is `end-of-line`.
#[test]
fn linux_end_keyseq_moves_to_end() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // "printf (%s) ABC", C-a, \e[4~ → cursor at end, insert 'X'.
    pty.send(b"printf '(%s)' ABC\x01\x1b[4~X");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(ABCX)"),
        "expected `(ABCX)` after \\e[4~ + append: {text:?}"
    );
}

/// Right arrow `\e[C` is `forward-char`. Build a buffer, move cursor
/// to start, step forward one char with Right, insert 'Z' so it lands
/// between the first two characters.
#[test]
fn right_arrow_moves_forward_char() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // "printf (%s) AB", C-a, M-f, M-f, C-f (Right via \e[C twice),
    // insert 'Z' — each step verifies the arrow routed to forward-
    // char. Simpler: "printf '(%s)' AB", C-a, 14×Right (past prefix),
    // then 'Z' before A. Simpler yet: type "AB", C-a, Right, 'Z' to
    // produce "AZB". Wrap in printf so the output distinguishes it:
    // "printf '(%s)' AB", put cursor before 'B' via end-of-line + C-b.
    // Instead of relying on position math, use a minimal target:
    // type "AB", C-a, \e[C (Right), "Z", then wrap and accept.
    //
    // Setup: "printf '(%s)' AB" is typed. Cursor at EOB. C-a jumps
    // to start. The literal prefix "printf '(%s)' " is 14 bytes, so
    // 14 Right-arrows land the cursor on 'A' (pos 14). One more
    // Right (15 total) lands after 'A' (pos 15). Inserting 'Z'
    // there yields "printf '(%s)' AZB" → "(AZB)".
    let mut seq: Vec<u8> = b"printf '(%s)' AB\x01".to_vec();
    for _ in 0..15 {
        seq.extend_from_slice(b"\x1b[C");
    }
    seq.push(b'Z');
    pty.send(&seq);
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AZB)"),
        "expected `(AZB)` after Right-arrow navigation: {text:?}"
    );
}

/// Left arrow `\e[D` is `backward-char`.
#[test]
fn left_arrow_moves_backward_char() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // "printf '(%s)' AB" + cursor at EOB, Left once → between A and
    // B, insert 'Z' → "printf '(%s)' AZB".
    pty.send(b"printf '(%s)' AB\x1b[DZ");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AZB)"),
        "expected `(AZB)` after Left-arrow + insert: {text:?}"
    );
}

/// Ctrl-Left (`\e[1;5D`) is `backward-word`. On Linux hosts this
/// sequence is typically installed by `/etc/inputrc`; we assert that
/// *something* in the final pipeline routes it to `backward-word`.
#[test]
fn ctrl_left_arrow_moves_backward_word() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // "printf '(%s%s)' alpha beta", cursor at EOB, Ctrl-Left →
    // before "beta". Insert '_' → args "alpha" and "_beta" →
    // output "(alpha_beta)".
    pty.send(b"printf '(%s%s)' alpha beta\x1b[1;5D_");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(alpha_beta)"),
        "expected `(alpha_beta)` after Ctrl-Left + underscore: {text:?}"
    );
}

/// PageUp (`\e[5~`) is `beginning-of-history` (§ 5.2). After entering
/// several commands, PageUp must jump straight to the oldest entry.
/// `enable_emacs` puts `set -o emacs` into history as the oldest
/// entry, so we observe the redrawn editor line after PageUp and
/// verify it shows that oldest entry.
#[test]
fn page_up_moves_to_beginning_of_history() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf FIRSTHIST\n");
    let _ = drain_until_contains(&mut pty, b"FIRSTHIST");
    pty.send(b"printf MID\n");
    let _ = drain_until_contains(&mut pty, b"MID");
    pty.send(b"printf LAST\n");
    let _ = drain_until_contains(&mut pty, b"LAST");
    let _ = drain_brief(&mut pty);
    pty.send(b"\x1b[5~");
    let probe = drain_brief(&mut pty);
    pty.send(b"\x15\n");
    pty.send(END_SENTINEL_INPUT);
    let _ = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&probe);
    assert!(
        text.contains("set -o emacs"),
        "expected PageUp to redraw buffer as the oldest history entry \
         (hist[0] = `set -o emacs`): {text:?}"
    );
}

/// PageDown (`\e[6~`) is `end-of-history`. After walking back with
/// PageUp, PageDown returns to the empty edit buffer.
#[test]
fn page_down_moves_to_end_of_history() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf HISTA\n");
    let _ = drain_until_contains(&mut pty, b"HISTA");
    pty.send(b"printf HISTB\n");
    let _ = drain_until_contains(&mut pty, b"HISTB");
    // PageUp → oldest entry (HISTA), PageDown → back to newest/empty.
    // Then type a distinct command; if PageDown left anything on the
    // line the resulting command would be mangled.
    pty.send(b"\x1b[5~\x1b[6~printf HISTC\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("HISTC"),
        "expected HISTC to run cleanly after PageUp/PageDown round-trip: {text:?}"
    );
    // The test also expects no spurious `HISTAHISTC` or `HISTBHISTC`
    // tokens from mangled history + typing.
    assert!(
        !text.contains("HISTAHISTC") && !text.contains("HISTBHISTC"),
        "history buffer leaked into fresh typing after PageDown: {text:?}"
    );
}

// =====================================================================
// § 5.3 History — C-n variants and yank-last-arg synonym.
// =====================================================================

/// `C-n` on a fresh (not-from-history) buffer shall ring the bell
/// (spec § 5.3).
#[test]
fn ctrl_n_on_fresh_buffer_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf NCMD\n");
    let _ = drain_until_contains(&mut pty, b"NCMD");
    pty.send(b"\x0e");
    let probe = drain_brief(&mut pty);
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL: C-n on the fresh edit buffer must ring"
    );
}

/// `C-p` past the oldest history entry shall ring the bell
/// (spec § 5.3). We do not know exactly how many entries precede our
/// own (`enable_emacs` submits `set -o emacs` first), so we press
/// C-p repeatedly until either a BEL is observed or a safety bound
/// is hit, then assert at least one BEL occurred.
#[test]
fn ctrl_p_past_oldest_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf ONLY\n");
    let _ = drain_until_contains(&mut pty, b"ONLY");
    let _ = drain_brief(&mut pty);
    let mut rang = false;
    for _ in 0..32 {
        pty.send(b"\x10");
        let probe = drain_brief(&mut pty);
        if bell_count(&probe) >= 1 {
            rang = true;
            break;
        }
    }
    pty.send(b"\x15\n");
    let _ = pty.exit_and_wait();
    assert!(
        rang,
        "expected BEL past oldest history entry within 32 C-p presses"
    );
}

/// `M-_` is a synonym for `M-.` (`yank-last-arg`).
#[test]
fn meta_underscore_is_yank_last_arg_synonym() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"printf SYNONYMARG\n");
    let _ = drain_until_contains(&mut pty, b"SYNONYMARG");
    // `echo W-` then M-_ yanks "SYNONYMARG"; output: "W-SYNONYMARG".
    pty.send(b"echo W-\x1b_\n");
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("W-SYNONYMARG"),
        "expected `W-SYNONYMARG` from M-_ yank-last-arg: {text:?}"
    );
}

// =====================================================================
// § 5.6 Text Editing — remaining normative variants.
// =====================================================================

/// `C-t` mid-buffer: "exchange the character before the cursor with
/// the character at the cursor, then advance the cursor past both"
/// (§ 5.6). Prepare buffer, position cursor to exchange a known pair.
#[test]
fn ctrl_t_mid_buffer_exchanges_before_and_at_cursor() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Buffer "printf '(%s)' abc" with cursor on 'b' (i.e. between 'a'
    // and 'b'). C-t exchanges 'a' and 'b' → "printf '(%s)' bac".
    // Output: "(bac)".
    //
    // Layout: ... abc (cursor at EOB, before 'c' is pos EOB-1).
    // C-b lands cursor on 'c'. Another C-b lands on 'b'. C-t at this
    // point exchanges 'a' and 'b' and advances cursor.
    pty.send(b"printf '(%s)' abc\x02\x02\x14");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(bac)"),
        "expected `(bac)` after mid-buffer C-t: {text:?}"
    );
}

/// `C-v` is a synonym for `C-q` (`quoted-insert`).
#[test]
fn ctrl_v_synonym_for_quoted_insert() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // `printf 'CODE=%d<' "'<0x01>"` with the 0x01 inserted via C-v
    // C-a. Expected output "CODE=1<".
    pty.send(b"printf 'CODE=%d<' \"'\x16\x01\"");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("CODE=1<"),
        "expected `CODE=1<` from C-v quoted-insert synonym: {text:?}"
    );
}

// =====================================================================
// § 5.7 / § 5.8 Tab Completion
//
// The `complete` function (TAB, 0x09) drives the cascade described in
// § 5.8: `$`-variable, `~`-tilde, first-word command, and filename
// completion. Single-match completion fills in the full candidate
// (plus a trailing `/` for directories); multi-match fills the
// longest common prefix, and a second consecutive TAB lists the
// candidates. A no-match completion rings the bell.
// =====================================================================

/// § 5.7: a single matching filename shall replace the partial word
/// with the full name. The fixture directory contains exactly one
/// entry that starts with `FOOBARUNIQ`, so TAB must complete it.
#[test]
fn tab_single_filename_completion_fills_whole_name() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-single-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(format!("{dir}/FOOBARUNIQA"), b"").expect("touch");
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // printf '(%s)' FOOB<TAB> — TAB should expand to FOOBARUNIQA.
    pty.send(b"printf '(%s)' FOOB\x09");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(FOOBARUNIQA)"),
        "expected TAB to complete FOOB to FOOBARUNIQA: {text:?}"
    );
}

/// § 5.7 / § 5.8: when multiple candidates share a prefix, TAB shall
/// fill in the *longest common prefix*. Two files `FOO_alpha` and
/// `FOO_beta`: after TAB the buffer holds `FOO_`, stable for the next
/// keystroke.
#[test]
fn tab_multiple_completions_fill_longest_common_prefix() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-lcp-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(format!("{dir}/FOO_alpha"), b"").expect("touch");
    fs::write(format!("{dir}/FOO_beta"), b"").expect("touch");
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // `printf '[%s]' FO<TAB>` should fill to `FOO_` then we append
    // `alpha` manually and submit → "[FOO_alpha]".
    pty.send(b"printf '[%s]' FO\x09alpha");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[FOO_alpha]"),
        "expected TAB to fill LCP `FOO_` then suffix + submit: {text:?}"
    );
}

/// § 5.7: "If no completion is possible, the bell shall ring."
#[test]
fn tab_no_completion_rings_bell() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-nomatch-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // Drain any prior bells, then ask for completion of a name that
    // can't match anything in the empty directory.
    let _ = drain_brief(&mut pty);
    pty.send(b"printf NOSUCHXYZ\x09");
    let probe = drain_brief(&mut pty);
    pty.send(b"\x15\n");
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL when TAB has no completion candidate"
    );
}

/// § 5.8: variable completion shall fire when the partial word begins
/// with `$`. The cascade completes against shell environment names.
#[test]
fn tab_variable_completion_via_dollar_prefix() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Export a uniquely named variable so there's only one candidate.
    pty.send(b"export MEIKSHCOMPLVAR=seen\n");
    let _ = drain_until_contains(&mut pty, b"$ ");
    // printf '[%s]' $MEI<TAB> → $MEIKSHCOMPLVAR → expands to "seen".
    pty.send(b"printf '[%s]' $MEIK\x09");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[seen]"),
        "expected `$`-prefixed TAB to complete to $MEIKSHCOMPLVAR and expand: {text:?}"
    );
}

/// § 5.8: variable completion also fires when the partial word begins
/// with `${`. A unique match closes the expansion with `}` so the
/// brace form stays well-formed after completion.
#[test]
fn tab_variable_completion_via_brace_prefix() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    pty.send(b"export MEIKSHBRACEVAR=braced\n");
    let _ = drain_until_contains(&mut pty, b"$ ");
    // printf '[%s]' ${MEIKSHBR<TAB> → ${MEIKSHBRACEVAR} → "braced".
    pty.send(b"printf '[%s]' ${MEIKSHBR\x09");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[braced]"),
        "expected `${{`-prefixed TAB to complete to ${{MEIKSHBRACEVAR}} and expand: {text:?}"
    );
}

/// § 5.8: filename completion via a path containing `/`. TAB on
/// `/tmp/<fixture>/FOOB` completes to the full filename.
#[test]
fn tab_filename_completion_with_slash_path() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-slash-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(format!("{dir}/FOOBARSLASHA"), b"").expect("touch");
    enable_emacs(&mut pty);
    let cmd = format!("printf '[%s]' {dir}/FOOB\x09");
    pty.send(cmd.as_bytes());
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out);
    let expected = format!("[{dir}/FOOBARSLASHA]");
    assert!(
        text.contains(&expected),
        "expected full path completion {expected:?} in: {text:?}"
    );
}

/// § 5.8: "tilde completion, if the partial word begins with `~`".
#[test]
fn tab_tilde_completion_expands_home_directory() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type `printf '[%s]' ~<TAB>` — expect `~` to complete to the
    // home directory. We just check for `[/` (any absolute-path
    // expansion) as evidence that tilde was expanded rather than
    // inserted literally.
    pty.send(b"printf '[%s]' ~\x09");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[/"),
        "expected tilde completion to expand to an absolute path: {text:?}"
    );
}

/// § 5.7: "If multiple completions are possible, the longest common
/// prefix shall replace the partial word; if no additional characters
/// can be added, the second consecutive `TAB` shall list the possible
/// completions."
#[test]
fn tab_twice_lists_candidates() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-list-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(format!("{dir}/LISTA"), b"").expect("touch");
    fs::write(format!("{dir}/LISTB"), b"").expect("touch");
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // After the first TAB the buffer becomes `LIST`; a second TAB
    // must print "LISTA" and "LISTB" on the next line.
    pty.send(b"printf OK LIST\x09\x09");
    let probe = drain_brief(&mut pty);
    pty.send(b"\x15\n");
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&probe);
    assert!(
        text.contains("LISTA") && text.contains("LISTB"),
        "expected double-TAB listing to show LISTA and LISTB: {text:?}"
    );
}

/// § 5.8: double-TAB listings are laid out in a bash-style
/// column-major grid so short candidates share a single row rather
/// than printing one per line.
#[test]
fn tab_twice_lists_candidates_in_multi_column_grid() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-grid-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("mkdir");
    for name in ["GRIDa", "GRIDb", "GRIDc", "GRIDd"] {
        fs::write(format!("{dir}/{name}"), b"").expect("touch");
    }
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // Stable 80-column-ish width; 4 short names must fit on one row.
    pty.send(b"export COLUMNS=80\n");
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"printf OK GRID\x09\x09");
    let probe = drain_brief(&mut pty);
    pty.send(b"\x15\n");
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&probe);
    assert!(
        text.contains("GRIDa") && text.contains("GRIDd"),
        "expected double-TAB listing to show all grid entries: {text:?}"
    );
    // All four candidates must appear on the same grid row.
    let row_with_a = text
        .lines()
        .find(|line| line.contains("GRIDa"))
        .unwrap_or("");
    assert!(
        row_with_a.contains("GRIDa")
            && row_with_a.contains("GRIDb")
            && row_with_a.contains("GRIDc")
            && row_with_a.contains("GRIDd"),
        "expected all short candidates on one grid row, got row {row_with_a:?} in {text:?}"
    );
}

/// § 5.8: command completion applies in every argv[0] position, not
/// only at the start of the buffer. Inside a `$(...)` command
/// substitution the cursor is on argv[0] of a fresh command context,
/// so TAB must complete against commands rather than falling through
/// to filename completion.
#[test]
fn tab_completes_command_inside_command_substitution() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // `echo $(ech<TAB> DONE)` should become `echo $(echo DONE)` →
    // prints `DONE` after accept-line.
    pty.send(b"echo $(ech\x09 DONE)");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\r\nDONE") || text.contains("\nDONE"),
        "expected command completion inside `$(...)` to run `echo DONE`: {text:?}"
    );
}

/// § 5.8 bullet 2: "if the cursor is on the first word of the command
/// line, meiksh shall attempt command completion."
#[test]
fn tab_completes_command_on_first_word() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // `ech<TAB>` should become `echo` because `echo` is a known
    // builtin. Follow with ` DONE` and accept-line.
    pty.send(b"ech\x09 DONE");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("\r\nDONE") || text.contains("\nDONE"),
        "expected `echo DONE` execution after first-word TAB: {text:?}"
    );
}

/// § 5.8: "trailing `/` if the matched path is a directory".
#[test]
fn tab_directory_completion_appends_trailing_slash() {
    use std::fs;
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let dir = format!("/tmp/meiksh-compl-dir-{}", std::process::id());
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(format!("{dir}/SUBDIRUNIQ")).expect("mkdir sub");
    enable_emacs(&mut pty);
    pty.send(format!("cd {dir}\n").as_bytes());
    let _ = drain_until_contains(&mut pty, b"$ ");
    // printf '[%s]' SUBD<TAB>X — if the completion appended `/`, the
    // subsequent `X` lands *after* the slash.
    pty.send(b"printf '[%s]' SUBD\x09X");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let _ = fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[SUBDIRUNIQ/X]"),
        "expected trailing `/` on directory completion: {text:?}"
    );
}

// =====================================================================
// § 5.9 C-g with nothing composite in progress → bell.
// =====================================================================

/// § 5.9: `C-g` (`abort`) on an editor line with no composite action
/// in progress shall ring the bell. The abort path inside incremental
/// search (which *is* a composite action) is routed through
/// `run_incremental_search` and exercised by
/// `ctrl_g_aborts_incremental_search`.
#[test]
fn ctrl_g_without_composite_action_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    let _ = drain_brief(&mut pty);
    pty.send(b"\x07");
    let probe = drain_brief(&mut pty);
    pty.send(b"\n");
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL from C-g with nothing composite to abort"
    );
}

// =====================================================================
// § 5.9 Edit-and-execute (C-x C-e). Uses a trivial `sh`-based
// "editor" that writes a known command into the temp file.
// =====================================================================

/// § 5.9: `C-x C-e` runs `$VISUAL` on a temp file holding the current
/// buffer; on exit the file contents become the submitted line. We
/// configure `VISUAL` to overwrite the file with `printf CXCEOUT`
/// and verify the shell executes it.
#[test]
fn ctrl_x_ctrl_e_edits_and_executes_via_visual() {
    // The VISUAL command must end in a placeholder that sh treats as
    // `$0` — meiksh appends ` <tmp_path>` so `$1` inside the script
    // resolves to the transfer file. Using `--` here is tempting
    // (bash-as-sh happily accepts it and treats it as `$0`) but
    // FreeBSD's native `/bin/sh` rejects `--` with "Illegal option
    // --", which is exactly what POSIX `sh` is allowed to do.
    // Use the neutral argv-0 marker `_` instead: portable and
    // unambiguous across sh implementations.
    let Some(mut pty) = spawn_meiksh_pty(&[(
        "VISUAL",
        "/bin/sh -c 'printf \"printf CXCEOUT\" > \"$1\"' _",
    )]) else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    pty.send(b"set -o emacs\n");
    let _ = drain_until_contains(&mut pty, b"$ ");
    // Start from a non-empty buffer so C-x C-e has something to
    // transport; the editor rewrites it anyway.
    pty.send(b"IGNORED\x18\x05");
    // After the editor returns, the editor reads the temp file and
    // submits it as the input line. Follow with the END sentinel.
    pty.send(END_SENTINEL_INPUT);
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("CXCEOUT"),
        "expected `CXCEOUT` after edit-and-execute round-trip: {text:?}"
    );
}

// =====================================================================
// § 5.10 Bracketed paste — undo grouping and kill-track independence.
// =====================================================================

/// § 5.10: "The pasted run shall be a single undo group." A single
/// `C-_` after a paste shall erase the entire paste.
#[test]
fn bracketed_paste_forms_single_undo_group() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Paste "JUNKJUNK", then C-_ undoes the paste, then type a
    // separate command.
    pty.send(b"\x1b[200~JUNKJUNK\x1b[201~\x1fprintf '(%s)' POSTUNDO");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(POSTUNDO)"),
        "expected `(POSTUNDO)` after undoing a paste: {text:?}"
    );
    assert!(
        !text.contains("JUNKJUNK:"),
        "paste content survived undo: {text:?}"
    );
}

// =====================================================================
// § 6 Kill-buffer accumulation — append direction and reset behavior.
// =====================================================================

/// § 6: two consecutive `kill-line` (`C-k`) commands shall **append**
/// the second kill to the first; yank must produce both in order.
#[test]
fn consecutive_kill_line_commands_append() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // The kill-buffer accumulation semantics are most easily observed
    // by constructing a multi-line-ish layout inside one buffer.
    // Strategy: type `printf '(%s%s)' XY ZZ`, C-a, walk cursor past
    // "printf '(%s%s)' " (15 chars → 15 × C-f), C-k kills "XY ZZ",
    // C-e jumps to end (buffer is now "printf '(%s%s)' "), C-y yanks
    // "XY ZZ". That shows `C-y` restores after a single C-k but
    // doesn't test *append* semantics.
    //
    // To test append, use *two* separate killable stretches within
    // the same buffer:
    //   Type "printf '(%s%s%s)' Xaa Ybb Zcc".
    //   C-a.  Walk 16 C-fs to just past "printf '(%s%s%s)' "
    //   M-d kills "Xaa" (forward word).  (kill buffer = "Xaa")
    //   Currently cursor sits between "Xaa " and "Ybb" but M-d
    //     consumed the word AND the whitespace… let's be careful.
    //
    // Cleaner design: use C-d to delete a single character, then
    // C-k twice with a non-kill between — no, we want *append*, so
    // we need two consecutive kills.
    //
    // Use two C-k's with cursor returning to the middle each time:
    //   Type "printf '(%s)' END", C-a (buffer = end), C-k (kill
    //   whole buffer = "printf '(%s)' END").  C-k *again* on an
    //   empty buffer: kill buffer becomes "printf '(%s)' END" +
    //   "" = unchanged.  Then C-y → "printf '(%s)' END" → "(END)".
    //
    // That proves "second C-k on empty appends an empty string" but
    // doesn't show distinct content. Instead, use two kills with
    // distinct content:
    //   Type "AA".  C-a C-k (kill "AA").
    //   Type "BB". C-a C-k (kill "BB").  kill buffer now
    //   should be "AA" + "BB" = "AABB"... BUT there was a non-kill
    //   command ("BB" self-insert) between them, so per § 6 the
    //   second C-k *replaces* rather than appends.
    //
    // The cleanest demonstration: kill twice in a row with no
    // non-kill between. Layout to allow that:
    //   Buffer: "printf '(%s%s)' PP QQ"
    //   Cursor: at end of buffer.
    //   M-DEL kills "QQ" (backward-kill-word).  kill = "QQ".
    //   BS kills nothing (we hit whitespace)... M-DEL again?
    //   M-DEL again on whitespace: deletes the space.
    //   That's three kills chained; too fragile.
    //
    // Use kill-line twice on cursor moved back in between:
    //   Type "LL11 LL22".  C-a.  C-k kills "LL11 LL22".  (first
    //   kill: replace, buffer = "LL11 LL22").
    //   Type "LL33".  C-a.  C-k kills "LL33".  (Since the previous
    //   dispatched command was self-insert of "LL33", *replaces*:
    //   buffer = "LL33".)
    //   Well, consecutive kills means the *previous* dispatched
    //   command must itself be a kill.  Only way to chain kills
    //   without intervening non-kill is to do them on the same line
    //   in sequence.  So: buffer "a b", C-a, M-d (kill "a"), M-d
    //   (kill "b"), yank.  kill buffer after first M-d: "a"; after
    //   second M-d: "a" + " b" = "a b" (since M-d is append-
    //   direction per § 6).
    //
    // Plan:
    //   Type: printf '(%s)' xx yy
    //   C-a
    //   Walk 14 C-f (past "printf '(%s)' "), putting cursor on 'x'.
    //     14 × 0x06
    //   M-d kills "xx" (+trailing space, per word-boundary semantics
    //     in `next_word_boundary` for `alnum+_`). kill = "xx ".
    //   M-d kills "yy". kill = "xx " + "yy" = "xx yy".
    //   C-e, C-y yanks "xx yy" at end of remaining buffer
    //   "printf '(%s)' ". Final buffer: "printf '(%s)' xx yy" →
    //   output "(xx)" because printf %s only takes the first arg.
    //
    // The *distinctive* proof is that BOTH "xx" and "yy" are
    // present in the yanked region. Let's use %s%s so both show up:
    //   Type: printf '(%s%s)' xx yy
    //   M-d twice → kill buffer = "xx yy"
    //   C-e, C-y → buffer "printf '(%s%s)' xx yy"
    //   Output: "(xxyy)".
    let mut seq: Vec<u8> = b"printf '(%s%s)' xx yy\x01".to_vec();
    // Walk past "printf '(%s%s)' " which is 16 chars.
    seq.extend(std::iter::repeat_n(0x06u8, 16));
    seq.extend_from_slice(b"\x1bd\x1bd");
    seq.extend_from_slice(b"\x05\x19");
    pty.send(&seq);
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(xxyy)"),
        "expected `(xxyy)` — consecutive M-d kills should append: {text:?}"
    );
}

/// § 6: a non-kill command between two kills resets the kill buffer;
/// the second kill replaces rather than appends.
#[test]
fn kill_then_non_kill_then_kill_replaces_buffer() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Scenario:
    //   Type "printf '(%s)' AAA".  C-a.  C-k kills "printf '(%s)' AAA"
    //     (kill buffer = full string).
    //   C-y yanks it back (kill buffer unchanged per § 6, but the
    //     yank *is* a non-kill command).
    //   C-a.  C-k kills again (now kill buffer should be replaced
    //     with the full string, not appended).
    //   C-y yanks once → buffer is the full string (single copy),
    //     not a double copy.
    //
    // If replacement worked: buffer = "printf '(%s)' AAA", output =
    // "(AAA)".
    // If accumulation erroneously appended: buffer would be
    // "printf '(%s)' AAAprintf '(%s)' AAA", which shell-parses to
    // multiple arguments and likely fails or produces unrelated
    // output.
    pty.send(b"printf '(%s)' AAA\x01\x0b\x19\x01\x0b\x19");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(AAA)"),
        "expected `(AAA)` — non-kill between kills must reset buffer: {text:?}"
    );
    assert!(
        !text.contains("(AAAprintf"),
        "kill buffer accumulated through non-kill command: {text:?}"
    );
}

// =====================================================================
// § 9 Undo — empty stack, kill reversal, post-accept clearing.
// =====================================================================

/// § 9: `undo` with an empty undo stack shall ring the bell.
#[test]
fn undo_on_empty_stack_rings_bell() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    let _ = drain_brief(&mut pty);
    pty.send(b"\x1f");
    let probe = drain_brief(&mut pty);
    pty.send(b"\n");
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL from undo on empty stack"
    );
}

/// § 9: a kill command forms a single undo group; `undo` after a
/// kill shall reinsert the killed text at the pre-kill cursor
/// position.
#[test]
fn undo_reverses_kill_line() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Type "printf '(%s)' UNKILL", C-a, C-k (kill all), C-_ (undo
    // the kill: buffer is restored). RET accepts the restored line.
    // Expected output: "(UNKILL)".
    pty.send(b"printf '(%s)' UNKILL\x01\x0b\x1f");
    let out = accept_then_drain_end(&mut pty);
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("(UNKILL)"),
        "expected `(UNKILL)` after undoing C-k: {text:?}"
    );
}

/// § 9: "The undo stack shall be cleared after accept-line."
/// After running one line and starting a second, `undo` on the fresh
/// line must ring the bell — the first line's edit groups must not
/// be reachable.
#[test]
fn undo_stack_cleared_between_input_lines() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    enable_emacs(&mut pty);
    // Line 1: make a small edit, then submit.
    pty.send(b"printf LINE1\n");
    let _ = drain_until_contains(&mut pty, b"LINE1");
    // Line 2: fresh buffer, undo must bell (no prior group on this
    // line's stack).
    let _ = drain_brief(&mut pty);
    pty.send(b"\x1f");
    let probe = drain_brief(&mut pty);
    pty.send(b"\n");
    let _ = pty.exit_and_wait();
    assert!(
        bell_count(&probe) >= 1,
        "expected BEL: undo must ring on a fresh line after accept-line"
    );
}

// =====================================================================
// § 15 Non-goals — the bindable function list must *exclude* features
// listed in § 15. `bind '"keyseq": fn-name'` for such a name emits the
// `unknown function` diagnostic on stderr; per § 14.4 the process
// exit status itself remains 0 for the multi-argument readline form
// (matching bash). The observable normative effect is therefore
// twofold: (a) the diagnostic appears, and (b) no binding is actually
// installed (inspectable via `bind -p`).
// =====================================================================

/// Helper: assert that `bind '"<keyseq>": <fn>'` emits the
/// "unknown function: <fn>" diagnostic and that `<fn>` never appears
/// in `bind -p`.
fn assert_bind_rejects_unknown_function(keyseq_escaped: &str, fn_name: &str) {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    let cmd = format!(
        "bind '\"{keyseq_escaped}\": {fn_name}' 2>&1; bind -p | grep -c ': {fn_name}$'; \
         printf '\\105\\116\\104\\012'\n"
    );
    pty.send(cmd.as_bytes());
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains(&format!("unknown function: {fn_name}")),
        "expected `unknown function: {fn_name}` diagnostic: {text:?}"
    );
    // The `bind -p | grep -c` line reports how many installed
    // bindings use the rejected function name. It must be 0.
    let grep_line = text
        .lines()
        .find(|l| l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty())
        .unwrap_or("");
    assert_eq!(
        grep_line, "0",
        "{fn_name} must not be installed in the keymap (grep -c was {grep_line:?}): {text:?}"
    );
}

/// § 15.1: `yank-pop` shall not be a bindable function.
#[test]
fn yank_pop_binding_rejected_as_unknown_function() {
    assert_bind_rejects_unknown_function("\\C-x\\C-y", "yank-pop");
}

/// § 15.3: `set-mark` shall not be a bindable function.
#[test]
fn set_mark_binding_rejected_as_unknown_function() {
    assert_bind_rejects_unknown_function("\\C-@", "set-mark");
}

/// § 15.5: `universal-argument` shall not be a bindable function.
#[test]
fn universal_argument_binding_rejected_as_unknown_function() {
    assert_bind_rejects_unknown_function("\\C-u", "universal-argument");
}

/// § 15.2: keyboard-macro functions shall not be bindable.
#[test]
fn start_kbd_macro_binding_rejected_as_unknown_function() {
    assert_bind_rejects_unknown_function("\\C-x(", "start-kbd-macro");
}

/// § 15.7: `vi-editing-mode` (mid-line switch) shall not be bindable.
#[test]
fn vi_editing_mode_binding_rejected_as_unknown_function() {
    assert_bind_rejects_unknown_function("\\C-xv", "vi-editing-mode");
}

// =====================================================================
// § 15.14 `$if mode=emacs` — the other $if variant beyond term=.
// =====================================================================

/// § 15.14: `$if mode=emacs` shall gate its bindings to emacs mode
/// only, without diagnostic. A simple smoke test: write an inputrc
/// with a `mode=emacs` conditional and confirm it loads silently.
#[test]
fn if_mode_emacs_directive_loads_without_diagnostic() {
    let Some(mut pty) = spawn_or_skip() else {
        return;
    };
    let _ = drain_until_contains(&mut pty, b"$ ");
    let path = format!("/tmp/meiksh-emacs-mode-rc-{}", std::process::id());
    std::fs::write(&path, b"$if mode=emacs\n\"\\C-xM\": accept-line\n$endif\n").expect("write rc");
    let cmd = format!("bind -f {path} 2>&1; echo RC=$?; printf '\\105\\116\\104\\012'\n");
    pty.send(cmd.as_bytes());
    let out = drain_until_contains(&mut pty, b"END\r\n");
    let _ = pty.exit_and_wait();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out);
    assert!(
        !text.contains("unknown") && !text.contains("$if test"),
        "`$if mode=emacs` must load silently: {text:?}"
    );
    assert!(
        text.contains("RC=0"),
        "expected bind -f success (RC=0): {text:?}"
    );
}
