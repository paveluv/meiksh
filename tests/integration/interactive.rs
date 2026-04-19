use super::common::*;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

// Integration tests that drive `meiksh -i` over a real PTY. The shell
// only activates its vi-mode line editor, terminal-mode wrappers
// (`tcgetattr` / `tcsetattr`), and the locale wrappers used by
// rendering (`encode_char`, `to_upper`, `to_lower`, `char_width`) when
// stdin is an interactive terminal, so these paths cannot be reached
// from the standard `-c` tests in the rest of this suite.
//
// All helpers and fixtures that involve `openpty(3)` and raw terminal
// handling live here so the simpler tests in sibling modules stay
// unencumbered by terminal setup code.

/// Drain everything the PTY has produced up to `deadline`, short-
/// circuiting on EOF (child exit) or a hard error. The fd is returned
/// to the caller via `into_raw_fd` so that ownership is not consumed.
fn drain_pty(fd: RawFd, deadline: Instant) -> Vec<u8> {
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    while Instant::now() < deadline {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
    let _ = file.into_raw_fd();
    buf
}

/// Open a PTY and spawn `meiksh -i` with the secondary side wired up
/// as the controlling terminal. Returns `None` when the host has no
/// `/dev/pts` (some CI sandboxes); the caller should treat that as a
/// skip rather than a failure.
fn spawn_pty_meiksh(env_locale: &str) -> Option<(std::process::Child, RawFd)> {
    let mut primary: i32 = -1;
    let mut secondary: i32 = -1;
    let rc = unsafe {
        libc::openpty(
            &mut primary,
            &mut secondary,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return None;
    }
    let secondary_fd = secondary;
    let mut cmd = Command::new(meiksh());
    cmd.env("LC_ALL", env_locale)
        .env("TERM", "dumb")
        .arg("-i")
        .stdin(unsafe { Stdio::from_raw_fd(secondary_fd) })
        .stdout(unsafe { Stdio::from_raw_fd(libc::dup(secondary_fd)) })
        .stderr(unsafe { Stdio::from_raw_fd(libc::dup(secondary_fd)) });
    unsafe {
        cmd.pre_exec(move || {
            libc::setsid();
            libc::ioctl(secondary_fd, libc::TIOCSCTTY as _, 0);
            Ok(())
        });
    }
    let child = cmd.spawn().expect("spawn meiksh -i on pty");
    // Note: we do NOT explicitly `close(secondary)` here — the three
    // `Stdio::from_raw_fd` handles now own the secondary fd (stdin)
    // plus its two dup'd clones (stdout/stderr). Rust will close them
    // all when the `Command` is dropped; closing the raw fd ourselves
    // would trigger the "owned file descriptor already closed" IO-
    // safety abort.
    Some((child, primary))
}

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
    let Some((mut child, primary)) = spawn_pty_meiksh("C.UTF-8") else {
        // Host has no PTY support — nothing to assert on in that case.
        return;
    };

    let mut master = unsafe { std::fs::File::from_raw_fd(primary) };
    master.write_all(b"set -o vi\n").expect("write set -o vi");
    thread::sleep(Duration::from_millis(100));
    // Insert `hElLo`, Esc → normal mode, `0` → beginning of line,
    // `5~` → toggle five characters, Enter → submit.
    master
        .write_all(b"hElLo\x1b05~\n")
        .expect("write vi editing sequence");
    thread::sleep(Duration::from_millis(100));
    master.write_all(b"exit\n").expect("write exit");
    let _ = master.flush();

    let deadline = Instant::now() + Duration::from_secs(5);
    let output = drain_pty(primary, deadline);
    let _ = child.wait();

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("HeLlO"),
        "expected case-flipped token `HeLlO` in PTY transcript, got: {text:?}",
    );

    drop(master);
}
