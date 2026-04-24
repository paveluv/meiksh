//! Backslash-escape decoder for bash-compat prompt expansion.
//!
//! This module implements the escape pass described in
//! `docs/features/ps1-prompt-extensions.md` § 5–§ 8. It consumes the
//! raw bytes of a prompt variable and produces a [`Prompt`] value
//! containing the rendered bytes plus an invisible-region mask for
//! `\[...\]` delimiters.
//!
//! The decoder is side-effect-free: every value it needs (hostname,
//! user, CWD, jobs count, time, ...) is pre-resolved into a
//! [`PromptEnv`] by the caller. This keeps the decoder pure and easily
//! unit-testable, and it keeps every `libc` call at the `sys::`
//! boundary (the caller populates `PromptEnv` through `sys::` helpers).

use crate::bstr;
use crate::sys::time::{LocalTime, format_strftime};

/// Rendered prompt with invisible-region metadata.
///
/// `invisible` is a sorted list of non-overlapping half-open byte
/// ranges, each `[start, end)`, denoting spans of `bytes` that were
/// introduced by `\[...\]` and must be excluded from the line editor's
/// cursor-column accounting. The ranges are byte offsets into
/// `bytes`, after the escape pass (and after parameter / history
/// substitution, for callers that thread the mask through those
/// passes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Prompt {
    pub(crate) bytes: Vec<u8>,
    pub(crate) invisible: Vec<(usize, usize)>,
}

impl Prompt {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Prompt {
            bytes,
            invisible: Vec::new(),
        }
    }

    /// True iff offset `i` lies inside any invisible range.
    #[cfg(test)]
    pub(crate) fn is_invisible(&self, i: usize) -> bool {
        self.invisible.iter().any(|(s, e)| i >= *s && i < *e)
    }
}

/// Pre-resolved inputs for the escape decoder.
///
/// Every field is filled by the caller before invoking the decoder,
/// typically by reading shell variables and calling a handful of
/// `sys::` helpers. The decoder never issues syscalls on its own.
///
/// All byte fields are owned rather than borrowed to simplify
/// construction by the REPL path, which mixes borrowed values from
/// `Shell` with freshly-computed ones from `sys::`.
#[derive(Clone, Debug, Default)]
pub(crate) struct PromptEnv {
    /// Login name — `$USER` if non-empty, else `getpwuid(geteuid())`.
    /// `None` means "both sources failed"; `\u` will emit `?`.
    pub(crate) user: Option<Vec<u8>>,
    /// Host name — `gethostname(2)` output. `None` means failure;
    /// `\h` / `\H` will emit `?`.
    pub(crate) hostname: Option<Vec<u8>>,
    /// Current working directory. `None` means `getcwd(3)` failed;
    /// `\w` / `\W` will emit `?`.
    pub(crate) cwd: Option<Vec<u8>>,
    /// Value of `$HOME` (may be unset / empty). Used to collapse
    /// `$HOME` prefixes of `cwd` into `~` for `\w`.
    pub(crate) home: Option<Vec<u8>>,
    /// Basename of the controlling tty for stdin. `None` means
    /// `ttyname(0)` failed; `\l` will emit `tty`.
    pub(crate) tty_basename: Option<Vec<u8>>,
    /// `true` iff `geteuid() == 0`. Controls `\$` output.
    pub(crate) euid_is_root: bool,
    /// Shell invocation basename (for `\s`).
    pub(crate) shell_name: Vec<u8>,
    /// Job count (for `\j`).
    pub(crate) jobs_count: usize,
    /// History number of the command about to be read (for `\!`).
    pub(crate) history_number: usize,
    /// Per-session counter of accepted input lines (for `\#`).
    pub(crate) session_counter: u64,
    /// Meiksh version string in the form `MAJOR.MINOR` (for `\v`).
    pub(crate) version_short: Vec<u8>,
    /// Meiksh version string in the form `MAJOR.MINOR.PATCH`
    /// (for `\V`).
    pub(crate) version_long: Vec<u8>,
}

/// Run the escape pass on `raw` using `env` and `tm` for the
/// corresponding escapes.
///
/// Returns a [`Prompt`] whose `bytes` may still contain `$...`
/// parameter references and literal `!` bytes — the caller is
/// responsible for running the parameter and history passes after
/// this function (see § 5 of the spec).
pub(crate) fn decode(raw: &[u8], env: &PromptEnv, tm: &LocalTime) -> Prompt {
    let mut out = Vec::with_capacity(raw.len());
    let mut invisible = Vec::<(usize, usize)>::new();
    let mut invisible_open: Option<usize> = None;

    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b != b'\\' {
            out.push(b);
            i += 1;
            continue;
        }
        // We have a backslash. Look at the next byte.
        if i + 1 >= raw.len() {
            // Trailing backslash — spec § 6.7.
            out.push(b'\\');
            i += 1;
            continue;
        }
        let c = raw[i + 1];

        // Special handling for `\[` / `\]` does not depend on whether
        // we are currently inside an invisible region (§ 8.1).
        if c == b'[' {
            if invisible_open.is_none() {
                invisible_open = Some(out.len());
                i += 2;
                continue;
            } else {
                // Nested `\[`: treat as literal per spec.
                out.push(b'\\');
                out.push(b'[');
                i += 2;
                continue;
            }
        }
        if c == b']' {
            if let Some(start) = invisible_open.take() {
                let end = out.len();
                if end > start {
                    invisible.push((start, end));
                }
            }
            // Otherwise silently drop (§ 8.1).
            i += 2;
            continue;
        }

        // Octal escape `\nnn`: leading octal digit.
        if (b'0'..=b'7').contains(&c) {
            let mut val: u16 = (c - b'0') as u16;
            let mut consumed = 2; // \ and one digit
            for extra in 0..2usize {
                let idx = i + 2 + extra;
                if idx >= raw.len() {
                    break;
                }
                let d = raw[idx];
                if !(b'0'..=b'7').contains(&d) {
                    break;
                }
                val = (val << 3) | ((d - b'0') as u16);
                consumed += 1;
            }
            out.push((val & 0xFF) as u8);
            i += consumed;
            continue;
        }

        // Recognized single-letter escapes.
        match c {
            b'a' => {
                out.push(0x07);
                i += 2;
            }
            b'e' => {
                out.push(0x1B);
                i += 2;
            }
            b'n' => {
                out.push(b'\n');
                i += 2;
            }
            b'r' => {
                out.push(b'\r');
                i += 2;
            }
            b'\\' => {
                out.push(b'\\');
                i += 2;
            }
            b'$' => {
                out.push(if env.euid_is_root { b'#' } else { b'$' });
                i += 2;
            }
            b's' => {
                push_basename(&mut out, &env.shell_name);
                i += 2;
            }
            b'v' => {
                out.extend_from_slice(&env.version_short);
                i += 2;
            }
            b'V' => {
                out.extend_from_slice(&env.version_long);
                i += 2;
            }
            b'j' => {
                bstr::push_u64(&mut out, env.jobs_count as u64);
                i += 2;
            }
            b'!' => {
                bstr::push_u64(&mut out, env.history_number as u64);
                i += 2;
            }
            b'#' => {
                bstr::push_u64(&mut out, env.session_counter);
                i += 2;
            }
            b'u' => {
                match env.user.as_deref() {
                    Some(u) if !u.is_empty() => out.extend_from_slice(u),
                    _ => out.push(b'?'),
                }
                i += 2;
            }
            b'h' => {
                match env.hostname.as_deref() {
                    Some(h) if !h.is_empty() => {
                        let cut = h.iter().position(|b| *b == b'.').unwrap_or(h.len());
                        out.extend_from_slice(&h[..cut]);
                    }
                    _ => out.push(b'?'),
                }
                i += 2;
            }
            b'H' => {
                match env.hostname.as_deref() {
                    Some(h) if !h.is_empty() => out.extend_from_slice(h),
                    _ => out.push(b'?'),
                }
                i += 2;
            }
            b'l' => {
                match env.tty_basename.as_deref() {
                    Some(t) if !t.is_empty() => out.extend_from_slice(t),
                    _ => out.extend_from_slice(b"tty"),
                }
                i += 2;
            }
            b'w' => {
                render_cwd(
                    &mut out,
                    env.cwd.as_deref(),
                    env.home.as_deref(),
                    CwdMode::Full,
                );
                i += 2;
            }
            b'W' => {
                render_cwd(
                    &mut out,
                    env.cwd.as_deref(),
                    env.home.as_deref(),
                    CwdMode::Basename,
                );
                i += 2;
            }
            b'd' => {
                out.extend_from_slice(&format_strftime(b"%a %b %e", tm, 64));
                i += 2;
            }
            b't' => {
                out.extend_from_slice(&format_strftime(b"%H:%M:%S", tm, 32));
                i += 2;
            }
            b'T' => {
                out.extend_from_slice(&format_strftime(b"%I:%M:%S", tm, 32));
                i += 2;
            }
            b'@' => {
                out.extend_from_slice(&format_strftime(b"%I:%M %p", tm, 32));
                i += 2;
            }
            b'A' => {
                out.extend_from_slice(&format_strftime(b"%H:%M", tm, 32));
                i += 2;
            }
            b'D' => {
                // \D{format}: requires a literal `{`.
                if i + 2 >= raw.len() || raw[i + 2] != b'{' {
                    // Unknown escape per § 6.6: emit two raw bytes.
                    out.push(b'\\');
                    out.push(b'D');
                    i += 2;
                    continue;
                }
                let start = i + 3;
                let mut end = start;
                while end < raw.len() && raw[end] != b'}' {
                    end += 1;
                }
                if end >= raw.len() {
                    // Unterminated \D{ — § 6.3: emit the bytes verbatim.
                    out.push(b'\\');
                    out.push(b'D');
                    out.push(b'{');
                    out.extend_from_slice(&raw[start..]);
                    i = raw.len();
                    continue;
                }
                let fmt = &raw[start..end];
                let fmt_effective: &[u8] = if fmt.is_empty() { b"%X" } else { fmt };
                out.extend_from_slice(&format_strftime(fmt_effective, tm, 256));
                i = end + 1;
            }
            _ => {
                // Unknown escape — § 6.6: emit two raw bytes.
                out.push(b'\\');
                out.push(c);
                i += 2;
            }
        }
    }

    // Unmatched `\[`: extend invisibility to end-of-output per § 8.2.
    if let Some(start) = invisible_open
        && out.len() > start
    {
        invisible.push((start, out.len()));
    }

    Prompt {
        bytes: out,
        invisible,
    }
}

#[derive(Clone, Copy)]
enum CwdMode {
    Full,
    Basename,
}

fn render_cwd(out: &mut Vec<u8>, cwd: Option<&[u8]>, home: Option<&[u8]>, mode: CwdMode) {
    let Some(cwd) = cwd else {
        out.push(b'?');
        return;
    };
    if cwd.is_empty() {
        out.push(b'?');
        return;
    }
    // § 6.2: CWD equal to $HOME collapses to ~ for both \w and \W.
    let home_matches = matches!(home, Some(h) if !h.is_empty() && h == cwd);
    if home_matches {
        out.push(b'~');
        return;
    }
    match mode {
        CwdMode::Full => {
            // If HOME is a prefix followed by '/', collapse to ~/<rest>.
            if let Some(h) = home
                && !h.is_empty()
                && cwd.len() > h.len()
                && cwd.starts_with(h)
                && cwd[h.len()] == b'/'
            {
                out.push(b'~');
                out.extend_from_slice(&cwd[h.len()..]);
            } else {
                out.extend_from_slice(cwd);
            }
        }
        CwdMode::Basename => {
            let slash = cwd.iter().rposition(|b| *b == b'/');
            match slash {
                Some(i) if i + 1 < cwd.len() => out.extend_from_slice(&cwd[i + 1..]),
                Some(_) => out.push(b'/'),
                None => out.extend_from_slice(cwd),
            }
        }
    }
}

fn push_basename(out: &mut Vec<u8>, s: &[u8]) {
    let slash = s.iter().rposition(|b| *b == b'/');
    match slash {
        Some(i) if i + 1 < s.len() => out.extend_from_slice(&s[i + 1..]),
        Some(_) => { /* trailing slash: empty basename */ }
        None => out.extend_from_slice(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_tm() -> LocalTime {
        // Mon 2024-01-15 13:45:30 — matches the default fixed time
        // returned by sys::time::local_time_now in the test build.
        LocalTime::from_fields(30, 45, 13, 15, 1, 2024, 1, 14)
    }

    fn base_env() -> PromptEnv {
        PromptEnv {
            user: Some(b"alice".to_vec()),
            hostname: Some(b"myhost.example.com".to_vec()),
            cwd: Some(b"/home/alice/work".to_vec()),
            home: Some(b"/home/alice".to_vec()),
            tty_basename: Some(b"pts/3".to_vec()),
            euid_is_root: false,
            shell_name: b"meiksh".to_vec(),
            jobs_count: 2,
            history_number: 42,
            session_counter: 7,
            version_short: b"0.1".to_vec(),
            version_long: b"0.1.0".to_vec(),
        }
    }

    fn decode_default(raw: &[u8]) -> Prompt {
        decode(raw, &base_env(), &fixed_tm())
    }

    #[test]
    fn plain_text_passes_through() {
        let p = decode_default(b"hello world");
        assert_eq!(p.bytes, b"hello world");
        assert!(p.invisible.is_empty());
    }

    #[test]
    fn c_escapes_render_control_bytes() {
        let p = decode_default(b"\\a\\e\\n\\r\\\\");
        assert_eq!(p.bytes, b"\x07\x1b\n\r\\");
    }

    #[test]
    fn user_hostname_and_full_hostname_escapes() {
        let p = decode_default(b"\\u@\\h / \\H");
        assert_eq!(&p.bytes[..], b"alice@myhost / myhost.example.com");
    }

    #[test]
    fn user_falls_back_to_question_mark_when_missing() {
        let mut env = base_env();
        env.user = None;
        let p = decode(b"\\u", &env, &fixed_tm());
        assert_eq!(p.bytes, b"?");
    }

    #[test]
    fn host_falls_back_to_question_mark_when_missing() {
        let mut env = base_env();
        env.hostname = None;
        let p = decode(b"\\h-\\H", &env, &fixed_tm());
        assert_eq!(p.bytes, b"?-?");
    }

    #[test]
    fn tty_basename_is_rendered_when_set() {
        // Covers the `Some(t) if !t.is_empty()` arm of the `\l`
        // handler that `tty_falls_back_to_tty_literal` can't touch.
        let p = decode(b"on \\l", &base_env(), &fixed_tm());
        assert_eq!(p.bytes, b"on pts/3");
    }

    #[test]
    fn d_escape_without_brace_falls_through_as_literal() {
        // `\D` not followed by `{` must fall through to the § 6.6
        // unknown-escape branch that emits the raw two bytes.
        let p = decode_default(b"pre \\Dxy");
        assert_eq!(p.bytes, b"pre \\Dxy");
    }

    #[test]
    fn octal_escape_at_end_of_input_stops_short() {
        // `\1` with no following digit is a two-byte octal escape; the
        // inner loop must `break` when `idx >= raw.len()`.
        let p = decode_default(b"\\1");
        assert_eq!(p.bytes, b"\x01");
    }

    #[test]
    fn empty_cwd_renders_as_question_mark() {
        // `cwd` is `Some`, but empty — must hit the
        // `if cwd.is_empty() { out.push(b'?') }` arm in `render_cwd`.
        let mut env = base_env();
        env.cwd = Some(Vec::new());
        let p = decode(b"\\w", &env, &fixed_tm());
        assert_eq!(p.bytes, b"?");
    }

    #[test]
    fn tty_falls_back_to_tty_literal() {
        let mut env = base_env();
        env.tty_basename = None;
        let p = decode(b"on \\l", &env, &fixed_tm());
        assert_eq!(p.bytes, b"on tty");
    }

    #[test]
    fn dollar_escape_reflects_euid() {
        let p1 = decode_default(b"\\$");
        assert_eq!(p1.bytes, b"$");
        let mut env = base_env();
        env.euid_is_root = true;
        let p2 = decode(b"\\$", &env, &fixed_tm());
        assert_eq!(p2.bytes, b"#");
    }

    #[test]
    fn w_and_big_w_collapse_home() {
        let p = decode_default(b"\\w | \\W");
        assert_eq!(p.bytes, b"~/work | work");

        // cwd == home
        let mut env = base_env();
        env.cwd = Some(b"/home/alice".to_vec());
        let p = decode(b"\\w | \\W", &env, &fixed_tm());
        assert_eq!(p.bytes, b"~ | ~");

        // cwd outside home
        let mut env = base_env();
        env.cwd = Some(b"/var/log".to_vec());
        let p = decode(b"\\w | \\W", &env, &fixed_tm());
        assert_eq!(p.bytes, b"/var/log | log");
    }

    #[test]
    fn cwd_missing_yields_question_mark() {
        let mut env = base_env();
        env.cwd = None;
        let p = decode(b"\\w / \\W", &env, &fixed_tm());
        assert_eq!(p.bytes, b"? / ?");
    }

    #[test]
    fn w_renders_basename_trailing_slash_case() {
        // Coverage for render_cwd's `Some(i) if i + 1 < cwd.len()`
        // vs `Some(_)` branches: CWD `"/"` has slash at index 0 and
        // length 1, so the Basename branch falls through to emit `/`.
        let mut env = base_env();
        env.cwd = Some(b"/".to_vec());
        env.home = Some(b"/home/alice".to_vec());
        let p = decode(b"\\W", &env, &fixed_tm());
        assert_eq!(p.bytes, b"/");
    }

    #[test]
    fn w_renders_no_slash_basename() {
        let mut env = base_env();
        env.cwd = Some(b"bare".to_vec());
        env.home = None;
        let p = decode(b"\\W", &env, &fixed_tm());
        assert_eq!(p.bytes, b"bare");
    }

    #[test]
    fn time_short_escapes_use_strftime() {
        let p = decode_default(b"\\t | \\T | \\A | \\@");
        // 13:45:30 for \t, 01:45:30 for \T, 13:45 for \A, 01:45 PM-ish
        // locale-dependent for \@. We assert only non-locale parts.
        let s = p.bytes;
        let parts: Vec<&[u8]> = s.split(|b| *b == b'|').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].trim_ascii(), b"13:45:30");
        assert_eq!(parts[1].trim_ascii(), b"01:45:30");
        assert_eq!(parts[2].trim_ascii(), b"13:45");
        assert!(parts[3].starts_with(b" 01:45"));
    }

    #[test]
    fn d_literal_format_renders() {
        let p = decode_default(b"\\D{%Y-%m-%d %H:%M}");
        assert_eq!(p.bytes, b"2024-01-15 13:45");
    }

    #[test]
    fn d_empty_format_defaults_to_locale_time() {
        let p = decode_default(b"[\\D{}]");
        // Just assert the brackets are preserved and something was
        // rendered inside — the exact output is locale-dependent.
        assert!(p.bytes.starts_with(b"["));
        assert!(p.bytes.ends_with(b"]"));
        assert!(p.bytes.len() > 2);
    }

    #[test]
    fn d_unterminated_emits_verbatim() {
        let p = decode_default(b"x\\D{%Y");
        assert_eq!(p.bytes, b"x\\D{%Y");
    }

    #[test]
    fn octal_escapes_up_to_three_digits() {
        let p = decode_default(b"\\101\\0\\7a\\377");
        assert_eq!(p.bytes, b"A\x00\x07a\xff");
    }

    #[test]
    fn octal_stops_at_non_octal_digit() {
        let p = decode_default(b"\\18");
        assert_eq!(p.bytes, b"\x018");
    }

    #[test]
    fn trailing_backslash_is_literal() {
        let p = decode_default(b"abc\\");
        assert_eq!(p.bytes, b"abc\\");
    }

    #[test]
    fn unknown_escape_is_emitted_as_two_bytes() {
        let p = decode_default(b"x\\q y");
        assert_eq!(p.bytes, b"x\\q y");
    }

    #[test]
    fn job_count_and_counter() {
        let p = decode_default(b"[\\j] (\\#) !\\!");
        assert_eq!(p.bytes, b"[2] (7) !42");
    }

    #[test]
    fn version_escapes() {
        let p = decode_default(b"\\s-\\v | \\V");
        assert_eq!(p.bytes, b"meiksh-0.1 | 0.1.0");
    }

    #[test]
    fn shell_name_is_basename_of_argv0() {
        let mut env = base_env();
        env.shell_name = b"/usr/local/bin/meiksh".to_vec();
        let p = decode(b"\\s", &env, &fixed_tm());
        assert_eq!(p.bytes, b"meiksh");
    }

    #[test]
    fn invisible_region_records_byte_range() {
        let p = decode_default(b"\\[\x1b[31m\\]red\\[\x1b[0m\\]");
        assert_eq!(p.bytes, b"\x1b[31mred\x1b[0m");
        assert_eq!(p.invisible.len(), 2);
        assert_eq!(p.invisible[0], (0, 5));
        assert_eq!(p.invisible[1], (8, 12));
        assert!(p.is_invisible(0));
        assert!(p.is_invisible(4));
        assert!(!p.is_invisible(5));
        assert!(!p.is_invisible(7));
        assert!(p.is_invisible(8));
    }

    #[test]
    fn nested_open_bracket_is_literal() {
        // \[ ... \[ ... \] should treat the inner \[ as literal bytes.
        let p = decode_default(b"\\[A\\[B\\]C");
        // Outer region opens at offset 0; inner \[ emits `\[` (literal
        // two bytes) continuing the open region; \] closes it.
        assert_eq!(p.bytes, b"A\\[BC");
        assert_eq!(p.invisible, vec![(0, 4)]);
    }

    #[test]
    fn unmatched_close_bracket_is_dropped() {
        let p = decode_default(b"a\\]b");
        assert_eq!(p.bytes, b"ab");
        assert!(p.invisible.is_empty());
    }

    #[test]
    fn unmatched_open_bracket_extends_to_end() {
        let p = decode_default(b"a\\[bc");
        assert_eq!(p.bytes, b"abc");
        assert_eq!(p.invisible, vec![(1, 3)]);
    }

    #[test]
    fn empty_invisible_region_produces_no_range() {
        let p = decode_default(b"\\[\\]x");
        assert_eq!(p.bytes, b"x");
        assert!(p.invisible.is_empty());
    }

    // === Spec § 6.5 (Octal) additional coverage ===================

    /// § 6.5: a fourth octal digit shall begin a fresh run of literal
    /// bytes and shall not extend the octal escape.
    #[test]
    fn fourth_octal_digit_starts_fresh_literal_run() {
        // `\1011` — `\101` decodes to 'A', the trailing '1' is a
        // literal byte.
        let p = decode_default(b"\\1011");
        assert_eq!(p.bytes, b"A1");
    }

    /// § 6.5: an octal value greater than 0o377 shall keep the low 8
    /// bits. `\777` has value 511 = 0x1FF; the emitted byte is 0xFF.
    #[test]
    fn octal_value_above_0o377_keeps_low_eight_bits() {
        let p = decode_default(b"\\777");
        assert_eq!(p.bytes, b"\xff");
    }

    /// § 6.5: `\0` alone emits the NUL byte.
    #[test]
    fn octal_escape_nul_byte() {
        let p = decode_default(b"a\\0b");
        assert_eq!(p.bytes, b"a\x00b");
    }

    // === Spec § 6.2 (CWD) additional coverage =====================

    /// § 6.2: if `$HOME` is unset, `\w` emits the absolute path
    /// unchanged.
    #[test]
    fn w_without_home_emits_absolute_path() {
        let mut env = base_env();
        env.home = None;
        env.cwd = Some(b"/var/lib/spool".to_vec());
        let p = decode(b"\\w", &env, &fixed_tm());
        assert_eq!(p.bytes, b"/var/lib/spool");
    }

    /// § 6.2: if `$HOME` is empty, `\w` emits the absolute path
    /// unchanged (empty is treated the same as unset).
    #[test]
    fn w_with_empty_home_emits_absolute_path() {
        let mut env = base_env();
        env.home = Some(Vec::new());
        env.cwd = Some(b"/var/log".to_vec());
        let p = decode(b"\\w", &env, &fixed_tm());
        assert_eq!(p.bytes, b"/var/log");
    }

    /// § 6.2: `$HOME` must be a proper prefix *followed by `/`* for
    /// `\w` to collapse. If CWD merely happens to start with the
    /// bytes of `$HOME` but is not a `/`-delimited extension, the
    /// absolute path is emitted verbatim (e.g. HOME=/foo, CWD=/foobar).
    #[test]
    fn w_does_not_collapse_non_proper_home_prefix() {
        let mut env = base_env();
        env.home = Some(b"/home/alic".to_vec());
        env.cwd = Some(b"/home/alice/work".to_vec());
        let p = decode(b"\\w", &env, &fixed_tm());
        assert_eq!(p.bytes, b"/home/alice/work");
    }

    // === Spec § 6.3 (Time) additional coverage ====================

    /// § 6.3: `\D{format}` output buffer is capped at 256 bytes; a
    /// format that would produce more than that shall be truncated
    /// silently (strftime returns 0 → empty output per the graceful-
    /// degradation contract in § 10.2).
    #[test]
    fn d_format_output_capped_at_256_bytes() {
        // A repeated literal filler of 300 bytes will overflow the
        // 256-byte buffer; strftime returns 0 and the decoder emits
        // nothing for that escape. Surrounding literal text proves
        // the decoder continued rather than aborted.
        let filler = b"A".repeat(300);
        let mut raw = Vec::from(&b"[\\D{"[..]);
        raw.extend_from_slice(&filler);
        raw.extend_from_slice(b"}]");
        let p = decode(&raw, &base_env(), &fixed_tm());
        assert_eq!(p.bytes, b"[]");
    }

    // === Spec § 6.4 (Session counter) =============================

    /// § 6.4: `\#` shall emit a decimal integer; it starts at 1 on
    /// shell startup and is decoupled from `\!`.
    #[test]
    fn session_counter_starts_at_one() {
        let mut env = base_env();
        env.session_counter = 1;
        env.history_number = 12;
        let p = decode(b"(\\#)|(\\!)", &env, &fixed_tm());
        assert_eq!(p.bytes, b"(1)|(12)");
    }

    // === Spec § 6.1 (`\v` / `\V`) =================================

    /// § 6.1: `\v` is `MAJOR.MINOR`; `\V` is `MAJOR.MINOR.PATCH`. Both
    /// are independent of the shell name.
    #[test]
    fn version_escapes_independent_of_shell_name() {
        let mut env = base_env();
        env.version_short = b"9.42".to_vec();
        env.version_long = b"9.42.7".to_vec();
        env.shell_name = b"other".to_vec();
        let p = decode(b"\\v|\\V", &env, &fixed_tm());
        assert_eq!(p.bytes, b"9.42|9.42.7");
    }

    // === Spec § 6.1 (`\s` basename) ===============================

    /// § 6.1: `\s` is the basename of `$0`. A trailing-slash path
    /// (edge case) must not panic and yields the empty string.
    #[test]
    fn shell_name_with_trailing_slash_emits_empty() {
        let mut env = base_env();
        env.shell_name = b"/tmp/".to_vec();
        let p = decode(b"<\\s>", &env, &fixed_tm());
        assert_eq!(p.bytes, b"<>");
    }

    // === Spec § 6.1 (`\u` user priority) ==========================

    /// § 6.1: `\u` shall prefer `$USER` over `getpwuid(geteuid())`.
    /// The `PromptEnv` carries a single already-resolved `user` so
    /// we verify the emit path, and show that an empty user value
    /// falls back to `?`.
    #[test]
    fn user_empty_falls_back_to_question_mark() {
        let mut env = base_env();
        env.user = Some(Vec::new());
        let p = decode(b"\\u", &env, &fixed_tm());
        assert_eq!(p.bytes, b"?");
    }

    // === Spec § 6.1 (`\h` / `\H`) =================================

    /// § 6.1: `\h` is the host up to but not including the first `.`.
    /// A hostname with no dot must be emitted in full for `\h`.
    #[test]
    fn short_host_with_no_dot_is_emitted_in_full() {
        let mut env = base_env();
        env.hostname = Some(b"localhost".to_vec());
        let p = decode(b"\\h", &env, &fixed_tm());
        assert_eq!(p.bytes, b"localhost");
    }

    /// § 6.1: an empty hostname (distinguished from `None`) still
    /// falls back to `?` per the graceful-degradation contract.
    #[test]
    fn empty_hostname_renders_question_mark() {
        let mut env = base_env();
        env.hostname = Some(Vec::new());
        let p = decode(b"\\h|\\H", &env, &fixed_tm());
        assert_eq!(p.bytes, b"?|?");
    }

    // === Spec § 8.3 (mask threading) ==============================

    /// § 8.3: the mask refers to byte offsets in the decoder output.
    /// This test ensures that two non-adjacent regions produce two
    /// non-overlapping sorted ranges (monotonic byte offsets) with
    /// the visible text in between reported as *not* invisible.
    #[test]
    fn two_invisible_regions_produce_non_overlapping_sorted_ranges() {
        let p = decode_default(b"\\[AA\\]V\\[BB\\]");
        assert_eq!(p.bytes, b"AAVBB");
        assert_eq!(p.invisible, vec![(0, 2), (3, 5)]);
        // `V` is visible.
        assert!(!p.is_invisible(2));
    }

    // === Spec § 6.1 (`\j` zero jobs) ==============================

    /// § 6.1: `\j` emits the decimal job count; zero is a valid value.
    #[test]
    fn job_count_zero_emits_literal_zero() {
        let mut env = base_env();
        env.jobs_count = 0;
        let p = decode(b"[\\j]", &env, &fixed_tm());
        assert_eq!(p.bytes, b"[0]");
    }

    // === Spec § 6.3 (Time shorthand equivalences) =================

    /// § 6.3 shorthand table: `\d` is `\D{%a %b %e}`. At our fixed
    /// time 2024-01-15 (Monday) the output starts with "Mon Jan" in
    /// the C locale. We allow locale-specific day/month names by
    /// only asserting that the day-of-month digit appears.
    #[test]
    fn backslash_d_renders_date_shorthand() {
        let p = decode_default(b"\\d");
        let s = p.bytes;
        assert!(!s.is_empty());
        // Day-of-month 15 must appear somewhere in the output.
        assert!(s.windows(2).any(|w| w == b"15"), "got: {:?}", s);
    }
}
