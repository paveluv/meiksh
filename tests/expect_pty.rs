#![allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
//! expect_pty — A scriptable PTY driver and test suite runner.
//!
//! ## Legacy mode (backward compatible)
//!
//! Reads a line-oriented conversation script from stdin (or a single file):
//!
//!   expect_pty [script.txt]
//!
//! ## Suite mode (.epty files)
//!
//! Runs structured test suites with isolated environments:
//!
//!   expect_pty --shell "/usr/bin/bash --posix" tests/*.epty
//!   expect_pty --shell "/usr/bin/bash --posix" --script-modes dash-c,tempfile,stdin tests/*.epty
//!   expect_pty --shell "/usr/bin/bash --posix" --test "name" tests/*.epty
//!   expect_pty --shell "/usr/bin/bash --posix" --verbose tests/*.epty
//!
//! ### Suite DSL:
//!   testsuite "name"                      — suite name (one per file)
//!   requirement "ID" doc="..."            — POSIX requirement reference
//!   begin interactive test "name"         — start an interactive (PTY) test case
//!   begin test "name"                     — start a non-interactive test case
//!   end interactive test "name"           — end an interactive test case
//!   end test "name"                       — end a non-interactive test case
//!   setenv "KEY" "VALUE"                  — set env var for the current test
//!
//! ### Non-interactive script and expect blocks (inside begin/end test):
//!   script                                — start script block (col 2, body at col 4)
//!     <shell code>                        — script body (stripped to col 0)
//!   expect                                — start expect block (col 2, assertions at col 4)
//!     stdout "pattern"                    — assert stdout matches regex (full match)
//!     stderr "pattern"                    — assert stderr matches regex (full match)
//!     exit_code <expr>                    — assert exit code satisfies expression
//!
//!   All three assertions required in order: stdout, stderr, exit_code.
//!   The script body is taken verbatim (no quoting/escaping needed).
//!   $SHELL is set to the --shell value. Executed via --script-modes (default: dash-c).
//!   Tests run in an isolated sandbox directory; prefer local relative file paths
//!   (for example, `_tmp_file`) instead of `${TMPDIR:-/tmp}` indirections, and
//!   do not add explicit cleanup-only commands unless cleanup behavior itself is tested.
//!
//! ### Interactive (PTY) commands (inside begin/end interactive test):
//!   spawn [flags...]                       — fork an interactive shell (shell from --shell, flags appended)
//!   expect "regex"                        — wait for regex match in PTY output
//!   expect timeout=2s "regex"             — with per-command timeout
//!   expect_line "regex"                   — wait for matching line
//!   expect_line timeout=1s "regex"        — with per-command timeout
//!   send "text"                           — write text + newline to PTY
//!   sendraw <hex> [<hex>...]              — write raw bytes to PTY
//!   signal <SIGNAME>                      — send signal to child
//!   sendeof                               — send EOF to PTY
//!   wait exitcode=N                       — wait for child exit
//!   sleep <duration>                      — sleep (e.g. 100ms or 1s)
//!
//! ### Formatting rules:
//! - Trailing whitespace is forbidden on any line.
//! - Lines starting with '#' and empty lines are ignored (outside script blocks).
//! - Quoted strings in send/setenv support backslash escapes: \" \\ \n \r \t
//! - Regex patterns in expect/expect_*/not_expect_* use raw quoting: backslash
//!   has no special meaning and passes through verbatim to the regex engine.
//!   To embed a literal double-quote in a pattern, use "" (doubled quote).
//! - $SHELL env var is set to the --shell value (including flags, e.g. "/usr/bin/bash --posix").
//! - All pattern matching uses the built-in regex engine (no external deps).
//! - Use --verbose (-v) to emit detailed diagnostics to stderr (PTY spawn, expect
//!   buffer state, cleanup steps, per-test isolation progress).

#[path = "epty_parser.rs"]
mod epty_parser;
#[path = "json.rs"]
#[allow(dead_code)]
mod json;
#[path = "md_parser.rs"]
mod md_parser;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

static VERBOSE: AtomicBool = AtomicBool::new(false);

fn verbose() -> bool {
    VERBOSE.load(AtomicOrdering::Relaxed)
}

macro_rules! vlog {
    ($($arg:tt)*) => {
        if VERBOSE.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("[verbose] {}", format!($($arg)*));
        }
    };
}

// ── FFI types and functions ──────────────────────────────────────────────────

type CInt = libc::c_int;

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

unsafe extern "C" {
    fn forkpty(
        amaster: *mut CInt,
        name: *mut libc::c_char,
        termp: *mut libc::termios,
        winp: *mut Winsize,
    ) -> libc::pid_t;
}

fn wifexited(status: CInt) -> bool {
    libc::WIFEXITED(status)
}

fn wexitstatus(status: CInt) -> i32 {
    libc::WEXITSTATUS(status)
}

/// Kill every process whose session ID equals `sid`.
/// Since forkpty() makes the child the session leader, `sid` == child PID.
fn kill_session(sid: libc::pid_t) {
    vlog!("kill_session: sending SIGKILL to process group -{sid}");
    let ret = unsafe { libc::kill(-sid, libc::SIGKILL) };
    vlog!(
        "kill_session: kill(-{sid}, SIGKILL) returned {ret} (errno={})",
        if ret < 0 {
            io::Error::last_os_error().to_string()
        } else {
            "n/a".into()
        }
    );

    let mut killed = 0;

    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Ok(pid) = name.to_string_lossy().parse::<libc::pid_t>() else {
                    continue;
                };
                if pid <= 1 {
                    continue;
                }
                let got_sid = unsafe { libc::getsid(pid) };
                if got_sid == sid {
                    vlog!("kill_session: killing pid={pid} (sid={got_sid})");
                    unsafe {
                        libc::kill(pid, libc::SIGKILL);
                    }
                    killed += 1;
                }
            }
        }
    }

    #[cfg(target_os = "freebsd")]
    {
        killed = kill_session_sysctl(sid);
    }

    vlog!("kill_session: killed {killed} session members");
}

#[cfg(target_os = "freebsd")]
fn kill_session_sysctl(sid: libc::pid_t) -> usize {
    use std::mem;

    let mib: [libc::c_int; 4] = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_SESSION,
        sid as libc::c_int,
    ];

    let mut buf_len: libc::size_t = 0;
    let ret = unsafe {
        libc::sysctl(
            mib.as_ptr(),
            mib.len() as libc::c_uint,
            std::ptr::null_mut(),
            &mut buf_len,
            std::ptr::null(),
            0,
        )
    };
    if ret != 0 || buf_len == 0 {
        vlog!(
            "kill_session_sysctl: sysctl size query failed (ret={ret}, errno={})",
            io::Error::last_os_error()
        );
        return 0;
    }

    buf_len = buf_len * 3 / 2;
    let kinfo_size = mem::size_of::<libc::kinfo_proc>();
    let count = buf_len / kinfo_size;
    let mut buf: Vec<libc::kinfo_proc> = Vec::with_capacity(count);

    let ret = unsafe {
        libc::sysctl(
            mib.as_ptr(),
            mib.len() as libc::c_uint,
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut buf_len,
            std::ptr::null(),
            0,
        )
    };
    if ret != 0 {
        vlog!(
            "kill_session_sysctl: sysctl data query failed (errno={})",
            io::Error::last_os_error()
        );
        return 0;
    }

    let actual_count = buf_len / kinfo_size;
    unsafe { buf.set_len(actual_count) };

    let mut killed = 0;
    for kp in &buf {
        let pid = kp.ki_pid;
        if pid <= 1 {
            continue;
        }
        vlog!("kill_session_sysctl: killing pid={pid}");
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        killed += 1;
    }
    killed
}

// ── PTY spawning ─────────────────────────────────────────────────────────────

struct PtySession {
    master_fd: RawFd,
    child_pid: libc::pid_t,
    buf: Arc<Mutex<Vec<u8>>>,
    reader_handle: Option<thread::JoinHandle<()>>,
    stop_pipe_w: RawFd,
    eof_sent: bool,
}

impl PtySession {
    fn spawn(argv: &[String], env_vars: &[(String, String)]) -> io::Result<Self> {
        Self::spawn_inner(argv, env_vars, false, None)
    }

    fn spawn_clean(
        argv: &[String],
        env_vars: &[(String, String)],
        workdir: &str,
    ) -> io::Result<Self> {
        Self::spawn_inner(argv, env_vars, true, Some(workdir))
    }

    fn spawn_inner(
        argv: &[String],
        env_vars: &[(String, String)],
        clear_env: bool,
        workdir: Option<&str>,
    ) -> io::Result<Self> {
        if argv.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty argv"));
        }

        unsafe {
            let mut master: CInt = -1;

            let mut termp: libc::termios = std::mem::zeroed();
            termp.c_iflag = (libc::ICRNL | libc::IXON) as libc::tcflag_t;
            termp.c_oflag = (libc::OPOST | libc::ONLCR) as libc::tcflag_t;
            termp.c_cflag = (libc::CS8 | libc::CREAD | libc::CLOCAL) as libc::tcflag_t;
            termp.c_lflag =
                (libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ICANON | libc::ISIG | libc::IEXTEN)
                    as libc::tcflag_t;
            termp.c_cc[libc::VINTR] = 0x03;
            termp.c_cc[libc::VQUIT] = 0x1c;
            termp.c_cc[libc::VERASE] = 0x7f;
            termp.c_cc[libc::VKILL] = 0x15;
            termp.c_cc[libc::VEOF] = 0x04;
            termp.c_cc[libc::VSUSP] = 0x1a;
            termp.c_cc[libc::VMIN] = 1;
            termp.c_cc[libc::VTIME] = 0;
            libc::cfsetispeed(&mut termp, libc::B38400);
            libc::cfsetospeed(&mut termp, libc::B38400);

            let mut winp: Winsize = std::mem::zeroed();
            winp.ws_row = 24;
            winp.ws_col = 80;

            let pid = forkpty(&mut master, std::ptr::null_mut(), &mut termp, &mut winp);

            if pid < 0 {
                return Err(io::Error::last_os_error());
            }

            if pid == 0 {
                let mut cmd = Command::new(&argv[0]);
                cmd.args(&argv[1..]);
                if clear_env {
                    cmd.env_clear();
                }
                for (k, v) in env_vars {
                    cmd.env(k, v);
                }
                if let Some(dir) = workdir {
                    cmd.current_dir(dir);
                }
                let err = cmd.exec();
                eprintln!("expect_pty: exec failed: {err}");
                std::process::exit(127);
            }

            // Parent — create a stop-pipe so we can interrupt the reader thread
            let mut stop_fds = [0 as CInt; 2];
            if libc::pipe(stop_fds.as_mut_ptr()) != 0 {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
                return Err(io::Error::last_os_error());
            }
            let stop_r = stop_fds[0];
            let stop_w = stop_fds[1];

            let buf = Arc::new(Mutex::new(Vec::new()));
            let buf_clone = Arc::clone(&buf);
            let reader_fd = master;

            let reader_handle = thread::spawn(move || {
                let mut tmp = [0u8; 4096];
                loop {
                    let mut fds = [
                        libc::pollfd {
                            fd: reader_fd,
                            events: libc::POLLIN,
                            revents: 0,
                        },
                        libc::pollfd {
                            fd: stop_r,
                            events: libc::POLLIN,
                            revents: 0,
                        },
                    ];
                    let ret = libc::poll(fds.as_mut_ptr(), 2, -1);
                    if ret < 0 {
                        let e = io::Error::last_os_error();
                        if e.raw_os_error() == Some(libc::EINTR) {
                            continue;
                        }
                        break;
                    }
                    if fds[1].revents != 0 {
                        break;
                    }
                    if fds[0].revents & libc::POLLNVAL != 0 {
                        break;
                    }
                    if fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                        let n = libc::read(reader_fd, tmp.as_mut_ptr() as *mut _, tmp.len());
                        if n <= 0 {
                            break;
                        }
                        let mut lock = buf_clone.lock().unwrap();
                        lock.extend_from_slice(&tmp[..n as usize]);
                    }
                }
                libc::close(stop_r);
            });

            vlog!("PtySession::spawn: child_pid={pid}, master_fd={master}, argv={:?}", argv);
            Ok(PtySession {
                master_fd: master,
                child_pid: pid,
                buf,
                reader_handle: Some(reader_handle),
                stop_pipe_w: stop_w,
                eof_sent: false,
            })
        }
    }

    fn write_bytes(&self, data: &[u8]) -> io::Result<()> {
        let n = unsafe { libc::write(self.master_fd, data.as_ptr() as *const _, data.len()) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn send_line(&self, text: &str) -> io::Result<()> {
        let mut data = text.as_bytes().to_vec();
        data.push(b'\n');
        self.write_bytes(&data)
    }

    fn send_eof(&mut self) -> io::Result<()> {
        if !self.eof_sent {
            self.eof_sent = true;
            // Send Ctrl-D (VEOF) character
            self.write_bytes(&[0x04])
        } else {
            Ok(())
        }
    }

    fn send_signal(&self, sig: CInt) -> io::Result<()> {
        let ret = unsafe { libc::killpg(self.child_pid, sig) };
        if ret < 0 {
            // Try kill() directly if killpg fails (child may not be a group leader)
            let ret2 = unsafe { libc::kill(self.child_pid, sig) };
            if ret2 < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Wait for a regex match in the output buffer.
    /// On match, consumes all output up to and including the match end.
    fn expect(
        &self,
        pattern: &[RegexNode],
        pattern_str: &str,
        timeout: Duration,
    ) -> Result<String, String> {
        let start = Instant::now();
        let mut last_verbose_log = Instant::now();
        loop {
            {
                let mut lock = self.buf.lock().unwrap();
                let haystack = String::from_utf8_lossy(&lock).to_string();
                if let Some((_match_start, match_end)) = regex_find(pattern, &haystack) {
                    let consumed = haystack[..match_end].to_string();
                    lock.drain(..match_end);
                    vlog!("expect: matched {:?} after {:.3}s", pattern_str, start.elapsed().as_secs_f64());
                    return Ok(consumed);
                }
                if verbose() && last_verbose_log.elapsed() >= Duration::from_millis(50) {
                    vlog!("expect: waiting for {:?} ({:.3}s elapsed, buf={} bytes): {:?}",
                        pattern_str, start.elapsed().as_secs_f64(), haystack.len(),
                        if haystack.len() > 200 { format!("{}...", &haystack[..200]) } else { haystack.clone() });
                    last_verbose_log = Instant::now();
                }
                if start.elapsed() >= timeout {
                    return Err(format!(
                        "expect: timed out after {:.1}s waiting for {:?}\nOutput so far ({} bytes):\n{}",
                        timeout.as_secs_f64(),
                        pattern_str,
                        haystack.len(),
                        haystack
                    ));
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Wait for the next complete line matching a regex pattern.
    /// Consumes non-matching lines while scanning forward.
    fn expect_line(
        &self,
        pattern: &[RegexNode],
        pattern_str: &str,
        timeout: Duration,
    ) -> Result<String, String> {
        let start = Instant::now();
        loop {
            {
                let mut lock = self.buf.lock().unwrap();
                let haystack = String::from_utf8_lossy(&lock).to_string();
                if let Some(nl_pos) = haystack.find('\n') {
                    let line = haystack[..nl_pos].trim_end_matches('\r');
                    if regex_find(pattern, line).is_some() {
                        let consumed_end = nl_pos + 1;
                        let consumed = haystack[..consumed_end].to_string();
                        lock.drain(..consumed_end);
                        return Ok(consumed);
                    }
                    lock.drain(..nl_pos + 1);
                    continue;
                }
                if start.elapsed() >= timeout {
                    return Err(format!(
                        "expect_line: timed out after {:.1}s waiting for line matching {:?}\nOutput so far ({} bytes):\n{}",
                        timeout.as_secs_f64(),
                        pattern_str,
                        haystack.len(),
                        haystack
                    ));
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_child(&mut self, expected_code: Option<i32>) -> Result<i32, String> {
        vlog!("wait_child: closing master_fd to signal EOF");
        if self.master_fd >= 0 {
            unsafe {
                libc::close(self.master_fd);
            }
            self.master_fd = -1;
        }

        vlog!("wait_child: waiting for child_pid={}", self.child_pid);
        let mut status: CInt = 0;
        unsafe {
            let ret = libc::waitpid(self.child_pid, &mut status, 0);
            if ret < 0 {
                return Err(format!("waitpid failed: {}", io::Error::last_os_error()));
            }
        }
        vlog!("wait_child: child {} exited, status=0x{:x}", self.child_pid, status);

        kill_session(self.child_pid);

        // Now stop the reader thread — all slave holders are dead so the
        // stop-pipe write wakes poll() and the thread exits cleanly.
        self.stop_reader();

        let code = if wifexited(status) {
            wexitstatus(status)
        } else {
            128 + (status & 0x7f)
        };

        if let Some(expected) = expected_code {
            if code != expected {
                return Err(format!("wait: expected exit code {expected}, got {code}"));
            }
        }
        Ok(code)
    }

    fn stop_reader(&mut self) {
        vlog!("stop_reader: closing stop_pipe_w={}", self.stop_pipe_w);
        if self.stop_pipe_w >= 0 {
            unsafe {
                libc::close(self.stop_pipe_w);
            }
            self.stop_pipe_w = -1;
        }
        if let Some(h) = self.reader_handle.take() {
            vlog!("stop_reader: joining reader thread...");
            let _ = h.join();
            vlog!("stop_reader: reader thread joined");
        }
    }

    fn cleanup(&mut self) {
        vlog!("cleanup: killing session for child_pid={}", self.child_pid);
        kill_session(self.child_pid);
        vlog!("cleanup: stopping reader thread");
        self.stop_reader();
        if self.master_fd >= 0 {
            vlog!("cleanup: closing master_fd={}", self.master_fd);
            unsafe {
                libc::close(self.master_fd);
            }
            self.master_fd = -1;
        }
        vlog!("cleanup: reaping child_pid={}", self.child_pid);
        unsafe {
            let mut status: CInt = 0;
            let ret = libc::waitpid(self.child_pid, &mut status, libc::WNOHANG);
            if ret == 0 {
                vlog!("cleanup: child {} still alive after kill_session, sending SIGKILL directly", self.child_pid);
                libc::kill(self.child_pid, libc::SIGKILL);
                libc::waitpid(self.child_pid, &mut status, 0);
            }
            vlog!("cleanup: child {} reaped, status=0x{:x}", self.child_pid, status);
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if self.master_fd >= 0 || self.stop_pipe_w >= 0 || self.reader_handle.is_some() {
            self.cleanup();
        }
    }
}

// ── Script parser and executor ───────────────────────────────────────────────

fn parse_signal(name: &str) -> Result<CInt, String> {
    match name {
        "SIGINT" | "INT" => Ok(libc::SIGINT),
        "SIGTSTP" | "TSTP" => Ok(libc::SIGTSTP),
        "SIGCONT" | "CONT" => Ok(libc::SIGCONT),
        "SIGTERM" | "TERM" => Ok(libc::SIGTERM),
        "SIGQUIT" | "QUIT" => Ok(libc::SIGQUIT),
        "SIGHUP" | "HUP" => Ok(libc::SIGHUP),
        "SIGUSR1" | "USR1" => Ok(libc::SIGUSR1),
        "SIGUSR2" | "USR2" => Ok(libc::SIGUSR2),
        "SIGSTOP" | "STOP" => Ok(libc::SIGSTOP),
        "SIGTTIN" | "TTIN" => Ok(libc::SIGTTIN),
        "SIGTTOU" | "TTOU" => Ok(libc::SIGTTOU),
        _ => Err(format!("unknown signal: {name}")),
    }
}

fn expand_env(s: &str) -> String {
    let mut result = s.to_string();
    for (key, value) in env::vars() {
        let var = format!("${key}");
        result = result.replace(&var, &value);
    }
    result
}

/// Extract a regex pattern from a quoted argument: `"pattern"` -> `pattern`
/// Backslash has NO special meaning — patterns pass through verbatim to the
/// regex engine.  To embed a literal double-quote, use `""` (doubled quote).
/// This means the regex you write between the quotes is exactly what you'd
/// pass to `rg`.
fn extract_pattern(arg: &str) -> Result<String, String> {
    if !arg.starts_with('"') || arg.len() < 2 {
        return Err(format!("expected quoted pattern, got: {arg}"));
    }
    let bytes = arg.as_bytes();
    let mut result = String::with_capacity(arg.len());
    let mut i = 1; // skip opening quote
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                result.push('"');
                i += 2;
                continue;
            }
            if i + 1 == bytes.len() {
                return Ok(result);
            }
            return Err(format!("unexpected content after closing quote: {arg}"));
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    Err(format!("unterminated quoted pattern: {arg}"))
}

/// Extract a quoted string argument: `"contents"` -> `contents`
/// Supports backslash escapes: `\"` -> `"`, `\\` -> `\`, `\n` -> newline, etc.
fn extract_quoted(arg: &str) -> Result<String, String> {
    if !arg.starts_with('"') || arg.len() < 2 {
        return Err(format!("expected quoted string, got: {arg}"));
    }
    let inner = &arg[1..];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.next().is_none() {
                return Ok(result);
            }
            return Err(format!("unexpected content after closing quote: {arg}"));
        }
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some(esc) => result.push(esc),
                None => return Err(format!("trailing backslash in quoted string: {arg}")),
            }
        } else {
            result.push(c);
        }
    }
    Err(format!("unterminated quoted string: {arg}"))
}

fn parse_timeout_value(val: &str) -> Result<Duration, String> {
    if let Some(ms_str) = val.strip_suffix("ms") {
        let ms: u64 = ms_str
            .parse()
            .map_err(|e| format!("bad timeout value: {e}"))?;
        Ok(Duration::from_millis(ms))
    } else if let Some(s_str) = val.strip_suffix("s") {
        let s: u64 = s_str
            .parse()
            .map_err(|e| format!("bad timeout value: {e}"))?;
        Ok(Duration::from_secs(s))
    } else {
        Err(format!(
            "timeout value must end with 'ms' or 's', got: {val}"
        ))
    }
}

// ── Cgroup v2 helpers ────────────────────────────────────────────────────────

fn discover_cgroup_base() -> Option<String> {
    let cgroup_info = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    for line in cgroup_info.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            let sysfs = format!("/sys/fs/cgroup{path}");
            let procs_path = format!("{sysfs}/cgroup.procs");
            if std::fs::metadata(&procs_path).is_ok() {
                return Some(sysfs);
            }
        }
    }
    None
}

fn create_test_cgroup(base: &str, id: &str) -> io::Result<String> {
    let path = format!("{base}/epty_{id}");
    std::fs::create_dir(&path)?;
    Ok(path)
}

fn move_to_cgroup(cg: &str, pid: u32) -> io::Result<()> {
    std::fs::write(format!("{cg}/cgroup.procs"), pid.to_string())
}

fn kill_cgroup(cg: &str) -> io::Result<()> {
    std::fs::write(format!("{cg}/cgroup.kill"), "1")
}

fn remove_cgroup(cg: &str) {
    for _ in 0..20 {
        if std::fs::remove_dir(cg).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = std::fs::remove_dir(cg);
}

/// Parse the arguments of an expect command, handling optional timeout=Nms/Ns prefix.
/// Returns (None, pattern_str) when no timeout is given — caller picks default.
fn parse_expect_args(rest: &str) -> Result<(Option<Duration>, &str), String> {
    let rest = rest.trim();
    if let Some(after) = rest.strip_prefix("timeout=") {
        let space = after.find(' ').ok_or_else(|| {
            "timeout= must be followed by a space and a quoted string".to_string()
        })?;
        let timeout = parse_timeout_value(&after[..space])?;
        let quoted = after[space..].trim();
        Ok((Some(timeout), quoted))
    } else {
        Ok((None, rest))
    }
}

// ── Exit code expression language ────────────────────────────────────────────
//
// Grammar:
//   expr     = or_expr
//   or_expr  = and_expr ("||" and_expr)*
//   and_expr = atom ("&&" atom)*
//   atom     = "(" expr ")" | comparison | literal
//   comparison = ("<=" | ">=" | "!=" | "==" | "<" | ">") INTEGER
//   literal  = INTEGER   (sugar for == INTEGER)

#[derive(Debug, Clone)]
enum ExitExpr {
    Literal(i32),
    Cmp(CmpOp, i32),
    And(Vec<ExitExpr>),
    Or(Vec<ExitExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl ExitExpr {
    fn eval(&self, code: i32) -> bool {
        match self {
            ExitExpr::Literal(n) => code == *n,
            ExitExpr::Cmp(op, n) => match op {
                CmpOp::Eq => code == *n,
                CmpOp::Ne => code != *n,
                CmpOp::Lt => code < *n,
                CmpOp::Le => code <= *n,
                CmpOp::Gt => code > *n,
                CmpOp::Ge => code >= *n,
            },
            ExitExpr::And(exprs) => exprs.iter().all(|e| e.eval(code)),
            ExitExpr::Or(exprs) => exprs.iter().any(|e| e.eval(code)),
        }
    }

    fn display(&self) -> String {
        match self {
            ExitExpr::Literal(n) => n.to_string(),
            ExitExpr::Cmp(op, n) => {
                let op_str = match op {
                    CmpOp::Eq => "==",
                    CmpOp::Ne => "!=",
                    CmpOp::Lt => "<",
                    CmpOp::Le => "<=",
                    CmpOp::Gt => ">",
                    CmpOp::Ge => ">=",
                };
                format!("{op_str}{n}")
            }
            ExitExpr::And(exprs) => exprs
                .iter()
                .map(|e| e.display())
                .collect::<Vec<_>>()
                .join(" && "),
            ExitExpr::Or(exprs) => exprs
                .iter()
                .map(|e| {
                    if matches!(e, ExitExpr::And(v) if v.len() > 1) {
                        format!("({})", e.display())
                    } else {
                        e.display()
                    }
                })
                .collect::<Vec<_>>()
                .join(" || "),
        }
    }
}

fn parse_exit_expr(input: &str) -> Result<ExitExpr, String> {
    let tokens = tokenize_exit_expr(input)?;
    let mut pos = 0;
    let expr = parse_or(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(format!(
            "unexpected token {:?} at position {pos} in exit expression: {input}",
            tokens[pos]
        ));
    }
    Ok(expr)
}

#[derive(Debug, Clone, PartialEq)]
enum ExitToken {
    Int(i32),
    Op(CmpOp),
    And,
    Or,
    LParen,
    RParen,
}

fn tokenize_exit_expr(input: &str) -> Result<Vec<ExitToken>, String> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' => {
                i += 1;
            }
            b'(' => {
                tokens.push(ExitToken::LParen);
                i += 1;
            }
            b')' => {
                tokens.push(ExitToken::RParen);
                i += 1;
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    tokens.push(ExitToken::And);
                    i += 2;
                } else {
                    return Err(format!("unexpected '&' in exit expression: {input}"));
                }
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(ExitToken::Or);
                    i += 2;
                } else {
                    return Err(format!("unexpected '|' in exit expression: {input}"));
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Ne));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                } else {
                    return Err(format!("unexpected '!' in exit expression: {input}"));
                }
            }
            b'<' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Le));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                } else {
                    i += 1;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Lt));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                }
            }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Ge));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                } else {
                    i += 1;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Gt));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                }
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    i += 2;
                    let (n, adv) = parse_int_at(bytes, i, input)?;
                    tokens.push(ExitToken::Op(CmpOp::Eq));
                    tokens.push(ExitToken::Int(n));
                    i += adv;
                } else {
                    return Err(format!("unexpected '=' in exit expression: {input}"));
                }
            }
            b'0'..=b'9' => {
                let (n, adv) = parse_int_at(bytes, i, input)?;
                tokens.push(ExitToken::Int(n));
                i += adv;
            }
            _ => {
                return Err(format!(
                    "unexpected character '{}' in exit expression: {input}",
                    bytes[i] as char
                ));
            }
        }
    }
    Ok(tokens)
}

fn parse_int_at(bytes: &[u8], start: usize, input: &str) -> Result<(i32, usize), String> {
    let mut end = start;
    if end < bytes.len() && bytes[end] == b'-' {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start || (end == start + 1 && bytes[start] == b'-') {
        return Err(format!("expected integer in exit expression: {input}"));
    }
    let s = &input[start..end];
    let n: i32 = s
        .parse()
        .map_err(|e| format!("bad integer in exit expression: {e}"))?;
    Ok((n, end - start))
}

fn parse_or(tokens: &[ExitToken], pos: &mut usize) -> Result<ExitExpr, String> {
    let mut exprs = vec![parse_and(tokens, pos)?];
    while *pos < tokens.len() && tokens[*pos] == ExitToken::Or {
        *pos += 1;
        exprs.push(parse_and(tokens, pos)?);
    }
    if exprs.len() == 1 {
        Ok(exprs.pop().unwrap())
    } else {
        Ok(ExitExpr::Or(exprs))
    }
}

fn parse_and(tokens: &[ExitToken], pos: &mut usize) -> Result<ExitExpr, String> {
    let mut exprs = vec![parse_atom(tokens, pos)?];
    while *pos < tokens.len() && tokens[*pos] == ExitToken::And {
        *pos += 1;
        exprs.push(parse_atom(tokens, pos)?);
    }
    if exprs.len() == 1 {
        Ok(exprs.pop().unwrap())
    } else {
        Ok(ExitExpr::And(exprs))
    }
}

fn parse_atom(tokens: &[ExitToken], pos: &mut usize) -> Result<ExitExpr, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of exit expression".to_string());
    }
    match &tokens[*pos] {
        ExitToken::LParen => {
            *pos += 1;
            let expr = parse_or(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != ExitToken::RParen {
                return Err("missing ')' in exit expression".to_string());
            }
            *pos += 1;
            Ok(expr)
        }
        ExitToken::Op(op) => {
            let op = *op;
            *pos += 1;
            if *pos >= tokens.len() {
                return Err("expected integer after comparison operator".to_string());
            }
            if let ExitToken::Int(n) = tokens[*pos] {
                *pos += 1;
                Ok(ExitExpr::Cmp(op, n))
            } else {
                Err("expected integer after comparison operator".to_string())
            }
        }
        ExitToken::Int(n) => {
            let n = *n;
            *pos += 1;
            Ok(ExitExpr::Literal(n))
        }
        other => Err(format!("unexpected token {other:?} in exit expression")),
    }
}

// ── POSIX bracket expressions ────────────────────────────────────────────────

/// Test whether character `c` belongs to a POSIX named character class.
fn is_char_class(name: &str, c: char) -> bool {
    match name {
        "alnum" => c.is_alphanumeric(),
        "alpha" => c.is_alphabetic(),
        "blank" => c == ' ' || c == '\t',
        "cntrl" => c.is_control(),
        "digit" => c.is_ascii_digit(),
        "graph" => !c.is_control() && c != ' ',
        "lower" => c.is_lowercase(),
        "print" => !c.is_control(),
        "punct" => c.is_ascii_punctuation(),
        "space" => c.is_whitespace(),
        "upper" => c.is_uppercase(),
        "xdigit" => c.is_ascii_hexdigit(),
        _ => false,
    }
}

/// Try to parse a POSIX bracket expression starting at `pat[start]` where
/// `pat[start] == '['`. Returns `Some((matched, end))` where `matched` is
/// true if `ch` is in the set, and `end` is the index past the closing `]`.
/// Returns `None` if the brackets don't form a valid bracket expression
/// (unmatched `[`), in which case `[` should be treated as literal.
fn match_bracket_expr(pat: &[char], start: usize, ch: char) -> Option<(bool, usize)> {
    let len = pat.len();
    let mut i = start + 1; // skip opening '['
    if i >= len {
        return None;
    }

    // Check for negation: [!...] or [^...]
    let negate = if pat[i] == '!' || pat[i] == '^' {
        i += 1;
        true
    } else {
        false
    };

    let mut matched = false;
    let mut prev_char: Option<char> = None;

    // ']' immediately after '[' (or '[!' / '[^') is literal
    if i < len && pat[i] == ']' {
        if ch == ']' {
            matched = true;
        }
        prev_char = Some(']');
        i += 1;
    }

    while i < len && pat[i] != ']' {
        // Named character class: [[:name:]]
        if i + 1 < len && pat[i] == '[' && pat[i + 1] == ':' {
            if let Some(end_colon) = find_seq(pat, i + 2, ':') {
                if end_colon + 1 < len && pat[end_colon + 1] == ']' {
                    let name: String = pat[i + 2..end_colon].iter().collect();
                    if is_char_class(&name, ch) {
                        matched = true;
                    }
                    i = end_colon + 2;
                    prev_char = None;
                    continue;
                }
            }
        }

        // Collating symbol: [.x.] — treat as the character x
        if i + 1 < len && pat[i] == '[' && pat[i + 1] == '.' {
            if let Some(end_dot) = find_seq(pat, i + 2, '.') {
                if end_dot + 1 < len && pat[end_dot + 1] == ']' {
                    let sym: String = pat[i + 2..end_dot].iter().collect();
                    if sym.len() == 1 {
                        let sc = sym.chars().next().unwrap();
                        if ch == sc {
                            matched = true;
                        }
                        prev_char = Some(sc);
                    }
                    i = end_dot + 2;
                    continue;
                }
            }
        }

        // Equivalence class: [=x=] — treat as matching character x
        if i + 1 < len && pat[i] == '[' && pat[i + 1] == '=' {
            if let Some(end_eq) = find_seq(pat, i + 2, '=') {
                if end_eq + 1 < len && pat[end_eq + 1] == ']' {
                    let sym: String = pat[i + 2..end_eq].iter().collect();
                    if sym.len() == 1 {
                        let sc = sym.chars().next().unwrap();
                        if ch == sc {
                            matched = true;
                        }
                        prev_char = Some(sc);
                    }
                    i = end_eq + 2;
                    continue;
                }
            }
        }

        let c = pat[i];

        // Range expression: prev-c
        if c == '-' && prev_char.is_some() && i + 1 < len && pat[i + 1] != ']' {
            let range_start = prev_char.unwrap();
            let range_end = pat[i + 1];
            if ch >= range_start && ch <= range_end {
                matched = true;
            }
            i += 2;
            prev_char = Some(range_end);
            continue;
        }

        // Ordinary character in the list
        if ch == c {
            matched = true;
        }
        prev_char = Some(c);
        i += 1;
    }

    if i >= len {
        return None; // no closing ']' found — not a valid bracket expression
    }

    // i is at the closing ']'
    let result = if negate { !matched } else { matched };
    Some((result, i + 1))
}

/// Find the position of `target` char in `pat` starting from `from`.
fn find_seq(pat: &[char], from: usize, target: char) -> Option<usize> {
    for j in from..pat.len() {
        if pat[j] == target {
            return Some(j);
        }
    }
    None
}

// ── Regex engine (no external dependencies) ──────────────────────────────────

#[derive(Debug, Clone)]
enum RepeatKind {
    Star,
    Plus,
    Question,
}

#[derive(Debug, Clone)]
enum RegexNode {
    Literal(char),
    AnyChar,
    Class(Vec<char>),
    Repeat(Box<RegexNode>, RepeatKind),
    Group(Vec<Vec<RegexNode>>),
}

fn parse_regex(pattern: &str) -> Result<Vec<RegexNode>, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let (nodes, pos) = parse_alternation(&chars, 0)?;
    if pos != chars.len() {
        return Err(format!("unexpected '{}' at position {pos}", chars[pos]));
    }
    Ok(nodes)
}

fn parse_alternation(chars: &[char], start: usize) -> Result<(Vec<RegexNode>, usize), String> {
    let (first_seq, mut pos) = parse_re_sequence(chars, start)?;
    if pos < chars.len() && chars[pos] == '|' {
        let mut alternatives = vec![first_seq];
        while pos < chars.len() && chars[pos] == '|' {
            let (next_seq, new_pos) = parse_re_sequence(chars, pos + 1)?;
            alternatives.push(next_seq);
            pos = new_pos;
        }
        Ok((vec![RegexNode::Group(alternatives)], pos))
    } else {
        Ok((first_seq, pos))
    }
}

fn parse_re_sequence(chars: &[char], start: usize) -> Result<(Vec<RegexNode>, usize), String> {
    let mut nodes = Vec::new();
    let mut pos = start;
    while pos < chars.len() && chars[pos] != '|' && chars[pos] != ')' {
        let (atom, new_pos) = parse_re_atom(chars, pos)?;
        pos = new_pos;
        if pos < chars.len() && matches!(chars[pos], '*' | '+' | '?') {
            let kind = match chars[pos] {
                '*' => RepeatKind::Star,
                '+' => RepeatKind::Plus,
                '?' => RepeatKind::Question,
                _ => unreachable!(),
            };
            nodes.push(RegexNode::Repeat(Box::new(atom), kind));
            pos += 1;
        } else {
            nodes.push(atom);
        }
    }
    Ok((nodes, pos))
}

fn parse_re_atom(chars: &[char], pos: usize) -> Result<(RegexNode, usize), String> {
    if pos >= chars.len() {
        return Err("unexpected end of pattern".to_string());
    }
    match chars[pos] {
        '.' => Ok((RegexNode::AnyChar, pos + 1)),
        '\\' => {
            if pos + 1 >= chars.len() {
                return Err("trailing backslash in pattern".to_string());
            }
            let escaped = match chars[pos + 1] {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c => c,
            };
            Ok((RegexNode::Literal(escaped), pos + 2))
        }
        '[' => {
            let mut bracket = vec!['['];
            let mut i = pos + 1;
            if i < chars.len() && (chars[i] == '!' || chars[i] == '^') {
                bracket.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == ']' {
                bracket.push(']');
                i += 1;
            }
            while i < chars.len() && chars[i] != ']' {
                if i + 1 < chars.len() && chars[i] == '[' && matches!(chars[i + 1], ':' | '.' | '=')
                {
                    let delim = chars[i + 1];
                    bracket.push(chars[i]);
                    bracket.push(chars[i + 1]);
                    i += 2;
                    while i < chars.len() {
                        if chars[i] == delim && i + 1 < chars.len() && chars[i + 1] == ']' {
                            bracket.push(chars[i]);
                            bracket.push(chars[i + 1]);
                            i += 2;
                            break;
                        }
                        bracket.push(chars[i]);
                        i += 1;
                    }
                } else {
                    bracket.push(chars[i]);
                    i += 1;
                }
            }
            if i >= chars.len() {
                return Err(format!("unclosed bracket expression at position {pos}"));
            }
            bracket.push(']');
            Ok((RegexNode::Class(bracket), i + 1))
        }
        '(' => {
            let (inner_nodes, new_pos) = parse_alternation(chars, pos + 1)?;
            if new_pos >= chars.len() || chars[new_pos] != ')' {
                return Err(format!("unclosed parenthesis at position {pos}"));
            }
            if inner_nodes.len() == 1 {
                if let RegexNode::Group(_) = &inner_nodes[0] {
                    return Ok((inner_nodes.into_iter().next().unwrap(), new_pos + 1));
                }
            }
            Ok((RegexNode::Group(vec![inner_nodes]), new_pos + 1))
        }
        '*' | '+' | '?' => Err(format!(
            "quantifier '{}' without preceding element at position {pos}",
            chars[pos]
        )),
        ')' => Err(format!("unexpected ')' at position {pos}")),
        c => Ok((RegexNode::Literal(c), pos + 1)),
    }
}

/// Try to match a sequence of regex nodes starting at `pos` in `text`.
/// Returns `Some(end_pos)` on success. Handles backtracking for repeats.
fn match_re_seq(nodes: &[RegexNode], text: &[char], pos: usize) -> Option<usize> {
    if nodes.is_empty() {
        return Some(pos);
    }
    match &nodes[0] {
        RegexNode::Repeat(inner, kind) => {
            let rest = &nodes[1..];
            match kind {
                RepeatKind::Star => {
                    let positions = collect_repeat_positions(inner, text, pos);
                    for &p in positions.iter().rev() {
                        if let Some(end) = match_re_seq(rest, text, p) {
                            return Some(end);
                        }
                    }
                    None
                }
                RepeatKind::Plus => {
                    let mut matches = Vec::new();
                    let mut current = pos;
                    loop {
                        match match_re_one(inner, text, current) {
                            Some(next) if next > current => {
                                matches.push(next);
                                current = next;
                            }
                            _ => break,
                        }
                    }
                    for &p in matches.iter().rev() {
                        if let Some(end) = match_re_seq(rest, text, p) {
                            return Some(end);
                        }
                    }
                    None
                }
                RepeatKind::Question => {
                    if let Some(next) = match_re_one(inner, text, pos) {
                        if let Some(end) = match_re_seq(rest, text, next) {
                            return Some(end);
                        }
                    }
                    match_re_seq(rest, text, pos)
                }
            }
        }
        node => {
            if let Some(next) = match_re_one(node, text, pos) {
                match_re_seq(&nodes[1..], text, next)
            } else {
                None
            }
        }
    }
}

/// Match a single (non-Repeat) regex node at position `pos`.
fn match_re_one(node: &RegexNode, text: &[char], pos: usize) -> Option<usize> {
    match node {
        RegexNode::Literal(c) => {
            if pos < text.len() && text[pos] == *c {
                Some(pos + 1)
            } else {
                None
            }
        }
        RegexNode::AnyChar => {
            if pos < text.len() && text[pos] != '\n' {
                Some(pos + 1)
            } else {
                None
            }
        }
        RegexNode::Class(bracket) => {
            if pos < text.len() {
                match match_bracket_expr(bracket, 0, text[pos]) {
                    Some((true, _)) => Some(pos + 1),
                    _ => None,
                }
            } else {
                None
            }
        }
        RegexNode::Group(alternatives) => {
            for alt in alternatives {
                if let Some(end) = match_re_seq(alt, text, pos) {
                    return Some(end);
                }
            }
            None
        }
        RegexNode::Repeat(..) => match_re_seq(std::slice::from_ref(node), text, pos),
    }
}

fn collect_repeat_positions(inner: &RegexNode, text: &[char], pos: usize) -> Vec<usize> {
    let mut positions = vec![pos];
    let mut current = pos;
    loop {
        match match_re_one(inner, text, current) {
            Some(next) if next > current => {
                positions.push(next);
                current = next;
            }
            _ => break,
        }
    }
    positions
}

/// Check if the pattern matches the entire `text` (anchored at both ends).
fn regex_full_match(pattern: &[RegexNode], text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    match match_re_seq(pattern, &chars, 0) {
        Some(end) => end == chars.len(),
        None => false,
    }
}

/// Find the leftmost regex match in `text`. Returns byte offsets `(start, end)`.
fn regex_find(pattern: &[RegexNode], text: &str) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let byte_offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();

    for start_idx in 0..=chars.len() {
        if let Some(end_idx) = match_re_seq(pattern, &chars, start_idx) {
            let start_byte = if start_idx < byte_offsets.len() {
                byte_offsets[start_idx]
            } else {
                text.len()
            };
            let end_byte = if end_idx < byte_offsets.len() {
                byte_offsets[end_idx]
            } else {
                text.len()
            };
            return Some((start_byte, end_byte));
        }
    }
    None
}

fn run_script(script_lines: &[String]) -> Result<(), String> {
    let mut session: Option<PtySession> = None;
    let mut log = Vec::<String>::new();

    for (lineno, raw_line) in script_lines.iter().enumerate() {
        let line_num = lineno + 1;
        let line = raw_line.trim();

        // Skip comments and blanks
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let effective = strip_inline_comment(line);
        if effective.is_empty() {
            continue;
        }

        let (cmd, rest) = match effective.find(' ') {
            Some(pos) => (effective[..pos].trim(), effective[pos + 1..].trim()),
            None => (effective, ""),
        };

        match cmd {
            "spawn" => {
                if session.is_some() {
                    return Err(format!("line {line_num}: spawn called twice"));
                }
                let expanded = expand_env(rest);
                let words: Vec<String> = expanded.split_whitespace().map(String::from).collect();
                let mut env_vars: Vec<(String, String)> = Vec::new();
                let mut cmd_start = 0;
                for (i, w) in words.iter().enumerate() {
                    if let Some(eq) = w.find('=') {
                        let key = w[..eq].to_string();
                        let raw_val = &w[eq + 1..];
                        let val = raw_val.replace("\\s", " ").replace("\\\\", "\\");
                        env_vars.push((key, val));
                        cmd_start = i + 1;
                    } else {
                        break;
                    }
                }
                let argv = &words[cmd_start..];
                log.push(format!(">>> spawn {}", expanded));
                session = Some(
                    PtySession::spawn(argv, &env_vars)
                        .map_err(|e| format!("line {line_num}: spawn failed: {e}"))?,
                );
            }

            "expect" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str =
                    extract_pattern(quoted_part).map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(
                    ">>> expect {:?} (timeout={:.1}s)",
                    pattern_str,
                    timeout.as_secs_f64()
                ));
                match sess.expect(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!("<<< matched (consumed {} bytes)", consumed.len()));
                    }
                    Err(e) => {
                        eprintln!("--- expect_pty conversation log ---");
                        for entry in &log {
                            eprintln!("{entry}");
                        }
                        eprintln!("--- end log ---");
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
            }

            "expect_line" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect_line before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str =
                    extract_pattern(quoted_part).map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(
                    ">>> expect_line {:?} (timeout={:.1}s)",
                    pattern_str,
                    timeout.as_secs_f64()
                ));
                match sess.expect_line(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!(
                            "<<< line matched (consumed {} bytes)",
                            consumed.len()
                        ));
                    }
                    Err(e) => {
                        eprintln!("--- expect_pty conversation log ---");
                        for entry in &log {
                            eprintln!("{entry}");
                        }
                        eprintln!("--- end log ---");
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
            }

            "send" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: send before spawn"))?;
                let text = extract_quoted(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                let expanded = expand_env(&text);
                log.push(format!(">>> send {:?}", expanded));
                sess.send_line(&expanded)
                    .map_err(|e| format!("line {line_num}: send failed: {e}"))?;
            }

            "sendraw" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: sendraw before spawn"))?;
                let hex_parts: Vec<&str> = rest.split_whitespace().collect();
                let mut bytes = Vec::new();
                for hex in &hex_parts {
                    let b = u8::from_str_radix(hex, 16)
                        .map_err(|e| format!("line {line_num}: bad hex byte '{hex}': {e}"))?;
                    bytes.push(b);
                }
                log.push(format!(">>> sendraw [{}]", hex_parts.join(" ")));
                sess.write_bytes(&bytes)
                    .map_err(|e| format!("line {line_num}: sendraw failed: {e}"))?;
            }

            "signal" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: signal before spawn"))?;
                let sig = parse_signal(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> signal {rest}"));
                sess.send_signal(sig)
                    .map_err(|e| format!("line {line_num}: signal failed: {e}"))?;
            }

            "sendeof" => {
                let sess = session
                    .as_mut()
                    .ok_or_else(|| format!("line {line_num}: sendeof before spawn"))?;
                log.push(">>> sendeof".to_string());
                sess.send_eof()
                    .map_err(|e| format!("line {line_num}: sendeof failed: {e}"))?;
            }

            "wait" => {
                let sess = session
                    .as_mut()
                    .ok_or_else(|| format!("line {line_num}: wait before spawn"))?;
                let expected_code = if let Some(val) = rest.strip_prefix("exitcode=") {
                    Some(
                        val.parse::<i32>()
                            .map_err(|e| format!("line {line_num}: bad exitcode: {e}"))?,
                    )
                } else if rest.is_empty() {
                    None
                } else {
                    return Err(format!("line {line_num}: unknown wait argument: {rest}"));
                };
                log.push(format!(">>> wait exitcode={:?}", expected_code));
                match sess.wait_child(expected_code) {
                    Ok(code) => {
                        log.push(format!("<<< child exited with code {code}"));
                    }
                    Err(e) => {
                        eprintln!("--- expect_pty conversation log ---");
                        for entry in &log {
                            eprintln!("{entry}");
                        }
                        eprintln!("--- end log ---");
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
                session = None; // consumed
            }

            "sleep" => {
                let dur = parse_timeout_value(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> sleep {}ms", dur.as_millis()));
                thread::sleep(dur);
            }

            _ => {
                return Err(format!("line {line_num}: unknown command: {cmd}"));
            }
        }
    }

    // If session is still alive (no explicit wait), clean up
    if let Some(mut sess) = session {
        let _ = sess.send_eof();
        let _ = sess.wait_child(None);
    }

    Ok(())
}

// ── Suite data structures ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScriptMode {
    DashC,
    Tempfile,
    Stdin,
}

fn parse_script_modes(s: &str) -> Result<Vec<ScriptMode>, String> {
    let mut modes = Vec::new();
    for part in s.split(',') {
        let mode = match part.trim() {
            "dash-c" => ScriptMode::DashC,
            "tempfile" => ScriptMode::Tempfile,
            "stdin" => ScriptMode::Stdin,
            other => return Err(format!("unknown script mode: {other:?}")),
        };
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    if modes.is_empty() {
        return Err("--script-modes requires at least one mode".to_string());
    }
    Ok(modes)
}

type TestCase = epty_parser::TestCase;
type TestSuite = epty_parser::TestSuite;

struct RunResult {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Clone, Copy)]
enum TestOutcome {
    Pass,
    Fail,
}

struct TestReport {
    name: String,
    outcome: TestOutcome,
    error: Option<String>,
}

// ── Suite parser ─────────────────────────────────────────────────────────────

fn parse_suite(text: &str, filename: &str) -> Result<TestSuite, String> {
    if filename.ends_with(".md") {
        md_parser::parse_md_suite(text, filename)
    } else {
        epty_parser::parse_suite(text, filename)
    }
}

// Matrix integrity checks moved to tests/check_matrix_integrity.rs.

/// Strip inline comments: everything after an unquoted `#` is removed.
/// Respects `\"` escapes inside quoted strings.
fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else if bytes[i] == b'#' {
            return line[..i].trim_end();
        } else {
            i += 1;
        }
    }
    line
}

// ── Test isolation ───────────────────────────────────────────────────────────

fn baseline_env(
    tmpdir: &str,
    shell_str: &str,
    locale_dir: Option<&str>,
) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("PATH".into(), "/usr/bin:/bin".into());
    env.insert("HOME".into(), tmpdir.into());
    env.insert("TMPDIR".into(), tmpdir.into());
    env.insert("TERM".into(), "xterm".into());
    env.insert("LANG".into(), "C".into());
    env.insert("LC_ALL".into(), "C".into());
    if let Some(dir) = locale_dir {
        env.insert("LOCPATH".into(), dir.into());
        env.insert("PATH_LOCALE".into(), dir.into());
    }
    env.insert("PS1".into(), "$ ".into());
    env.insert("PS2".into(), "> ".into());
    env.insert("ENV".into(), String::new());
    env.insert("HISTFILE".into(), "/dev/null".into());
    env.insert("SHELL".into(), shell_str.into());
    env.insert(
        "LLVM_PROFILE_FILE".into(),
        format!("{tmpdir}/default_%p_%m.profraw"),
    );
    env
}

fn compile_locale_variant(
    locale_dir: &str,
    def_path: &str,
    charmap: &str,
    locale_name: &str,
) -> bool {
    let out_path = format!("{locale_dir}/{locale_name}");
    if std::path::Path::new(&out_path).is_dir() {
        return true;
    }

    let status = Command::new("localedef")
        .args(["-f", charmap, "-i", def_path, &out_path])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            eprintln!("expect_pty: compiled test locale to {out_path}");
            true
        }
        _ => false,
    }
}

fn compile_test_locale() -> Option<String> {
    let locale_dir = "/tmp/epty_locale";

    let _ = fs::create_dir_all(locale_dir);

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let def_path = format!("{manifest_dir}/tests/matrix/locale/test_EPTY.def");

    if !std::path::Path::new(&def_path).exists() {
        eprintln!("expect_pty: locale definition not found at {def_path}, skipping locale setup");
        return None;
    }

    let utf8_ok = compile_locale_variant(locale_dir, &def_path, "UTF-8", "test_EPTY.UTF-8");
    let latin1_ok =
        compile_locale_variant(locale_dir, &def_path, "ISO-8859-1", "test_EPTY.ISO-8859-1");

    if utf8_ok || latin1_ok {
        Some(locale_dir.to_string())
    } else {
        eprintln!("expect_pty: localedef not available or failed, skipping locale tests");
        None
    }
}

fn make_test_tmpdir() -> io::Result<String> {
    let template = "/tmp/epty_test_XXXXXX";
    let mut buf = template.as_bytes().to_vec();
    buf.push(0);
    let ptr = unsafe { libc::mkdtemp(buf.as_mut_ptr() as *mut libc::c_char) };
    if ptr.is_null() {
        return Err(io::Error::last_os_error());
    }
    buf.pop(); // remove null
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn remove_dir_all(path: &str) {
    let _ = fs::remove_dir_all(path);
}

// ── Non-interactive run command ──────────────────────────────────────────────

fn run_command(
    script: &str,
    shell_argv: &[String],
    test_env: &HashMap<String, String>,
    workdir: &str,
    tmpdir: &str,
    mode: ScriptMode,
) -> Result<RunResult, String> {
    let output = match mode {
        ScriptMode::DashC => {
            let mut cmd = Command::new(&shell_argv[0]);
            for arg in &shell_argv[1..] {
                cmd.arg(arg);
            }
            cmd.arg("-c").arg(script);
            cmd.env_clear();
            for (k, v) in test_env {
                cmd.env(k, v);
            }
            cmd.current_dir(workdir);
            cmd.output()
                .map_err(|e| format!("failed to execute shell (dash-c): {e}"))?
        }
        ScriptMode::Tempfile => {
            let script_path = format!("{tmpdir}/_test.sh");
            fs::write(&script_path, script)
                .map_err(|e| format!("failed to write script file: {e}"))?;
            let mut cmd = Command::new(&shell_argv[0]);
            for arg in &shell_argv[1..] {
                cmd.arg(arg);
            }
            cmd.arg(&script_path);
            cmd.env_clear();
            for (k, v) in test_env {
                cmd.env(k, v);
            }
            cmd.current_dir(workdir);
            let result = cmd
                .output()
                .map_err(|e| format!("failed to execute shell (tempfile): {e}"))?;
            let _ = fs::remove_file(&script_path);
            result
        }
        ScriptMode::Stdin => {
            use std::io::Write;
            use std::process::Stdio;
            let mut cmd = Command::new(&shell_argv[0]);
            for arg in &shell_argv[1..] {
                cmd.arg(arg);
            }
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            cmd.env_clear();
            for (k, v) in test_env {
                cmd.env(k, v);
            }
            cmd.current_dir(workdir);
            let mut child = cmd
                .spawn()
                .map_err(|e| format!("failed to execute shell (stdin): {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(script.as_bytes());
            }
            child
                .wait_with_output()
                .map_err(|e| format!("failed to wait for shell (stdin): {e}"))?
        }
    };
    Ok(RunResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    })
}

// ── Suite test executor ──────────────────────────────────────────────────────

fn run_suite_test(
    test: &TestCase,
    shell_argv: &[String],
    shell_str: &str,
    script_modes: &[ScriptMode],
    locale_dir: Option<&str>,
) -> Result<(), String> {
    let tmpdir = make_test_tmpdir().map_err(|e| format!("failed to create tmpdir: {e}"))?;
    let workdir = format!("{tmpdir}/sandbox");
    fs::create_dir(&workdir).map_err(|e| format!("failed to create sandbox dir: {e}"))?;

    let result = run_suite_test_inner(
        test,
        shell_argv,
        shell_str,
        &tmpdir,
        &workdir,
        script_modes,
        locale_dir,
    );
    remove_dir_all(&tmpdir);
    result
}

fn run_suite_test_inner(
    test: &TestCase,
    shell_argv: &[String],
    shell_str: &str,
    tmpdir: &str,
    workdir: &str,
    script_modes: &[ScriptMode],
    locale_dir: Option<&str>,
) -> Result<(), String> {
    let mut test_env = baseline_env(tmpdir, shell_str, locale_dir);
    for (k, v) in &test.env_overrides {
        test_env.insert(k.clone(), v.clone());
    }

    let mut session: Option<PtySession> = None;
    let mut last_run: Option<RunResult> = None;
    let mut log = Vec::<String>::new();

    if let Some(ref script) = test.script {
        log.push(format!(
            ">>> script ({} modes) {:?}",
            script_modes.len(),
            if script.len() > 80 {
                format!("{}...", &script[..80])
            } else {
                script.clone()
            }
        ));
        let mut reference: Option<RunResult> = None;
        for &mode in script_modes {
            let rr = run_command(script, shell_argv, &test_env, workdir, tmpdir, mode)?;
            if let Some(ref prev) = reference {
                if rr.stdout != prev.stdout
                    || rr.stderr != prev.stderr
                    || rr.exit_code != prev.exit_code
                {
                    return Err(format!(
                        "script mode divergence: {:?} vs {:?}\n\
                         --- {:?} ---\nstdout: {:?}\nstderr: {:?}\nexit: {}\n\
                         --- {:?} ---\nstdout: {:?}\nstderr: {:?}\nexit: {}",
                        script_modes[0],
                        mode,
                        script_modes[0],
                        prev.stdout,
                        prev.stderr,
                        prev.exit_code,
                        mode,
                        rr.stdout,
                        rr.stderr,
                        rr.exit_code,
                    ));
                }
            } else {
                reference = Some(rr);
            }
        }
        last_run = reference;
    }

    if let Some(ref rr) = last_run {
        if let Some((line_num, ref raw_pattern)) = test.expect_stdout {
            let pattern_str =
                extract_pattern(raw_pattern).map_err(|e| format!("line {line_num}: {e}"))?;
            let pattern = parse_regex(&pattern_str)
                .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
            log.push(format!(">>> stdout {:?}", pattern_str));
            let trimmed = rr.stdout.trim_end();
            if !regex_full_match(&pattern, trimmed) {
                return Err(format!(
                    "line {line_num}: stdout did not match {:?}\nstdout:\n{}",
                    pattern_str, rr.stdout
                ));
            }
        }
        if let Some((line_num, ref raw_pattern)) = test.expect_stderr {
            let pattern_str =
                extract_pattern(raw_pattern).map_err(|e| format!("line {line_num}: {e}"))?;
            let pattern = parse_regex(&pattern_str)
                .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
            log.push(format!(">>> stderr {:?}", pattern_str));
            let trimmed = rr.stderr.trim_end();
            if !regex_full_match(&pattern, trimmed) {
                return Err(format!(
                    "line {line_num}: stderr did not match {:?}\nstderr:\n{}",
                    pattern_str, rr.stderr
                ));
            }
        }
        if let Some((line_num, ref raw_expr)) = test.expect_exit_code {
            let expr =
                parse_exit_expr(raw_expr.trim()).map_err(|e| format!("line {line_num}: {e}"))?;
            log.push(format!(">>> exit_code {}", expr.display()));
            if !expr.eval(rr.exit_code) {
                return Err(format!(
                    "line {line_num}: exit_code: expression `{}` not satisfied by exit code {}",
                    expr.display(),
                    rr.exit_code
                ));
            }
        }
    }

    let lines: Vec<(usize, &str)> = test
        .script_lines
        .iter()
        .map(|(ln, s)| (*ln, s.as_str()))
        .collect();

    for &(line_num, raw_line) in &lines {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let effective = strip_inline_comment(line);
        if effective.is_empty() {
            continue;
        }

        let (cmd, rest) = match effective.find(' ') {
            Some(pos) => (effective[..pos].trim(), effective[pos + 1..].trim()),
            None => (effective, ""),
        };

        match cmd {
            "spawn" => {
                if !test.interactive {
                    return Err(format!(
                        "line {line_num}: spawn is only allowed in interactive tests (use begin interactive test)"
                    ));
                }
                if rest.contains("{{SHELL}}") {
                    return Err(format!(
                        "line {line_num}: spawn does not accept {{{{SHELL}}}} \u{2014} write `spawn -i` instead (shell is prepended automatically from --shell)"
                    ));
                }
                if session.is_some() {
                    return Err(format!("line {line_num}: spawn called twice"));
                }
                let mut words: Vec<String> = shell_argv.iter().map(|s| s.to_string()).collect();
                if !rest.is_empty() {
                    words.extend(rest.split_whitespace().map(String::from));
                }
                let env_pairs: Vec<(String, String)> = test_env
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                log.push(format!(">>> spawn {}", words.join(" ")));
                vlog!("suite: spawn {}", words.join(" "));
                session = Some(
                    PtySession::spawn_clean(&words, &env_pairs, workdir)
                        .map_err(|e| format!("line {line_num}: spawn failed: {e}"))?,
                );
            }

            // Interactive PTY commands
            "expect" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str =
                    extract_pattern(quoted_part).map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(
                    ">>> expect {:?} (timeout={:.1}s)",
                    pattern_str,
                    timeout.as_secs_f64()
                ));
                vlog!("suite: expect {:?} (timeout={:.1}s)", pattern_str, timeout.as_secs_f64());
                match sess.expect(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!("<<< matched (consumed {} bytes)", consumed.len()));
                    }
                    Err(e) => {
                        vlog!("suite: expect FAILED: {e}");
                        dump_log(&log);
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
            }

            "expect_line" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect_line before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str =
                    extract_pattern(quoted_part).map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(
                    ">>> expect_line {:?} (timeout={:.1}s)",
                    pattern_str,
                    timeout.as_secs_f64()
                ));
                match sess.expect_line(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!(
                            "<<< line matched (consumed {} bytes)",
                            consumed.len()
                        ));
                    }
                    Err(e) => {
                        dump_log(&log);
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
            }

            "send" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: send before spawn"))?;
                let text = extract_quoted(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> send {:?}", text));
                vlog!("suite: send {:?}", text);
                sess.send_line(&text)
                    .map_err(|e| format!("line {line_num}: send failed: {e}"))?;
            }

            "sendraw" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: sendraw before spawn"))?;
                let hex_parts: Vec<&str> = rest.split_whitespace().collect();
                let mut bytes = Vec::new();
                for hex in &hex_parts {
                    let b = u8::from_str_radix(hex, 16)
                        .map_err(|e| format!("line {line_num}: bad hex byte '{hex}': {e}"))?;
                    bytes.push(b);
                }
                log.push(format!(">>> sendraw [{}]", hex_parts.join(" ")));
                sess.write_bytes(&bytes)
                    .map_err(|e| format!("line {line_num}: sendraw failed: {e}"))?;
            }

            "signal" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: signal before spawn"))?;
                let sig = parse_signal(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> signal {rest}"));
                sess.send_signal(sig)
                    .map_err(|e| format!("line {line_num}: signal failed: {e}"))?;
            }

            "sendeof" => {
                let sess = session
                    .as_mut()
                    .ok_or_else(|| format!("line {line_num}: sendeof before spawn"))?;
                log.push(">>> sendeof".to_string());
                sess.send_eof()
                    .map_err(|e| format!("line {line_num}: sendeof failed: {e}"))?;
            }

            "wait" => {
                let sess = session
                    .as_mut()
                    .ok_or_else(|| format!("line {line_num}: wait before spawn"))?;
                let expected_code = if let Some(val) = rest.strip_prefix("exitcode=") {
                    Some(
                        val.parse::<i32>()
                            .map_err(|e| format!("line {line_num}: bad exitcode: {e}"))?,
                    )
                } else if rest.is_empty() {
                    None
                } else {
                    return Err(format!("line {line_num}: unknown wait argument: {rest}"));
                };
                log.push(format!(">>> wait exitcode={:?}", expected_code));
                match sess.wait_child(expected_code) {
                    Ok(code) => {
                        log.push(format!("<<< child exited with code {code}"));
                    }
                    Err(e) => {
                        dump_log(&log);
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
                session = None;
            }

            "sleep" => {
                let dur = parse_timeout_value(rest).map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> sleep {}ms", dur.as_millis()));
                thread::sleep(dur);
            }

            _ => {
                return Err(format!("line {line_num}: unknown command: {cmd}"));
            }
        }
    }

    if let Some(mut sess) = session {
        vlog!("suite: test ended with live session, sending EOF and waiting");
        let _ = sess.send_eof();
        let _ = sess.wait_child(None);
    }

    Ok(())
}

fn dump_log(log: &[String]) {
    eprintln!("--- expect_pty conversation log ---");
    for entry in log {
        eprintln!("{entry}");
    }
    eprintln!("--- end log ---");
}

// ── Per-test isolation ───────────────────────────────────────────────────────

use std::sync::atomic::AtomicU64;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn run_test_isolated(
    test: &TestCase,
    shell_argv: &[String],
    shell_str: &str,
    script_modes: &[ScriptMode],
    locale_dir: Option<&str>,
    timeout: Duration,
    cgroup_base: Option<&str>,
) -> TestReport {
    let test_id = format!(
        "{}_{}_{}",
        std::process::id(),
        TEST_COUNTER.fetch_add(1, AtomicOrdering::Relaxed),
        test.name.replace(|c: char| !c.is_alphanumeric(), "_")
    );

    let cgroup_path = cgroup_base.and_then(|base| create_test_cgroup(base, &test_id).ok());

    let mut pipe_fds = [0 as CInt; 2];
    if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } != 0 {
        return TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Fail,
            error: Some("failed to create IPC pipe".to_string()),
        };
    }
    let pipe_r = pipe_fds[0];
    let pipe_w = pipe_fds[1];

    vlog!("run_test_isolated: starting test {:?}", test.name);
    let child_pid = unsafe { libc::fork() };
    if child_pid < 0 {
        unsafe {
            libc::close(pipe_r);
            libc::close(pipe_w);
        }
        if let Some(ref cg) = cgroup_path {
            remove_cgroup(cg);
        }
        return TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Fail,
            error: Some("fork() failed".to_string()),
        };
    }

    if child_pid == 0 {
        // ── Child process ──
        unsafe {
            libc::setsid();
            #[cfg(target_os = "linux")]
            libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
            #[cfg(target_os = "freebsd")]
            libc::procctl(
                libc::P_PID,
                0,
                libc::PROC_REAP_ACQUIRE,
                std::ptr::null_mut(),
            );
            libc::close(pipe_r);
        }

        if let Some(ref cg) = cgroup_path {
            let _ = move_to_cgroup(cg, std::process::id());
        }

        match run_suite_test(test, shell_argv, shell_str, script_modes, locale_dir) {
            Ok(()) => {
                unsafe { libc::close(pipe_w) };
                unsafe { libc::_exit(0) };
            }
            Err(msg) => {
                let bytes = msg.as_bytes();
                unsafe {
                    libc::write(pipe_w, bytes.as_ptr() as *const libc::c_void, bytes.len());
                    libc::close(pipe_w);
                    libc::_exit(1);
                }
            }
        }
    }

    // ── Parent process ──
    vlog!("run_test_isolated: forked isolation child_pid={child_pid} for {:?}", test.name);
    unsafe { libc::close(pipe_w) };

    let deadline = Instant::now() + timeout;
    let mut status: CInt = 0;
    let mut reaped = false;
    let mut last_progress = Instant::now();

    loop {
        let ret = unsafe { libc::waitpid(child_pid, &mut status, libc::WNOHANG) };
        if ret == child_pid {
            reaped = true;
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        if verbose() && last_progress.elapsed() >= Duration::from_secs(1) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            vlog!("run_test_isolated: still waiting for child_pid={child_pid} ({:.1}s remaining)", remaining.as_secs_f64());
            last_progress = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    if !reaped {
        vlog!("run_test_isolated: TIMEOUT for {:?}, killing child_pid={child_pid}", test.name);
        if let Some(ref cg) = cgroup_path {
            let _ = kill_cgroup(cg);
        } else {
            vlog!("run_test_isolated: sending SIGKILL to session -{child_pid}");
            unsafe { libc::kill(-child_pid, libc::SIGKILL) };
        }
        vlog!("run_test_isolated: blocking waitpid for child_pid={child_pid}");
        unsafe { libc::waitpid(child_pid, &mut status, 0) };
        vlog!("run_test_isolated: child {child_pid} reaped after timeout");

        let pipe_read = unsafe { std::fs::File::from_raw_fd(pipe_r) };
        drop(pipe_read);

        if let Some(ref cg) = cgroup_path {
            remove_cgroup(cg);
        }
        return TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Fail,
            error: Some(format!("timed out after {}s", timeout.as_secs_f64())),
        };
    }

    let mut error_buf = String::new();
    {
        let mut pipe_read = unsafe { std::fs::File::from_raw_fd(pipe_r) };
        let _ = pipe_read.read_to_string(&mut error_buf);
    }

    if let Some(ref cg) = cgroup_path {
        remove_cgroup(cg);
    }

    if wifexited(status) && wexitstatus(status) == 0 {
        vlog!("run_test_isolated: {:?} PASSED", test.name);
        TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Pass,
            error: None,
        }
    } else if !error_buf.is_empty() {
        vlog!("run_test_isolated: {:?} FAILED: {}", test.name, error_buf.lines().next().unwrap_or(""));
        TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Fail,
            error: Some(error_buf),
        }
    } else {
        vlog!("run_test_isolated: {:?} FAILED with status {status}", test.name);
        TestReport {
            name: test.name.clone(),
            outcome: TestOutcome::Fail,
            error: Some(format!("child exited with status {status}")),
        }
    }
}

// ── Suite runner ─────────────────────────────────────────────────────────────

fn run_suite(
    suite: &TestSuite,
    shell_argv: &[String],
    shell_str: &str,
    script_modes: &[ScriptMode],
    locale_dir: Option<&str>,
    timeout: Duration,
    cgroup_base: Option<&str>,
) -> Vec<TestReport> {
    let mut reports = Vec::new();
    for test in &suite.tests {
        let report = run_test_isolated(
            test,
            shell_argv,
            shell_str,
            script_modes,
            locale_dir,
            timeout,
            cgroup_base,
        );
        reports.push(report);
    }
    reports
}

fn print_suite_report(suite: &TestSuite, reports: &[TestReport]) -> (usize, usize) {
    eprintln!("=== {} ({}) ===", suite.name, suite.filename);
    let mut passed = 0;
    let mut failed = 0;
    for r in reports {
        match r.outcome {
            TestOutcome::Pass => {
                eprintln!("  PASS  {}", r.name);
                passed += 1;
            }
            TestOutcome::Fail => {
                eprintln!("  FAIL  {}", r.name);
                if let Some(ref e) = r.error {
                    for eline in e.lines() {
                        eprintln!("        {eline}");
                    }
                }
                failed += 1;
            }
        }
    }
    eprintln!("--- {passed} passed, {failed} failed ---");
    eprintln!();
    (passed, failed)
}

// ── CLI parsing ──────────────────────────────────────────────────────────────

fn parse_shell_arg(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn apply_test_filter(suites: &mut [(String, TestSuite)], test_name: &str) -> usize {
    let mut selected = 0usize;
    for (_file, suite) in suites.iter_mut() {
        suite.tests.retain(|t| t.name == test_name);
        selected += suite.tests.len();
    }
    selected
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut shell_flag: Option<String> = None;
    let mut script_modes_flag: Option<String> = None;
    let mut test_filter: Option<String> = None;
    let mut timeout_flag: Option<String> = None;
    let mut parse_only = false;
    let mut verbose_flag = false;
    let mut files: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--shell" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("expect_pty: --shell requires an argument");
                    std::process::exit(2);
                }
                shell_flag = Some(args[i].clone());
            }
            "--script-modes" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("expect_pty: --script-modes requires an argument");
                    std::process::exit(2);
                }
                script_modes_flag = Some(args[i].clone());
            }
            "--test" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("expect_pty: --test requires a test name argument");
                    std::process::exit(2);
                }
                test_filter = Some(args[i].clone());
            }
            "--timeout" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("expect_pty: --timeout requires a duration (e.g. 10s, 500ms)");
                    std::process::exit(2);
                }
                timeout_flag = Some(args[i].clone());
            }
            "--parse-only" => {
                parse_only = true;
            }
            "--verbose" | "-v" => {
                verbose_flag = true;
            }
            arg if arg.starts_with('-') && arg != "-" => {
                eprintln!("expect_pty: unknown flag: {arg}");
                std::process::exit(2);
            }
            _ => {
                files.push(args[i].clone());
            }
        }
        i += 1;
    }

    if verbose_flag {
        VERBOSE.store(true, AtomicOrdering::Relaxed);
    }

    let has_epty = files
        .iter()
        .any(|f| f.ends_with(".epty") || f.ends_with(".md"));

    if has_epty {
        // Suite mode
        let shell_str = shell_flag
            .or_else(|| env::var("TARGET_SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());
        let shell_argv = parse_shell_arg(&shell_str);
        if shell_argv.is_empty() || !shell_argv[0].starts_with('/') {
            eprintln!(
                "expect_pty: --shell must be an absolute path, got: {}",
                shell_argv.first().map(|s| s.as_str()).unwrap_or("<empty>")
            );
            std::process::exit(2);
        }
        let script_modes = match script_modes_flag {
            Some(ref s) => match parse_script_modes(s) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("expect_pty: {e}");
                    std::process::exit(2);
                }
            },
            None => vec![ScriptMode::DashC],
        };

        let test_timeout = match timeout_flag {
            Some(ref s) => match parse_timeout_value(s) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("expect_pty: --timeout: {e}");
                    std::process::exit(2);
                }
            },
            None => Duration::from_secs(10),
        };

        // Attempt to compile the test locale for locale-dependent tests
        let locale_dir = compile_test_locale();
        let locale_dir_ref = locale_dir.as_deref();

        let cgroup_base = discover_cgroup_base();
        if cgroup_base.is_none() {
            eprintln!(
                "expect_pty: cgroup v2 unavailable, falling back to session-group kill on timeout"
            );
        }
        let cgroup_base_ref = cgroup_base.as_deref();

        let mut total_passed: usize = 0;
        let mut total_failed: usize = 0;
        let mut suites_passed: usize = 0;
        let mut suites_failed: usize = 0;

        let mut suites: Vec<(String, TestSuite)> = Vec::new();
        let mut parse_errors = 0;

        for file in &files {
            let text = fs::read_to_string(file).unwrap_or_else(|e| {
                eprintln!("expect_pty: cannot read {file}: {e}");
                std::process::exit(2);
            });
            let filename = file.rsplit('/').next().unwrap_or(file);
            match parse_suite(&text, filename) {
                Ok(s) => {
                    suites.push((file.clone(), s));
                }
                Err(e) => {
                    eprintln!("expect_pty: parse error in {file}: {e}");
                    parse_errors += 1;
                }
            }
        }

        if parse_errors > 0 {
            if parse_only {
                eprintln!("{parse_errors} file(s) had parse errors");
            }
            std::process::exit(if parse_only { 1 } else { 2 });
        }

        if let Some(ref wanted_test) = test_filter {
            let selected = apply_test_filter(&mut suites, wanted_test);
            if selected == 0 {
                eprintln!(
                    "expect_pty: no test named {:?} found in provided .epty files",
                    wanted_test
                );
                std::process::exit(2);
            }
        }

        if parse_only {
            eprintln!(
                "all {} file(s) parsed OK ({} tests)",
                files.len(),
                suites.iter().map(|(_, s)| s.tests.len()).sum::<usize>()
            );
            std::process::exit(0);
        }

        for (file, suite) in &suites {
            let _ = file;
            if suite.tests.is_empty() {
                continue;
            }
            let reports = run_suite(
                suite,
                &shell_argv,
                &shell_str,
                &script_modes,
                locale_dir_ref,
                test_timeout,
                cgroup_base_ref,
            );
            let (p, f) = print_suite_report(suite, &reports);
            total_passed += p;
            total_failed += f;
            if f == 0 {
                suites_passed += 1;
            } else {
                suites_failed += 1;
            }
        }

        let total_suites = suites_passed + suites_failed;
        let total_tests = total_passed + total_failed;
        eprintln!("=== Summary ===");
        eprintln!("Suites: {suites_passed} passed, {suites_failed} failed (of {total_suites})");
        eprintln!("Tests:  {total_passed} passed, {total_failed} failed (of {total_tests})");

        if total_failed > 0 {
            std::process::exit(1);
        }
    } else {
        // Legacy mode: read from file or stdin
        let script_text = if files.len() == 1 {
            fs::read_to_string(&files[0]).unwrap_or_else(|e| {
                eprintln!("expect_pty: cannot read {}: {e}", files[0]);
                std::process::exit(2);
            })
        } else if files.is_empty() {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
                eprintln!("expect_pty: cannot read stdin: {e}");
                std::process::exit(2);
            });
            buf
        } else {
            eprintln!("expect_pty: multiple files require .epty extension for suite mode");
            std::process::exit(2);
        };

        let lines: Vec<String> = script_text.lines().map(String::from).collect();

        match run_script(&lines) {
            Ok(()) => {
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("expect_pty: FAIL: {e}");
                std::process::exit(1);
            }
        }
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- regex engine --

    #[test]
    fn regex_literal() {
        let pat = parse_regex("hello").unwrap();
        assert!(regex_find(&pat, "hello").is_some());
        assert!(regex_find(&pat, "say hello world").is_some());
        assert!(regex_find(&pat, "helxo").is_none());
    }

    #[test]
    fn regex_dot() {
        let pat = parse_regex("h.llo").unwrap();
        assert!(regex_find(&pat, "hello").is_some());
        assert!(regex_find(&pat, "hallo").is_some());
        assert!(regex_find(&pat, "hllo").is_none());
    }

    #[test]
    fn regex_star() {
        let pat = parse_regex("ab*c").unwrap();
        assert!(regex_find(&pat, "ac").is_some());
        assert!(regex_find(&pat, "abc").is_some());
        assert!(regex_find(&pat, "abbc").is_some());
        assert!(regex_find(&pat, "abbbc").is_some());
    }

    #[test]
    fn regex_plus() {
        let pat = parse_regex("ab+c").unwrap();
        assert!(regex_find(&pat, "ac").is_none());
        assert!(regex_find(&pat, "abc").is_some());
        assert!(regex_find(&pat, "abbc").is_some());
    }

    #[test]
    fn regex_question() {
        let pat = parse_regex("ab?c").unwrap();
        assert!(regex_find(&pat, "ac").is_some());
        assert!(regex_find(&pat, "abc").is_some());
        assert!(regex_find(&pat, "abbc").is_none());
    }

    #[test]
    fn regex_dot_star() {
        let pat = parse_regex("a.*b").unwrap();
        assert!(regex_find(&pat, "ab").is_some());
        assert!(regex_find(&pat, "aXb").is_some());
        assert!(regex_find(&pat, "aXYZb").is_some());
        assert!(regex_find(&pat, "a").is_none());
    }

    #[test]
    fn regex_dot_plus() {
        let pat = parse_regex("a.+b").unwrap();
        assert!(regex_find(&pat, "ab").is_none());
        assert!(regex_find(&pat, "aXb").is_some());
        assert!(regex_find(&pat, "aXYb").is_some());
    }

    #[test]
    fn regex_alternation() {
        let pat = parse_regex("(cat|dog)").unwrap();
        assert!(regex_find(&pat, "I have a cat").is_some());
        assert!(regex_find(&pat, "I have a dog").is_some());
        assert!(regex_find(&pat, "I have a bird").is_none());
    }

    #[test]
    fn regex_alternation_three() {
        let pat = parse_regex("(a|bb|ccc)").unwrap();
        assert!(regex_find(&pat, "xa").is_some());
        assert!(regex_find(&pat, "xbb").is_some());
        assert!(regex_find(&pat, "xccc").is_some());
        assert!(regex_find(&pat, "xbc").is_none());
    }

    #[test]
    fn regex_bracket_digit() {
        let pat = parse_regex("[[:digit:]]+").unwrap();
        assert!(regex_find(&pat, "abc123def").is_some());
        assert!(regex_find(&pat, "no digits").is_none());
    }

    #[test]
    fn regex_bracket_range() {
        let pat = parse_regex("[a-z]+").unwrap();
        assert!(regex_find(&pat, "HELLO world").is_some());
        assert!(regex_find(&pat, "12345").is_none());
    }

    #[test]
    fn regex_escape() {
        let pat = parse_regex(r"\[1\]").unwrap();
        assert!(regex_find(&pat, "[1]").is_some());
        assert!(regex_find(&pat, "x[1]y").is_some());
        assert!(regex_find(&pat, "[2]").is_none());
    }

    #[test]
    fn regex_escape_dot() {
        let pat = parse_regex(r"a\.b").unwrap();
        assert!(regex_find(&pat, "a.b").is_some());
        assert!(regex_find(&pat, "aXb").is_none());
    }

    #[test]
    fn regex_escape_special() {
        let pat = parse_regex(r"\(\)\+\*\?").unwrap();
        assert!(regex_find(&pat, "()+*?").is_some());
    }

    #[test]
    fn regex_job_notification() {
        let pat = parse_regex(r"\[[[:digit:]]+\] [[:digit:]]+").unwrap();
        assert!(regex_find(&pat, "[1] 12345").is_some());
        assert!(regex_find(&pat, "foo [2] 99 bar").is_some());
        assert!(regex_find(&pat, "[a] 123").is_none());
    }

    #[test]
    fn regex_stopped_suspended() {
        let pat = parse_regex("(Stopped|Suspended)").unwrap();
        assert!(regex_find(&pat, "[1]+  Stopped  sleep 60").is_some());
        assert!(regex_find(&pat, "[1]+  Suspended  sleep 60").is_some());
        assert!(regex_find(&pat, "[1]+  Running  sleep 60").is_none());
    }

    #[test]
    fn regex_complex_job_status() {
        let pat = parse_regex(r"\[[[:digit:]]+\][+ ] (Running|Done|Stopped|Suspended)").unwrap();
        assert!(regex_find(&pat, "[1]+ Running  sleep 60").is_some());
        assert!(regex_find(&pat, "[2]  Done  ls").is_some());
        assert!(regex_find(&pat, "[1]+ Stopped  sleep 60").is_some());
    }

    #[test]
    fn regex_done_with_code() {
        let pat = parse_regex(r"Done\([[:digit:]]+\)").unwrap();
        assert!(regex_find(&pat, "[1]+  Done(2)  false").is_some());
        assert!(regex_find(&pat, "[1]+  Done(127)  nosuchcmd").is_some());
        assert!(regex_find(&pat, "[1]+  Done  true").is_none());
    }

    #[test]
    fn regex_find_returns_position() {
        let pat = parse_regex("foo").unwrap();
        assert_eq!(regex_find(&pat, "xxfooyyfoozz"), Some((2, 5)));
    }

    #[test]
    fn regex_find_empty_pattern() {
        let pat = parse_regex("").unwrap();
        assert_eq!(regex_find(&pat, "hello"), Some((0, 0)));
    }

    #[test]
    fn regex_nested_group() {
        let pat = parse_regex("((a|b)c)+").unwrap();
        assert!(regex_find(&pat, "ac").is_some());
        assert!(regex_find(&pat, "bc").is_some());
        assert!(regex_find(&pat, "acbc").is_some());
        assert!(regex_find(&pat, "cc").is_none());
    }

    #[test]
    fn regex_group_with_repeat() {
        let pat = parse_regex("(ab)+").unwrap();
        assert!(regex_find(&pat, "ab").is_some());
        assert!(regex_find(&pat, "abab").is_some());
        assert!(regex_find(&pat, "ba").is_none());
    }

    #[test]
    fn regex_backtracking() {
        let pat = parse_regex("a.*b.*c").unwrap();
        assert!(regex_find(&pat, "aXbYc").is_some());
        assert!(regex_find(&pat, "abc").is_some());
        assert!(regex_find(&pat, "aXbY").is_none());
    }

    #[test]
    fn regex_negated_bracket() {
        let pat = parse_regex("[^abc]+").unwrap();
        assert!(regex_find(&pat, "xyz").is_some());
        assert!(regex_find(&pat, "abc").is_none());
    }

    // -- parse_timeout_value --

    #[test]
    fn timeout_millis() {
        assert_eq!(
            parse_timeout_value("500ms").unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(
            parse_timeout_value("2000ms").unwrap(),
            Duration::from_millis(2000)
        );
    }

    #[test]
    fn timeout_seconds() {
        assert_eq!(parse_timeout_value("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_timeout_value("1s").unwrap(), Duration::from_secs(1));
    }

    #[test]
    fn timeout_no_suffix_fails() {
        assert!(parse_timeout_value("500").is_err());
    }

    // -- extract_quoted --

    #[test]
    fn quoted_simple() {
        assert_eq!(extract_quoted(r#""hello""#).unwrap(), "hello");
    }

    #[test]
    fn quoted_escaped_quote() {
        assert_eq!(extract_quoted(r#""say \"hi\"""#).unwrap(), r#"say "hi""#);
    }

    #[test]
    fn quoted_escaped_backslash() {
        assert_eq!(extract_quoted(r#""a\\b""#).unwrap(), r#"a\b"#);
    }

    #[test]
    fn quoted_escape_sequences() {
        assert_eq!(extract_quoted(r#""a\nb\t""#).unwrap(), "a\nb\t");
    }

    #[test]
    fn quoted_mixed() {
        assert_eq!(
            extract_quoted(r#""alias foo=\"echo aliased\"""#).unwrap(),
            r#"alias foo="echo aliased""#
        );
    }

    #[test]
    fn quoted_unterminated() {
        assert!(extract_quoted(r#""hello"#).is_err());
    }

    #[test]
    fn quoted_not_quoted() {
        assert!(extract_quoted("hello").is_err());
    }

    // -- extract_pattern --

    #[test]
    fn pattern_simple() {
        assert_eq!(extract_pattern(r#""hello""#).unwrap(), "hello");
    }

    #[test]
    fn pattern_backslash_verbatim() {
        // backslash passes through verbatim — no escaping
        assert_eq!(extract_pattern(r#""a\\b""#).unwrap(), r#"a\\b"#);
        assert_eq!(extract_pattern(r#""a\nb""#).unwrap(), r#"a\nb"#);
        assert_eq!(extract_pattern(r#""a\*b""#).unwrap(), r#"a\*b"#);
        assert_eq!(extract_pattern(r#""a\tb""#).unwrap(), r#"a\tb"#);
    }

    #[test]
    fn pattern_trailing_backslash() {
        assert_eq!(extract_pattern(r#""foo\\""#).unwrap(), r#"foo\\"#);
    }

    #[test]
    fn pattern_doubled_quote() {
        // "" inside pattern → literal "
        assert_eq!(extract_pattern(r#""say ""hi""""#).unwrap(), r#"say "hi""#);
    }

    #[test]
    fn pattern_regex_special_chars() {
        assert_eq!(
            extract_pattern(r#"".*foo[0-9]+""#).unwrap(),
            r#".*foo[0-9]+"#
        );
    }

    #[test]
    fn pattern_unterminated() {
        assert!(extract_pattern(r#""hello"#).is_err());
    }

    #[test]
    fn pattern_not_quoted() {
        assert!(extract_pattern("hello").is_err());
    }

    // -- .epty parser --

    #[test]
    fn parse_suite_basic() {
        let input = "\
testsuite \"My Suite\"

requirement \"REQ-001\" doc=\"Some requirement.\"

begin test \"first test\"
  script
    echo hello
  expect
    stdout \"hello\"
    stderr \"\"
    exit_code 0
end test \"first test\"

requirement \"REQ-002\" doc=\"Another requirement.\"

begin interactive test \"second test\"
  spawn -i
  expect \"$ \"
  sendeof
  wait
end interactive test \"second test\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.name, "My Suite");
        assert_eq!(suite.tests.len(), 2);
        assert_eq!(suite.tests[0].name, "first test");
        assert!(!suite.tests[0].interactive);
        assert_eq!(suite.tests[0].script.as_deref(), Some("echo hello"));
        assert_eq!(suite.tests[0].requirements.len(), 1);
        assert_eq!(suite.tests[0].requirements[0].id, "REQ-001");
        assert_eq!(suite.tests[1].name, "second test");
        assert!(suite.tests[1].interactive);
        assert!(suite.tests[1].script.is_none());
        assert_eq!(suite.tests[1].requirements.len(), 1);
    }

    #[test]
    fn parse_suite_setenv() {
        let input = "\
testsuite \"Env Suite\"

requirement \"REQ-001\" doc=\"Test requirement.\"

begin test \"with env\"
  setenv \"FOO\" \"bar\"
  setenv \"BAZ\" \"qux\"
  script
    echo $FOO
  expect
    stdout \"bar\"
    stderr \"\"
    exit_code 0
end test \"with env\"
";
        let suite = parse_suite(input, "env.epty").unwrap();
        assert_eq!(suite.tests[0].env_overrides.len(), 2);
        assert_eq!(
            suite.tests[0].env_overrides[0],
            ("FOO".into(), "bar".into())
        );
        assert_eq!(
            suite.tests[0].env_overrides[1],
            ("BAZ".into(), "qux".into())
        );
    }

    #[test]
    fn parse_suite_mismatched_end() {
        let input = "\
testsuite \"Bad\"
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"alpha\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"beta\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_unterminated_test() {
        let input = "\
testsuite \"Bad\"
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"alpha\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_nested_test() {
        let input = "\
testsuite \"Bad\"
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"alpha\"
  begin test \"beta\"
  end test \"beta\"
end test \"alpha\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_mismatched_interactive() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin interactive test \"alpha\"
  spawn -i
end test \"alpha\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_no_testsuite_uses_filename() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"lone\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"lone\"
";
        let suite = parse_suite(input, "my_file.epty").unwrap();
        assert_eq!(suite.name, "my_file.epty");
    }

    #[test]
    fn parse_suite_multiple_requirements() {
        let input = "\
testsuite \"Multi Req\"

requirement \"REQ-001\" doc=\"First requirement.\"
requirement \"REQ-002\" doc=\"Something.\"

begin test \"covered\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"covered\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.tests[0].requirements.len(), 2);
        assert_eq!(suite.tests[0].requirements[0].id, "REQ-001");
        assert_eq!(suite.tests[0].requirements[0].doc, "First requirement.");
        assert_eq!(suite.tests[0].requirements[1].id, "REQ-002");
        assert_eq!(suite.tests[0].requirements[1].doc, "Something.");
    }

    #[test]
    fn parse_suite_requirement_missing_doc_fails() {
        let input = "\
requirement \"REQ-001\"
begin test \"bare\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"bare\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(
            err.contains("missing doc"),
            "expected 'missing doc' error, got: {err}"
        );
    }

    #[test]
    fn parse_suite_doc_allows_lowercase_start() {
        let input = "\
requirement \"REQ-001\" doc=\"lowercase start.\"
begin test \"t\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"t\"
";
        let suite = parse_suite(input, "bad.epty").unwrap();
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].requirements[0].doc, "lowercase start.");
    }

    #[test]
    fn parse_suite_doc_allows_missing_terminal_period() {
        let input = "\
requirement \"REQ-001\" doc=\"No period at end\"
begin test \"t\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"t\"
";
        let suite = parse_suite(input, "bad.epty").unwrap();
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].requirements[0].doc, "No period at end");
    }

    #[test]
    fn parse_suite_doc_must_not_end_with_colon_dot() {
        let input = "\
requirement \"REQ-001\" doc=\"Trailing colon:.\"
begin test \"t\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"t\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(
            err.contains("must not end with"),
            "expected colon-dot error, got: {err}"
        );
    }

    #[test]
    fn parse_suite_no_requirement_is_allowed() {
        let input = "\
begin test \"bare\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"bare\"
";
        let suite = parse_suite(input, "bad.epty").unwrap();
        assert_eq!(suite.tests.len(), 1);
        assert!(suite.tests[0].requirements.is_empty());
    }

    #[test]
    fn parse_suite_allows_more_than_three_requirements() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
requirement \"REQ-002\" doc=\"Test requirement.\"
requirement \"REQ-003\" doc=\"Test requirement.\"
requirement \"REQ-004\" doc=\"Test requirement.\"
begin test \"overloaded\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"overloaded\"
";
        let suite = parse_suite(input, "bad.epty").unwrap();
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].requirements.len(), 4);
    }

    #[test]
    fn parse_suite_three_requirements_ok() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
requirement \"REQ-002\" doc=\"Test requirement.\"
requirement \"REQ-003\" doc=\"Test requirement.\"
begin test \"three\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"three\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.tests[0].requirements.len(), 3);
    }

    #[test]
    fn parse_suite_multiline_script() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"multi\"
  script
    echo line1
    echo line2
    echo line3
  expect
    stdout \"line1\\nline2\\nline3\"
    stderr \"\"
    exit_code 0
end test \"multi\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(
            suite.tests[0].script.as_deref(),
            Some("echo line1\necho line2\necho line3")
        );
    }

    #[test]
    fn parse_suite_script_with_blank_lines() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"blanks\"
  script
    echo before

    echo after
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"blanks\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(
            suite.tests[0].script.as_deref(),
            Some("echo before\n\necho after")
        );
    }

    #[test]
    fn parse_suite_script_no_quoting_needed() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"quotes\"
  script
    echo \"hello world\" '$var' $(cmd)
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"quotes\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(
            suite.tests[0].script.as_deref(),
            Some("echo \"hello world\" '$var' $(cmd)")
        );
    }

    #[test]
    fn parse_suite_no_script_in_noninteractive_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"no script\"
end test \"no script\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_script_in_interactive_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin interactive test \"bad\"
  script
    echo hello
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end interactive test \"bad\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_trailing_whitespace_fails() {
        let input = "begin test \"ws\" \n  begin script\n    true\n  end script\nend test \"ws\"\n";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_unterminated_script_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"bad\"
  script
    echo hello
end test \"bad\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_script_modes_test() {
        assert_eq!(
            parse_script_modes("dash-c").unwrap(),
            vec![ScriptMode::DashC]
        );
        assert_eq!(
            parse_script_modes("tempfile").unwrap(),
            vec![ScriptMode::Tempfile]
        );
        assert_eq!(
            parse_script_modes("stdin").unwrap(),
            vec![ScriptMode::Stdin]
        );
        assert_eq!(
            parse_script_modes("dash-c,tempfile,stdin").unwrap(),
            vec![ScriptMode::DashC, ScriptMode::Tempfile, ScriptMode::Stdin]
        );
        assert!(parse_script_modes("bad").is_err());
        assert_eq!(
            parse_script_modes("dash-c,dash-c").unwrap(),
            vec![ScriptMode::DashC]
        );
    }

    #[test]
    fn apply_test_filter_selects_matching_tests() {
        let s1 = parse_suite(
            "testsuite \"S1\"
requirement \"REQ-1\" doc=\"Doc one.\"
begin test \"alpha\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"alpha\"
requirement \"REQ-2\" doc=\"Doc two.\"
begin test \"beta\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"beta\"
",
            "s1.epty",
        )
        .unwrap();

        let s2 = parse_suite(
            "testsuite \"S2\"
requirement \"REQ-3\" doc=\"Doc three.\"
begin test \"alpha\"
  script
    true
  expect
    stdout \"\"
    stderr \"\"
    exit_code 0
end test \"alpha\"
",
            "s2.epty",
        )
        .unwrap();

        let mut suites = vec![("a".to_string(), s1), ("b".to_string(), s2)];
        let selected = apply_test_filter(&mut suites, "alpha");
        assert_eq!(selected, 2);
        assert_eq!(suites[0].1.tests.len(), 1);
        assert_eq!(suites[0].1.tests[0].name, "alpha");
        assert_eq!(suites[1].1.tests.len(), 1);
        assert_eq!(suites[1].1.tests[0].name, "alpha");
    }
}
