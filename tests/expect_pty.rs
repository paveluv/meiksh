//! expect_pty — A scriptable PTY driver for interactive shell testing.
//!
//! Reads a line-oriented conversation script from stdin (or a file argument)
//! and executes it against a child process running inside a real pseudo-terminal.
//!
//! DSL commands:
//!   spawn <cmd> [args...]                 — fork a PTY child and exec the command
//!   expect "regex"                        — wait for regex match in PTY output (default 5s)
//!   expect timeout=2s "regex"             — wait with per-command timeout (ms or s suffix)
//!   not_expect "regex"                    — assert regex does NOT match current output buffer
//!   not_expect timeout=500ms "regex"      — timed negative: watch for duration, fail if matched
//!   expect_line "regex"                   — wait for the next complete line matching regex
//!   expect_line timeout=1s "regex"        — with per-command timeout
//!   send "text"                           — write text + newline to the PTY
//!   sendraw <hex> [<hex>...]              — write raw bytes (hex-encoded) to the PTY
//!   signal <SIGNAME>                      — send a signal to the child's process group
//!   sendeof                               — close the write side of the PTY
//!   wait exitcode=N                       — wait for child exit and assert exit code
//!   sleep <duration>                       — sleep (e.g. 100ms or 1s)
//!
//! Lines starting with '#' and empty lines are ignored.
//! All expect commands use a built-in regex engine (no external dependencies).
//! Supported regex syntax: . * + ? [...] (a|b) \escape

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
        if argv.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty argv"));
        }

        unsafe {
            let mut master: CInt = -1;

            // Properly initialize termios for a sane terminal
            let mut termp: libc::termios = std::mem::zeroed();
            termp.c_iflag = (libc::ICRNL | libc::IXON) as libc::tcflag_t;
            termp.c_oflag = (libc::OPOST | libc::ONLCR) as libc::tcflag_t;
            termp.c_cflag = (libc::CS8 | libc::CREAD | libc::CLOCAL) as libc::tcflag_t;
            termp.c_lflag =
                (libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ICANON | libc::ISIG | libc::IEXTEN)
                    as libc::tcflag_t;
            // Special characters
            termp.c_cc[libc::VINTR] = 0x03; // Ctrl-C
            termp.c_cc[libc::VQUIT] = 0x1c; // Ctrl-backslash
            termp.c_cc[libc::VERASE] = 0x7f; // DEL
            termp.c_cc[libc::VKILL] = 0x15; // Ctrl-U
            termp.c_cc[libc::VEOF] = 0x04; // Ctrl-D
            termp.c_cc[libc::VSUSP] = 0x1a; // Ctrl-Z
            termp.c_cc[libc::VMIN] = 1;
            termp.c_cc[libc::VTIME] = 0;
            // Set baud rate
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
                // Child — set env vars and exec the command
                let mut cmd = Command::new(&argv[0]);
                cmd.args(&argv[1..]);
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

/// Extract a quoted string argument: `"contents"` -> `contents`
fn extract_quoted(arg: &str) -> Result<&str, String> {
    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
        Ok(&arg[1..arg.len() - 1])
    } else {
        Err(format!("expected quoted string, got: {arg}"))
    }
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

        // Strip inline comments (only outside quoted strings)
        let effective = if let Some(q_start) = line.find('"') {
            if let Some(q_end) = line[q_start + 1..].find('"') {
                let after_quote = q_start + 1 + q_end + 1;
                if let Some(hash_pos) = line[after_quote..].find('#') {
                    line[..after_quote + hash_pos].trim()
                } else {
                    line
                }
            } else {
                line
            }
        } else if let Some(hash_pos) = line.find('#') {
            line[..hash_pos].trim()
        } else {
            line
        };

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
                let pattern_str = extract_quoted(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> expect {:?} (timeout={:.1}s)", pattern_str, timeout.as_secs_f64()));
                match sess.expect(&pattern, pattern_str, timeout) {
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
                let pattern_str = extract_quoted(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> not_expect {:?} (timeout={:?})", pattern_str, timeout));
                sess.not_expect(&pattern, pattern_str, timeout)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
            }

            "expect_line" => {
                let sess = session
                    .as_ref()
                    .ok_or_else(|| format!("line {line_num}: expect_line before spawn"))?;
                let (timeout, quoted_part) = parse_expect_args(rest)?;
                let timeout = timeout.unwrap_or(Duration::from_millis(200));
                let pattern_str = extract_quoted(quoted_part)
                    .map_err(|e| format!("line {line_num}: {e}"))?;
                let pattern = parse_regex(pattern_str)
                    .map_err(|e| format!("line {line_num}: bad regex: {e}"))?;
                log.push(format!(">>> expect_line {:?} (timeout={:.1}s)", pattern_str, timeout.as_secs_f64()));
                match sess.expect_line(&pattern, pattern_str, timeout) {
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
                let expanded = expand_env(text);
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

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    // Read script from file argument or stdin
    let script_text = if args.len() > 1 {
        fs::read_to_string(&args[1]).unwrap_or_else(|e| {
            eprintln!("expect_pty: cannot read {}: {e}", args[1]);
            std::process::exit(2);
        })
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
            eprintln!("expect_pty: cannot read stdin: {e}");
            std::process::exit(2);
        });
        buf
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
}
