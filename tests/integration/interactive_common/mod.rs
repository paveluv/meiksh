//! Safe PTY harness used by interactive integration tests.
//!
//! Individual tests rely on this module so they remain free of `unsafe`
//! blocks and direct FFI. Every libc call lives in
//! `tests/integration/sys.rs`; this file's only `unsafe` block is the
//! `pre_exec` trampoline, which is mandated by the `CommandExt` trait
//! contract regardless of whether the closure itself performs any
//! unsafe work.

#![allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    dead_code
)]

use super::common::meiksh;
use super::sys;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Interactive PTY tests share a single process-wide mutex to keep
/// them from contending for the host's scarce PTY slots when cargo's
/// test harness runs them in parallel with the rest of the suite.
/// Every entry point that spawns a `meiksh -i` PTY acquires this
/// guard and hands it to [`PtyChild`]; it is released when the guard
/// (and therefore the child) are dropped.
fn pty_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// A child `meiksh -i` process attached to a pseudo-terminal, with an
/// owning handle to the primary (driver) side so tests can read and
/// write as if they were the user's terminal.
pub struct PtyChild {
    child: Child,
    primary: std::fs::File,
    // Held for the lifetime of the child so concurrent PTY tests
    // serialize around the host's PTY slots.
    _lock: MutexGuard<'static, ()>,
}

/// Spawn `meiksh -i` on a fresh PTY. Returns `None` when the host lacks
/// PTY support (some CI sandboxes); callers should treat that as a
/// skip, matching the existing `spawn_pty_meiksh` convention.
///
/// `extra_env` lets tests inject environment overrides (for example
/// `HOME=<tmpdir>` or `LC_ALL=C.UTF-8`). `TERM` defaults to `"dumb"` so
/// no escape sequences from the prompt layer leak into assertions;
/// tests can still override it via `extra_env`.
pub fn spawn_meiksh_pty(extra_env: &[(&str, &str)]) -> Option<PtyChild> {
    let lock = match pty_test_lock().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (primary, secondary) = sys::open_pty_pair()?;
    let secondary_fd = secondary;
    let stdout_fd = sys::dup_fd(secondary_fd).ok()?;
    let stderr_fd = sys::dup_fd(secondary_fd).ok()?;

    let mut cmd = Command::new(meiksh());
    cmd.arg("-i")
        .env("TERM", "dumb")
        .env("LC_ALL", "C.UTF-8")
        .stdin(unsafe { Stdio::from_raw_fd(secondary_fd) })
        .stdout(unsafe { Stdio::from_raw_fd(stdout_fd) })
        .stderr(unsafe { Stdio::from_raw_fd(stderr_fd) });
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    unsafe {
        cmd.pre_exec(move || sys::make_controlling_tty_in_child(secondary_fd, false));
    }
    let child = cmd.spawn().ok()?;
    let primary = unsafe { std::fs::File::from_raw_fd(primary) };
    Some(PtyChild {
        child,
        primary,
        _lock: lock,
    })
}

impl PtyChild {
    /// Raw file descriptor of the primary side. Exposed for tests that
    /// want to toggle non-blocking mode or inspect the fd directly.
    pub fn primary_fd(&self) -> RawFd {
        std::os::fd::AsRawFd::as_raw_fd(&self.primary)
    }

    /// Write bytes to the child's terminal. Short writes are tolerated
    /// with retries; the test will fail if the write cannot be
    /// completed in a reasonable amount of time.
    pub fn send(&mut self, bytes: &[u8]) {
        self.primary.write_all(bytes).expect("write to PTY");
        let _ = self.primary.flush();
    }

    /// Drain everything the child has produced on its standard output
    /// (and stderr, since both are wired to the PTY) up to `dur` from
    /// now. Returns whatever was collected, even on timeout.
    pub fn drain_for(&mut self, dur: Duration) -> Vec<u8> {
        let deadline = Instant::now() + dur;
        self.drain_common(deadline, |_| false)
    }

    /// Drain until `pred` returns true over the accumulated buffer, or
    /// the timeout expires. Whatever was collected is returned either
    /// way so the test can emit a descriptive assertion message.
    pub fn drain_until(&mut self, pred: impl Fn(&[u8]) -> bool, timeout: Duration) -> Vec<u8> {
        let deadline = Instant::now() + timeout;
        self.drain_common(deadline, pred)
    }

    fn drain_common(&mut self, deadline: Instant, pred: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        let fd = self.primary_fd();
        let _ = sys::set_nonblocking(fd, true);
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // Short poll interval: PTY roundtrips commonly resolve in
        // 100µs-5ms, so a 2ms sleep on WouldBlock keeps us within ~2ms
        // of any arriving byte without busy-spinning the host. This
        // directly trims multi-hundreds-of-milliseconds off a test
        // suite of ~15 PTY roundtrips.
        let poll_sleep = Duration::from_millis(2);
        while Instant::now() < deadline {
            match self.primary.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if pred(&buf) {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if pred(&buf) {
                        break;
                    }
                    thread::sleep(poll_sleep);
                }
                Err(_) => break,
            }
        }
        buf
    }

    /// Consume the child and wait for exit. The caller loses the handle.
    pub fn wait(mut self) -> ExitStatus {
        self.child.wait().expect("wait for meiksh -i")
    }

    /// Wait for the child to exit, or kill it after `timeout`. Returns
    /// `Some(status)` if the child exited on its own, `None` on
    /// timeout (after the child has been killed and reaped).
    pub fn wait_with_timeout(mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(_) => return None,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        None
    }

    /// Ask the shell to `exit` and wait for the child to finish,
    /// with a bounded wall-clock budget so a misbehaving shell can't
    /// stall the test suite forever. `timeout` applies to the
    /// post-`exit` wait; exceeding it kills the child, reaps it, and
    /// returns `None`.
    pub fn exit_and_wait_with_timeout(mut self, timeout: Duration) -> Option<ExitStatus> {
        let _ = self.primary.write_all(b"exit\n");
        let _ = self.primary.flush();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(_) => return None,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        None
    }

    /// Ask the shell to `exit` and wait for the child to finish.
    /// Defaults to a 5-second budget — interactive shells respond to
    /// EOF in milliseconds; anything longer indicates a test bug.
    pub fn exit_and_wait(self) -> ExitStatus {
        match self.exit_and_wait_with_timeout(Duration::from_secs(5)) {
            Some(status) => status,
            None => panic!("meiksh -i did not exit within 5s after receiving `exit\\n`"),
        }
    }
}

impl Drop for PtyChild {
    fn drop(&mut self) {
        // Best-effort cleanup: kill the child if the test didn't wait
        // for it explicitly. Errors are swallowed because teardown
        // races are expected (child may already have exited).
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
