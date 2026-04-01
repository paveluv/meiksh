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
//! ### Non-interactive script block (inside begin/end test):
//!   begin script                          — start a shell script block (col 2)
//!     <shell code>                        — script body (col 4, stripped to col 0)
//!   end script                            — end the script block (col 2)
//!
//!   The script body is taken verbatim (no quoting/escaping needed).
//!   $SHELL is set to the --shell value. Executed via --script-modes (default: dash-c).
//!   Tests run in an isolated sandbox directory; prefer local relative file paths
//!   (for example, `_tmp_file`) instead of `${TMPDIR:-/tmp}` indirections, and
//!   do not add explicit cleanup-only commands unless cleanup behavior itself is tested.
//!
//! ### Non-interactive assertions (after end script):
//!   expect_stdout "pattern"               — assert stdout matches regex
//!   expect_stderr "pattern"               — assert stderr matches regex
//!   expect_stdout_line "pattern"          — assert a stdout line matches
//!   expect_stderr_line "pattern"          — assert a stderr line matches
//!   expect_exit_code N                    — assert exit code equals N
//!   not_expect_stdout "pattern"           — assert stdout does NOT match
//!   not_expect_stderr "pattern"           — assert stderr does NOT match
//!   not_expect_exit_code N                — assert exit code does NOT equal N
//!
//! ### Interactive (PTY) commands (inside begin/end interactive test):
//!   spawn [flags...]                       — fork an interactive shell (shell from --shell, flags appended)
//!   expect "regex"                        — wait for regex match in PTY output
//!   expect timeout=2s "regex"             — with per-command timeout
//!   not_expect "regex"                    — assert regex does NOT match
//!   not_expect timeout=500ms "regex"      — timed negative assertion
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

#[path = "json.rs"]
#[allow(dead_code)]
mod json;
#[path = "epty_parser.rs"]
mod epty_parser;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

// ── PTY spawning ─────────────────────────────────────────────────────────────

struct PtySession {
    master_fd: RawFd,
    child_pid: libc::pid_t,
    buf: Arc<Mutex<Vec<u8>>>,
    reader_handle: Option<thread::JoinHandle<()>>,
    eof_sent: bool,
}

impl PtySession {
    fn spawn(argv: &[String], env_vars: &[(String, String)]) -> io::Result<Self> {
        Self::spawn_inner(argv, env_vars, false)
    }

    fn spawn_clean(argv: &[String], env_vars: &[(String, String)]) -> io::Result<Self> {
        Self::spawn_inner(argv, env_vars, true)
    }

    fn spawn_inner(
        argv: &[String],
        env_vars: &[(String, String)],
        clear_env: bool,
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

            let pid = forkpty(
                &mut master,
                std::ptr::null_mut(),
                &mut termp,
                &mut winp,
            );

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
                let err = cmd.exec();
                eprintln!("expect_pty: exec failed: {err}");
                std::process::exit(127);
            }

            // Parent — start reader thread
            let buf = Arc::new(Mutex::new(Vec::new()));
            let buf_clone = Arc::clone(&buf);
            let reader_fd = master;

            let reader_handle = thread::spawn(move || {
                let mut f = std::fs::File::from_raw_fd(reader_fd);
                let mut tmp = [0u8; 4096];
                loop {
                    match f.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut lock = buf_clone.lock().unwrap();
                            lock.extend_from_slice(&tmp[..n]);
                        }
                        Err(e) => {
                            if e.raw_os_error() == Some(libc::EIO) {
                                break; // child closed the PTY
                            }
                            break;
                        }
                    }
                }
                // Prevent the File from closing the fd — we manage it ourselves
                std::mem::forget(f);
            });

            Ok(PtySession {
                master_fd: master,
                child_pid: pid,
                buf,
                reader_handle: Some(reader_handle),
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
    fn expect(&self, pattern: &[RegexNode], pattern_str: &str, timeout: Duration) -> Result<String, String> {
        let start = Instant::now();
        loop {
            {
                let mut lock = self.buf.lock().unwrap();
                let haystack = String::from_utf8_lossy(&lock).to_string();
                if let Some((_match_start, match_end)) = regex_find(pattern, &haystack) {
                    let consumed = haystack[..match_end].to_string();
                    lock.drain(..match_end);
                    return Ok(consumed);
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

    /// Assert that a regex does NOT match anywhere in the current buffer.
    /// With timeout: watch for the given duration, fail if pattern appears.
    /// Without timeout: instant check against current buffer contents.
    fn not_expect(&self, pattern: &[RegexNode], pattern_str: &str, timeout: Option<Duration>) -> Result<(), String> {
        match timeout {
            None => {
                let lock = self.buf.lock().unwrap();
                let haystack = String::from_utf8_lossy(&lock).to_string();
                if regex_find(pattern, &haystack).is_some() {
                    Err(format!(
                        "not_expect: found {:?} in output:\n{}",
                        pattern_str, haystack
                    ))
                } else {
                    Ok(())
                }
            }
            Some(dur) => {
                let start = Instant::now();
                loop {
                    {
                        let lock = self.buf.lock().unwrap();
                        let haystack = String::from_utf8_lossy(&lock).to_string();
                        if regex_find(pattern, &haystack).is_some() {
                            return Err(format!(
                                "not_expect: found {:?} in output during {:.1}s watch:\n{}",
                                pattern_str, dur.as_secs_f64(), haystack
                            ));
                        }
                    }
                    if start.elapsed() >= dur {
                        return Ok(());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    /// Wait for the next complete line matching a regex pattern.
    /// Consumes non-matching lines while scanning forward.
    fn expect_line(&self, pattern: &[RegexNode], pattern_str: &str, timeout: Duration) -> Result<String, String> {
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
        // Close master fd so the PTY signals EOF to the child
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd); }
            self.master_fd = -1;
        }

        // Wait for the reader thread to finish (it will exit once the fd closes)
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }

        let mut status: CInt = 0;
        unsafe {
            let ret = libc::waitpid(self.child_pid, &mut status, 0);
            if ret < 0 {
                return Err(format!("waitpid failed: {}", io::Error::last_os_error()));
            }
        }

        let code = if wifexited(status) {
            wexitstatus(status)
        } else {
            128 + (status & 0x7f)
        };

        if let Some(expected) = expected_code {
            if code != expected {
                return Err(format!(
                    "wait: expected exit code {expected}, got {code}"
                ));
            }
        }
        Ok(code)
    }

    fn cleanup(&mut self) {
        // Kill the child so the reader thread can finish
        unsafe {
            libc::kill(self.child_pid, libc::SIGKILL);
        }
        // Close master fd so the reader thread's read() returns
        unsafe {
            libc::close(self.master_fd);
        }
        self.master_fd = -1;
        // Join the reader thread
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }
        // Reap child
        unsafe {
            let mut status: CInt = 0;
            libc::waitpid(self.child_pid, &mut status, 0);
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if self.master_fd >= 0 {
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

/// Parse the arguments of an expect command, handling optional timeout=Nms/Ns prefix.
/// Returns (None, pattern_str) when no timeout is given — caller picks default.
fn parse_expect_args(rest: &str) -> Result<(Option<Duration>, &str), String> {
    let rest = rest.trim();
    if let Some(after) = rest.strip_prefix("timeout=") {
        let space = after
            .find(' ')
            .ok_or_else(|| "timeout= must be followed by a space and a quoted string".to_string())?;
        let timeout = parse_timeout_value(&after[..space])?;
        let quoted = after[space..].trim();
        Ok((Some(timeout), quoted))
    } else {
        Ok((None, rest))
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
                if i + 1 < chars.len()
                    && chars[i] == '['
                    && matches!(chars[i + 1], ':' | '.' | '=')
                {
                    let delim = chars[i + 1];
                    bracket.push(chars[i]);
                    bracket.push(chars[i + 1]);
                    i += 2;
                    while i < chars.len() {
                        if chars[i] == delim
                            && i + 1 < chars.len()
                            && chars[i + 1] == ']'
                        {
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
            if pos < text.len() {
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
                let words: Vec<String> = expanded
                    .split_whitespace()
                    .map(String::from)
                    .collect();
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
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> expect {:?} (timeout={:.1}s)", pattern_str, timeout.as_secs_f64()));
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

            "not_expect" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: not_expect before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> not_expect {:?} (timeout={:?})", pattern_str, timeout));
                sess.not_expect(&pattern, &pattern_str, timeout)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
            }

            "expect_line" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect_line before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> expect_line {:?} (timeout={:.1}s)", pattern_str, timeout.as_secs_f64()));
                match sess.expect_line(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!("<<< line matched (consumed {} bytes)", consumed.len()));
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
                let text = extract_quoted(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
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
                log.push(format!(
                    ">>> sendraw [{}]",
                    hex_parts.join(" ")
                ));
                sess.write_bytes(&bytes)
                    .map_err(|e| format!("line {line_num}: sendraw failed: {e}"))?;
            }

            "signal" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: signal before spawn"))?;
                let sig = parse_signal(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
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
                let dur = parse_timeout_value(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
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
    epty_parser::parse_suite(text, filename)
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

fn baseline_env(tmpdir: &str, shell_str: &str, locale_dir: Option<&str>) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("PATH".into(), "/usr/bin:/bin".into());
    env.insert("HOME".into(), tmpdir.into());
    env.insert("TMPDIR".into(), tmpdir.into());
    env.insert("TERM".into(), "xterm".into());
    env.insert("LANG".into(), "C".into());
    env.insert("LC_ALL".into(), "C".into());
    if let Some(dir) = locale_dir {
        env.insert("LOCPATH".into(), dir.into());
    }
    env.insert("PS1".into(), "$ ".into());
    env.insert("PS2".into(), "> ".into());
    env.insert("ENV".into(), String::new());
    env.insert("HISTFILE".into(), "/dev/null".into());
    env.insert("SHELL".into(), shell_str.into());
    env
}

fn compile_test_locale() -> Option<String> {
    let locale_dir = "/tmp/epty_locale";
    let out_path = format!("{locale_dir}/test_EPTY.UTF-8");

    if std::path::Path::new(&out_path).is_dir() {
        return Some(locale_dir.to_string());
    }

    let _ = fs::create_dir_all(locale_dir);

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let def_path = format!("{manifest_dir}/tests/matrix/locale/test_EPTY.def");

    if !std::path::Path::new(&def_path).exists() {
        eprintln!("expect_pty: locale definition not found at {def_path}, skipping locale setup");
        return None;
    }

    let status = Command::new("localedef")
        .args(["-f", "UTF-8", "-i", &def_path, &out_path])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            eprintln!("expect_pty: compiled test locale to {out_path}");
            Some(locale_dir.to_string())
        }
        _ => {
            eprintln!("expect_pty: localedef not available or failed, skipping locale tests");
            None
        }
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
            cmd.current_dir(tmpdir);
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
            cmd.current_dir(tmpdir);
            let result = cmd.output()
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
            cmd.current_dir(tmpdir);
            let mut child = cmd.spawn()
                .map_err(|e| format!("failed to execute shell (stdin): {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(script.as_bytes());
            }
            child.wait_with_output()
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
    let tmpdir = make_test_tmpdir()
        .map_err(|e| format!("failed to create tmpdir: {e}"))?;

    let result = run_suite_test_inner(test, shell_argv, shell_str, &tmpdir, script_modes, locale_dir);
    remove_dir_all(&tmpdir);
    result
}

fn run_suite_test_inner(
    test: &TestCase,
    shell_argv: &[String],
    shell_str: &str,
    tmpdir: &str,
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
        log.push(format!(">>> script ({} modes) {:?}", script_modes.len(),
            if script.len() > 80 { format!("{}...", &script[..80]) } else { script.clone() }));
        let mut reference: Option<RunResult> = None;
        for &mode in script_modes {
            let rr = run_command(script, shell_argv, &test_env, tmpdir, mode)?;
            if let Some(ref prev) = reference {
                if rr.stdout != prev.stdout || rr.stderr != prev.stderr || rr.exit_code != prev.exit_code {
                    return Err(format!(
                        "script mode divergence: {:?} vs {:?}\n\
                         --- {:?} ---\nstdout: {:?}\nstderr: {:?}\nexit: {}\n\
                         --- {:?} ---\nstdout: {:?}\nstderr: {:?}\nexit: {}",
                        script_modes[0], mode,
                        script_modes[0], prev.stdout, prev.stderr, prev.exit_code,
                        mode, rr.stdout, rr.stderr, rr.exit_code,
                    ));
                }
            } else {
                reference = Some(rr);
            }
        }
        last_run = reference;
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
                session = Some(
                    PtySession::spawn_clean(&words, &env_pairs)
                        .map_err(|e| format!("line {line_num}: spawn failed: {e}"))?,
                );
            }

            "expect_stdout" | "expect_stderr" | "expect_stdout_line"
            | "expect_stderr_line" | "not_expect_stdout" | "not_expect_stderr"
            | "not_expect_stdout_line" | "not_expect_stderr_line" => {
                let rr = last_run
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: {cmd} before script"))?;
                let pattern_str = extract_pattern(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                let (haystack, label) = if cmd.contains("stderr") {
                    (&rr.stderr, "stderr")
                } else {
                    (&rr.stdout, "stdout")
                };
                log.push(format!(">>> {cmd} {:?}", pattern_str));

                let negate = cmd.starts_with("not_");
                let line_mode = cmd.contains("_line");

                if line_mode {
                    let found = haystack
                        .lines()
                        .any(|l| regex_full_match(&pattern, l));
                    if !negate && !found {
                        return Err(format!(
                            "line {line_num}: {cmd}: no {label} line matched {:?}\n{label}:\n{}",
                            pattern_str, haystack
                        ));
                    }
                    if negate && found {
                        return Err(format!(
                            "line {line_num}: {cmd}: found {:?} in {label}\n{label}:\n{}",
                            pattern_str, haystack
                        ));
                    }
                } else {
                    let trimmed = haystack.trim_end();
                    let found = regex_full_match(&pattern, trimmed);
                    if !negate && !found {
                        return Err(format!(
                            "line {line_num}: {cmd}: {label} did not match {:?}\n{label}:\n{}",
                            pattern_str, haystack
                        ));
                    }
                    if negate && found {
                        return Err(format!(
                            "line {line_num}: {cmd}: {label} matched {:?} (expected no match)\n{label}:\n{}",
                            pattern_str, haystack
                        ));
                    }
                }
            }

            "expect_exit_code" | "not_expect_exit_code" => {
                let rr = last_run
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: {cmd} before script"))?;
                let expected: i32 = rest
                    .parse()
                    .map_err(|e| format!("line {line_num}: bad exit code: {e}"))?;
                log.push(format!(">>> {cmd} {expected}"));
                let negate = cmd.starts_with("not_");
                if !negate && rr.exit_code != expected {
                    return Err(format!(
                        "line {line_num}: {cmd}: expected {expected}, got {}",
                        rr.exit_code
                    ));
                }
                if negate && rr.exit_code == expected {
                    return Err(format!(
                        "line {line_num}: {cmd}: did not expect {expected}",
                    ));
                }
            }

            // Interactive PTY commands — delegate to existing logic
            "expect" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
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
                        dump_log(&log);
                        return Err(format!("line {line_num}: {e}"));
                    }
                }
            }

            "not_expect" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: not_expect before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> not_expect {:?} (timeout={:?})", pattern_str, timeout));
                sess.not_expect(&pattern, &pattern_str, timeout)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
            }

            "expect_line" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect_line before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str = extract_pattern(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(&pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(
                    ">>> expect_line {:?} (timeout={:.1}s)",
                    pattern_str,
                    timeout.as_secs_f64()
                ));
                match sess.expect_line(&pattern, &pattern_str, timeout) {
                    Ok(consumed) => {
                        log.push(format!("<<< line matched (consumed {} bytes)", consumed.len()));
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
                let text = extract_quoted(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> send {:?}", text));
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
                let sig = parse_signal(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
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
                let dur = parse_timeout_value(rest)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                log.push(format!(">>> sleep {}ms", dur.as_millis()));
                thread::sleep(dur);
            }

            _ => {
                return Err(format!("line {line_num}: unknown command: {cmd}"));
            }
        }
    }

    if let Some(mut sess) = session {
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

// ── Suite runner ─────────────────────────────────────────────────────────────

fn run_suite(suite: &TestSuite, shell_argv: &[String], shell_str: &str, script_modes: &[ScriptMode], locale_dir: Option<&str>) -> Vec<TestReport> {
    let mut reports = Vec::new();
    for test in &suite.tests {
        let outcome = match run_suite_test(test, shell_argv, shell_str, script_modes, locale_dir) {
            Ok(()) => TestReport {
                name: test.name.clone(),
                outcome: TestOutcome::Pass,
                error: None,
            },
            Err(e) => TestReport {
                name: test.name.clone(),
                outcome: TestOutcome::Fail,
                error: Some(e),
            },
        };
        reports.push(outcome);
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

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut shell_flag: Option<String> = None;
    let mut script_modes_flag: Option<String> = None;
    let mut parse_only = false;
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
            "--parse-only" => {
                parse_only = true;
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

    let has_epty = files.iter().any(|f| f.ends_with(".epty"));

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

        // Attempt to compile the test locale for locale-dependent tests
        let locale_dir = compile_test_locale();
        let locale_dir_ref = locale_dir.as_deref();

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
            let filename = file
                .rsplit('/')
                .next()
                .unwrap_or(file);
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
            let reports = run_suite(suite, &shell_argv, &shell_str, &script_modes, locale_dir_ref);
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
        eprintln!(
            "Suites: {suites_passed} passed, {suites_failed} failed (of {total_suites})"
        );
        eprintln!(
            "Tests:  {total_passed} passed, {total_failed} failed (of {total_tests})"
        );

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
        assert_eq!(parse_timeout_value("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_timeout_value("2000ms").unwrap(), Duration::from_millis(2000));
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
        assert_eq!(extract_pattern(r#"".*foo[0-9]+""#).unwrap(), r#".*foo[0-9]+"#);
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
  begin script
    echo hello
  end script
  expect_stdout \"hello\"
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
  begin script
    echo $FOO
  end script
  expect_stdout \"bar\"
end test \"with env\"
";
        let suite = parse_suite(input, "env.epty").unwrap();
        assert_eq!(suite.tests[0].env_overrides.len(), 2);
        assert_eq!(suite.tests[0].env_overrides[0], ("FOO".into(), "bar".into()));
        assert_eq!(suite.tests[0].env_overrides[1], ("BAZ".into(), "qux".into()));
    }

    #[test]
    fn parse_suite_mismatched_end() {
        let input = "\
testsuite \"Bad\"
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"alpha\"
  begin script
    true
  end script
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
  begin script
    true
  end script
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
  begin script
    true
  end script
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
  begin script
    true
  end script
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
  begin script
    true
  end script
end test \"bare\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("missing doc"), "expected 'missing doc' error, got: {err}");
    }

    #[test]
    fn parse_suite_doc_must_start_uppercase() {
        let input = "\
requirement \"REQ-001\" doc=\"lowercase start.\"
begin test \"t\"
  begin script
    true
  end script
end test \"t\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("capital letter"), "expected 'capital letter' error, got: {err}");
    }

    #[test]
    fn parse_suite_doc_must_end_with_period() {
        let input = "\
requirement \"REQ-001\" doc=\"No period at end\"
begin test \"t\"
  begin script
    true
  end script
end test \"t\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("end with a period"), "expected 'period' error, got: {err}");
    }

    #[test]
    fn parse_suite_doc_must_not_end_with_colon_dot() {
        let input = "\
requirement \"REQ-001\" doc=\"Trailing colon:.\"
begin test \"t\"
  begin script
    true
  end script
end test \"t\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("must not end with"), "expected colon-dot error, got: {err}");
    }

    #[test]
    fn parse_suite_no_requirement_fails() {
        let input = "\
begin test \"bare\"
  begin script
    true
  end script
end test \"bare\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("no requirement"), "expected 'no requirement' error, got: {err}");
    }

    #[test]
    fn parse_suite_too_many_requirements_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
requirement \"REQ-002\" doc=\"Test requirement.\"
requirement \"REQ-003\" doc=\"Test requirement.\"
requirement \"REQ-004\" doc=\"Test requirement.\"
begin test \"overloaded\"
  begin script
    true
  end script
end test \"overloaded\"
";
        let err = parse_suite(input, "bad.epty").unwrap_err();
        assert!(err.contains("4 requirements (max 3)"), "expected max-3 error, got: {err}");
    }

    #[test]
    fn parse_suite_three_requirements_ok() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
requirement \"REQ-002\" doc=\"Test requirement.\"
requirement \"REQ-003\" doc=\"Test requirement.\"
begin test \"three\"
  begin script
    true
  end script
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
  begin script
    echo line1
    echo line2
    echo line3
  end script
  expect_stdout \"line1\nline2\nline3\"
end test \"multi\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.tests[0].script.as_deref(), Some("echo line1\necho line2\necho line3"));
    }

    #[test]
    fn parse_suite_script_with_blank_lines() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"blanks\"
  begin script
    echo before

    echo after
  end script
end test \"blanks\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.tests[0].script.as_deref(), Some("echo before\n\necho after"));
    }

    #[test]
    fn parse_suite_script_no_quoting_needed() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"quotes\"
  begin script
    echo \"hello world\" '$var' $(cmd)
  end script
end test \"quotes\"
";
        let suite = parse_suite(input, "test.epty").unwrap();
        assert_eq!(suite.tests[0].script.as_deref(), Some("echo \"hello world\" '$var' $(cmd)"));
    }

    #[test]
    fn parse_suite_no_script_in_noninteractive_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin test \"no script\"
  expect_exit_code 0
end test \"no script\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_suite_script_in_interactive_fails() {
        let input = "\
requirement \"REQ-001\" doc=\"Test requirement.\"
begin interactive test \"bad\"
  begin script
    echo hello
  end script
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
  begin script
    echo hello
end test \"bad\"
";
        assert!(parse_suite(input, "bad.epty").is_err());
    }

    #[test]
    fn parse_script_modes_test() {
        assert_eq!(parse_script_modes("dash-c").unwrap(), vec![ScriptMode::DashC]);
        assert_eq!(parse_script_modes("tempfile").unwrap(), vec![ScriptMode::Tempfile]);
        assert_eq!(parse_script_modes("stdin").unwrap(), vec![ScriptMode::Stdin]);
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

    // -- find_closing_quote --

    #[test]
    fn closing_quote_simple() {
        assert_eq!(find_closing_quote(r#""hello""#).unwrap(), 6);
    }

    #[test]
    fn closing_quote_escaped() {
        // "say \"hi\"" — the closing " is at byte index 11
        assert_eq!(find_closing_quote(r#""say \"hi\"""#).unwrap(), 11);
    }

    #[test]
    fn closing_quote_unterminated() {
        assert!(find_closing_quote(r#""hello"#).is_err());
    }

    // -- requirements integrity --

    fn make_req_entry(id: &str, text: &str, testable: bool, tests: &[(&str, &str)]) -> ReqEntry {
        ReqEntry {
            id: id.to_string(),
            text: text.to_string(),
            file: String::new(),
            section_path: Vec::new(),
            testable,
            tests: tests.iter().map(|(s, t)| (s.to_string(), t.to_string())).collect(),
        }
    }

    fn make_suite(name: &str, filename: &str, tests: Vec<(&str, Vec<(&str, &str)>)>) -> TestSuite {
        TestSuite {
            name: name.to_string(),
            filename: filename.to_string(),
            tests: tests
                .into_iter()
                .map(|(tname, reqs)| TestCase {
                    name: tname.to_string(),
                    interactive: false,
                    line_num: 1,
                    requirements: reqs
                        .into_iter()
                        .map(|(id, doc)| Requirement {
                            id: id.to_string(),
                            doc: doc.to_string(),
                        })
                        .collect(),
                    env_overrides: vec![],
                    script_lines: vec![],
                    script: Some("true".to_string()),
                })
                .collect(),
        }
    }

    #[test]
    fn integrity_ok() {
        let reqs = vec![make_req_entry(
            "REQ-1",
            "Some text.",
            true,
            &[("Suite A", "test one")],
        )];
        let suite = make_suite(
            "Suite A",
            "a.epty",
            vec![("test one", vec![("REQ-1", "Some text.")])],
        );
        let errs = check_requirements_integrity(&reqs, &[("a.epty".into(), suite)], None);
        assert!(errs.is_empty(), "expected no errors, got: {errs:?}");
    }

    #[test]
    fn integrity_doc_mismatch() {
        let reqs = vec![make_req_entry("REQ-1", "Correct text.", true, &[("S", "t")])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Wrong text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], None);
        assert!(errs.iter().any(|e| e.contains("doc mismatch")), "got: {errs:?}");
    }

    #[test]
    fn integrity_untestable() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", false, &[])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], None);
        assert!(errs.iter().any(|e| e.contains("untestable")), "got: {errs:?}");
    }

    #[test]
    fn integrity_req_not_in_json() {
        let reqs = vec![];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-X", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], None);
        assert!(errs.iter().any(|e| e.contains("not found in requirements.json")), "got: {errs:?}");
    }

    #[test]
    fn integrity_duplicate_ids() {
        let reqs = vec![
            make_req_entry("REQ-1", "First.", true, &[]),
            make_req_entry("REQ-1", "Second.", true, &[]),
        ];
        let errs = check_requirements_integrity(&reqs, &[], None);
        assert!(errs.iter().any(|e| e.contains("duplicate id")), "got: {errs:?}");
    }

    #[test]
    fn integrity_duplicate_texts() {
        let reqs = vec![
            make_req_entry("REQ-1", "Same text.", true, &[]),
            make_req_entry("REQ-2", "Same text.", true, &[]),
        ];
        let errs = check_requirements_integrity(&reqs, &[], None);
        assert!(errs.iter().any(|e| e.contains("duplicate text")), "got: {errs:?}");
    }

    #[test]
    fn integrity_testable_no_tests() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", true, &[])];
        let errs = check_requirements_integrity(&reqs, &[], None);
        assert!(errs.iter().any(|e| e.contains("has no tests linked")), "got: {errs:?}");
    }

    #[test]
    fn integrity_untestable_no_tests_ok() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", false, &[])];
        let errs = check_requirements_integrity(&reqs, &[], None);
        assert!(!errs.iter().any(|e| e.contains("has no tests linked")), "got: {errs:?}");
    }

    #[test]
    fn integrity_json_extra_test_pair() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", true, &[("S", "t"), ("S", "ghost")])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], None);
        assert!(errs.iter().any(|e| e.contains("ghost") && e.contains("no such link")), "got: {errs:?}");
    }

    #[test]
    fn integrity_json_missing_test_pair() {
        let reqs = vec![make_req_entry("REQ-1", "Text.", true, &[])];
        let suite = make_suite("S", "s.epty", vec![("t", vec![("REQ-1", "Text.")])]);
        let errs = check_requirements_integrity(&reqs, &[("s.epty".into(), suite)], None);
        assert!(errs.iter().any(|e| e.contains("is missing test")), "got: {errs:?}");
    }
}
