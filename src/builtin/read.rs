use super::{BuiltinOutcome, diag_status, var_error_msg};
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

#[derive(Clone)]
pub(super) struct ReadOptions {
    pub(super) raw: bool,
    pub(super) delimiter: u8,
    /// Optional `-p prompt` value. When `Some`, the literal bytes
    /// are written to `stderr` exactly as supplied before any input
    /// is read. Per `docs/features/ps1-prompt-extensions.md` § 12.3
    /// this prompt is NOT subject to the backslash-escape pass that
    /// `PS1` / `PS4` go through — it is a straight POSIX pass-through.
    pub(super) prompt: Option<Vec<u8>>,
}

pub(super) fn read(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    sys::fd_io::ensure_blocking_read_fd(sys::constants::STDIN_FILENO)
        .map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    read_with_input(shell, argv, sys::constants::STDIN_FILENO)
}

pub(super) fn read_with_input(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    input_fd: i32,
) -> Result<BuiltinOutcome, ShellError> {
    let Some((options, vars)) = parse_read_options(argv) else {
        return Ok(diag_status(shell, 2, b"read: invalid usage"));
    };
    let vars = if vars.is_empty() {
        vec![b"REPLY".to_vec()]
    } else {
        vars
    };

    // Per ps1-prompt-extensions.md § 12.3, `read -p <prompt>` writes
    // the prompt bytes verbatim to stderr before any input is read.
    // The escape pass is intentionally skipped: users who want
    // variable expansion in the prompt can embed `$(...)` via the
    // caller's own quoting. The write is best-effort — a failure
    // here does not abort the read, matching bash.
    if let Some(prompt) = options.prompt.as_deref() {
        let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt);
    }

    let (pieces, hit_delimiter) = match read_logical_line(shell, options.clone(), input_fd) {
        Ok(result) => result,
        Err(error) => {
            let msg = ByteWriter::new()
                .bytes(b"read: ")
                .bytes(&error.strerror())
                .finish();
            return Ok(diag_status(shell, 2, &msg));
        }
    };
    let values = split_read_assignments(&pieces, &vars, shell.get_var(b"IFS").map(|s| s.to_vec()));
    for (name, value) in vars.iter().zip(values) {
        if let Err(error) = shell.set_var(name, &value) {
            let msg = var_error_msg(b"read", &error);
            return Ok(diag_status(shell, 2, &msg));
        }
    }
    Ok(BuiltinOutcome::Status(if hit_delimiter { 0 } else { 1 }))
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            raw: false,
            delimiter: b'\n',
            prompt: None,
        }
    }
}

pub(super) fn parse_read_options(argv: &[Vec<u8>]) -> Option<(ReadOptions, Vec<Vec<u8>>)> {
    let mut options = ReadOptions::default();
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"--" => {
                index += 1;
                break;
            }
            b"-r" => {
                options.raw = true;
                index += 1;
            }
            b"-d" => {
                let delim = argv.get(index + 1)?;
                options.delimiter = if delim.is_empty() {
                    0
                } else if delim.len() == 1 {
                    delim[0]
                } else {
                    return None;
                };
                index += 2;
            }
            b"-p" => {
                // `-p <prompt>` — two-argument form. A missing
                // operand is a usage error. The prompt may be empty.
                let prompt = argv.get(index + 1)?;
                options.prompt = Some(prompt.clone());
                index += 2;
            }
            // Joined short-option form like `-pfoo`: consume the
            // remainder of the token as the prompt value.
            _ if arg.len() > 2 && arg.starts_with(b"-p") => {
                options.prompt = Some(arg[2..].to_vec());
                index += 1;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => return None,
            _ => break,
        }
    }
    Some((options, argv[index..].to_vec()))
}

/// Size of the per-invocation read buffer used when the input fd is
/// seekable. Sized to fit a handful of typical log / CSV lines per
/// syscall without pushing stack use above a page.
const READ_CHUNK: usize = 1024;

/// Stateful byte-source used by `read_logical_line`. On seekable fds
/// the cursor buffers up to [`READ_CHUNK`] bytes per `read()` syscall
/// and rewinds any unconsumed bytes in `Drop`, so callers that share
/// the fd with subsequent commands still observe the byte-at-a-time fd
/// position they would get from the unbuffered path. On non-seekable
/// fds (pipes, FIFOs, sockets, terminals) we cannot over-read without
/// losing data, so the cursor reads one byte per syscall.
enum ReadCursor {
    Buffered {
        buf: [u8; READ_CHUNK],
        start: usize,
        end: usize,
        fd: i32,
    },
    Unbuffered,
}

impl ReadCursor {
    fn for_fd(fd: i32) -> Self {
        match sys::fd_io::fd_seek_cur(fd) {
            Ok(_) => Self::Buffered {
                buf: [0u8; READ_CHUNK],
                start: 0,
                end: 0,
                fd,
            },
            Err(_) => Self::Unbuffered,
        }
    }

    fn next_byte(&mut self, input_fd: i32) -> sys::error::SysResult<Option<u8>> {
        match self {
            Self::Buffered {
                buf, start, end, ..
            } => {
                if *start == *end {
                    let n = sys::fd_io::read_fd(input_fd, &mut buf[..])?;
                    if n == 0 {
                        return Ok(None);
                    }
                    *start = 0;
                    *end = n;
                }
                let b = buf[*start];
                *start += 1;
                Ok(Some(b))
            }
            Self::Unbuffered => {
                let mut one = [0u8; 1];
                let n = sys::fd_io::read_fd(input_fd, &mut one)?;
                if n == 0 { Ok(None) } else { Ok(Some(one[0])) }
            }
        }
    }
}

impl Drop for ReadCursor {
    fn drop(&mut self) {
        if let Self::Buffered { start, end, fd, .. } = self {
            let unused = end.saturating_sub(*start);
            if unused > 0 {
                // Best-effort rewind: if the seek fails the caller
                // will surface the real error on the next read from
                // the same fd.
                let _ = sys::fd_io::fd_seek_rewind(*fd, unused);
            }
        }
    }
}

pub(super) fn read_logical_line(
    shell: &Shell,
    options: ReadOptions,
    input_fd: i32,
) -> sys::error::SysResult<(Vec<(Vec<u8>, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = Vec::new();
    let mut current_quoted = false;
    let mut cursor = ReadCursor::for_fd(input_fd);

    loop {
        let Some(ch) = cursor.next_byte(input_fd)? else {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, false));
        };
        if !options.raw && ch == b'\\' {
            let Some(escaped) = cursor.next_byte(input_fd)? else {
                current.push(b'\\');
                push_read_piece(&mut pieces, &mut current, current_quoted);
                return Ok((pieces, false));
            };
            if escaped == b'\n' || escaped == options.delimiter {
                push_read_piece(&mut pieces, &mut current, current_quoted);
                current_quoted = false;
                if shell.is_interactive() {
                    let prompt = shell.get_var(b"PS2").unwrap_or(b"> ");
                    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt);
                }
                continue;
            }
            push_read_piece(&mut pieces, &mut current, current_quoted);
            current_quoted = true;
            current.push(escaped);
            continue;
        }
        if ch == options.delimiter {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, true));
        }
        if current_quoted {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            current_quoted = false;
        }
        current.push(ch);
    }
}

pub(super) fn push_read_piece(
    pieces: &mut Vec<(Vec<u8>, bool)>,
    current: &mut Vec<u8>,
    quoted: bool,
) {
    if current.is_empty() {
        return;
    }
    if let Some((last, last_quoted)) = pieces.last_mut() {
        if *last_quoted == quoted {
            last.extend_from_slice(current);
            current.clear();
            return;
        }
    }
    pieces.push((std::mem::take(current), quoted));
}

struct ReadIfsChar {
    byte_seq: Box<[u8]>,
    is_ws: bool,
}

fn decompose_read_ifs(ifs: &[u8]) -> Vec<ReadIfsChar> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < ifs.len() {
        let (_, len) = crate::sys::locale::decode_char(&ifs[i..]);
        let step = if len == 0 { 1 } else { len };
        let is_ws = step == 1 && matches!(ifs[i], b' ' | b'\t' | b'\n');
        result.push(ReadIfsChar {
            byte_seq: ifs[i..i + step].into(),
            is_ws,
        });
        i += step;
    }
    result
}

fn find_read_ifs_at<'a>(ifs_chars: &'a [ReadIfsChar], bytes: &[u8]) -> Option<(&'a [u8], bool)> {
    for ic in ifs_chars {
        if bytes.len() >= ic.byte_seq.len() && bytes[..ic.byte_seq.len()] == *ic.byte_seq {
            return Some((&ic.byte_seq, ic.is_ws));
        }
    }
    None
}

pub(super) fn split_read_assignments(
    pieces: &[(Vec<u8>, bool)],
    vars: &[Vec<u8>],
    ifs_value: Option<Vec<u8>>,
) -> Vec<Vec<u8>> {
    if vars.is_empty() {
        return Vec::new();
    }
    let ifs = ifs_value.unwrap_or_else(|| b" \t\n".to_vec());
    if ifs.is_empty() {
        let mut values = vec![flatten_read_pieces(pieces)];
        values.resize(vars.len(), Vec::new());
        return values;
    }

    let ifs_chars = decompose_read_ifs(&ifs);
    let bytes = flatten_read_bytes(pieces);
    if vars.len() == 1 {
        return vec![trim_read_ifs_ws(&bytes, &ifs_chars)];
    }

    let mut values = Vec::new();
    let mut index = 0usize;
    skip_read_ifs_ws(&bytes, &ifs_chars, &mut index);
    while index < bytes.len() && values.len() + 1 < vars.len() {
        let mut current = Vec::new();
        loop {
            if index >= bytes.len() {
                values.push(current);
                break;
            }
            let (_, quoted) = bytes[index];
            if !quoted {
                if let Some((seq, is_ws)) =
                    find_read_ifs_at(&ifs_chars, &unquoted_tail(&bytes, index))
                {
                    if is_ws {
                        debug_assert!(
                            !current.is_empty(),
                            "leading IFS whitespace should already be skipped"
                        );
                    }
                    values.push(current);
                    index += seq.len();
                    skip_read_ifs_ws(&bytes, &ifs_chars, &mut index);
                    break;
                }
            }
            current.push(bytes[index].0);
            index += 1;
        }
    }

    values.push(trim_read_ifs_ws(&bytes[index..], &ifs_chars));
    values.resize(vars.len(), Vec::new());
    values
}

pub(super) fn flatten_read_pieces(pieces: &[(Vec<u8>, bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (part, _) in pieces {
        out.extend_from_slice(part);
    }
    out
}

fn flatten_read_bytes(pieces: &[(Vec<u8>, bool)]) -> Vec<(u8, bool)> {
    let mut out = Vec::new();
    for (text, quoted) in pieces {
        for &b in text.iter() {
            out.push((b, *quoted));
        }
    }
    out
}

fn unquoted_tail(bytes: &[(u8, bool)], start: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for &(b, q) in &bytes[start..] {
        if q {
            break;
        }
        out.push(b);
    }
    out
}

fn skip_read_ifs_ws(bytes: &[(u8, bool)], ifs_chars: &[ReadIfsChar], index: &mut usize) {
    while *index < bytes.len() && !bytes[*index].1 {
        let tail = unquoted_tail(bytes, *index);
        if let Some((seq, true)) = find_read_ifs_at(ifs_chars, &tail) {
            *index += seq.len();
        } else {
            break;
        }
    }
}

fn trim_read_ifs_ws(bytes: &[(u8, bool)], ifs_chars: &[ReadIfsChar]) -> Vec<u8> {
    let mut start = 0usize;
    skip_read_ifs_ws(bytes, ifs_chars, &mut start);
    let mut end = bytes.len();
    loop {
        let mut found = false;
        for ic in ifs_chars {
            if !ic.is_ws {
                continue;
            }
            let slen = ic.byte_seq.len();
            if end >= start + slen {
                let candidate = end - slen;
                if bytes[candidate..end]
                    .iter()
                    .zip(ic.byte_seq.iter())
                    .all(|(&(b, q), &ib)| b == ib && !q)
                {
                    end = candidate;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }
    bytes[start..end].iter().map(|&(b, _)| b).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, test_shell};
    use crate::sys::test_support::{
        ArgMatcher, TraceResult, assert_no_syscalls, run_trace, set_test_locale_c,
        set_test_locale_utf8, t,
    };
    use crate::trace_entries;

    /// Prepends the "probe lseek fails with ESPIPE" entry every trace
    /// needs for `read_logical_line` to drop into the unbuffered path.
    /// Returning ESPIPE mirrors what the kernel does for
    /// pipes / FIFOs / sockets / terminals, which is what every unit
    /// test in this module is modelling.
    fn unseekable_probe(fd: i32) -> crate::sys::test_support::TraceEntry {
        t(
            "lseek",
            vec![
                ArgMatcher::Fd(fd),
                ArgMatcher::Int(0),
                ArgMatcher::Int(sys::constants::SEEK_CUR as i64),
            ],
            TraceResult::Err(sys::constants::ESPIPE),
        )
    }

    /// Byte-at-a-time reads without a leading seekability probe —
    /// for mid-call continuations that happen inside a single
    /// `read_logical_line` invocation and therefore share the same
    /// probe as the surrounding trace.
    fn byte_reads_continuation(fd: i32, input: &[u8]) -> Vec<crate::sys::test_support::TraceEntry> {
        input
            .iter()
            .map(|&b| {
                t(
                    "read",
                    vec![ArgMatcher::Fd(fd), ArgMatcher::Any],
                    TraceResult::Bytes(vec![b]),
                )
            })
            .collect()
    }

    fn byte_reads(fd: i32, input: &[u8]) -> Vec<crate::sys::test_support::TraceEntry> {
        let mut out = vec![unseekable_probe(fd)];
        out.extend(byte_reads_continuation(fd, input));
        out
    }

    fn byte_reads_then_eof(fd: i32, input: &[u8]) -> Vec<crate::sys::test_support::TraceEntry> {
        let mut out = byte_reads(fd, input);
        out.push(t(
            "read",
            vec![ArgMatcher::Fd(fd), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        out
    }

    #[test]
    fn read_options_and_assignments_parsing() {
        assert_no_syscalls(|| {
            let (options, vars) = parse_read_options(&[
                b"read".to_vec(),
                b"-r".to_vec(),
                b"-d".to_vec(),
                b",".to_vec(),
                b"A".to_vec(),
                b"B".to_vec(),
            ])
            .expect("read options");
            assert!(options.raw);
            assert_eq!(options.delimiter, b',');
            assert_eq!(vars, vec![b"A".to_vec(), b"B".to_vec()]);
            assert_eq!(
                parse_read_options(&[
                    b"read".to_vec(),
                    b"-d".to_vec(),
                    b"".to_vec(),
                    b"NUL".to_vec()
                ])
                .expect("read nul delim")
                .0
                .delimiter,
                0
            );
            assert_eq!(
                parse_read_options(&[b"read".to_vec(), b"--".to_vec(), b"NAME".to_vec()])
                    .expect("read dash dash")
                    .1,
                vec![b"NAME".to_vec()]
            );

            assert_eq!(
                split_read_assignments(
                    &[(b"alpha beta gamma".to_vec(), false)],
                    &[b"FIRST".to_vec(), b"SECOND".to_vec()],
                    None,
                ),
                vec![b"alpha".to_vec(), b"beta gamma".to_vec()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"  alpha beta  ".to_vec(), false)],
                    &[b"ONLY".to_vec()],
                    None,
                ),
                vec![b"alpha beta".to_vec()]
            );
            assert_eq!(
                split_read_assignments(&[], &[], None),
                Vec::<Vec<u8>>::new()
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha beta".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    Some(Vec::new()),
                ),
                vec![b"alpha beta".to_vec(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b" \t ".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    None,
                ),
                vec![Vec::new(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"left,right".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec()],
                    Some(b",".to_vec()),
                ),
                vec![b"left".to_vec(), b"right".to_vec()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec(), b"THREE".to_vec()],
                    None,
                ),
                vec![b"alpha".to_vec(), Vec::new(), Vec::new()]
            );
            assert_eq!(
                split_read_assignments(
                    &[(b"alpha,   ".to_vec(), false)],
                    &[b"ONE".to_vec(), b"TWO".to_vec(), b"THREE".to_vec()],
                    Some(b", ".to_vec()),
                ),
                vec![b"alpha".to_vec(), Vec::new(), Vec::new()]
            );

            let mut pieces = Vec::new();
            let mut empty = Vec::new();
            push_read_piece(&mut pieces, &mut empty, false);
            assert!(pieces.is_empty());
        });
    }

    #[test]
    fn parse_read_options_invalid_returns_none() {
        assert_no_syscalls(|| {
            assert!(parse_read_options(&[b"read".to_vec(), b"-x".to_vec()]).is_none());
        });
    }

    /// Spec: `docs/features/ps1-prompt-extensions.md` § 12.3 — the
    /// `-p prompt` option is parsed in both the separated-operand
    /// form (`-p PROMPT`) and the joined short-form (`-pPROMPT`).
    /// The prompt bytes are stored verbatim, and a missing operand
    /// in the separated form is a usage error.
    #[test]
    fn parse_read_options_dash_p_separated_and_joined_forms() {
        assert_no_syscalls(|| {
            // Separated form.
            let (opts, vars) = parse_read_options(&[
                b"read".to_vec(),
                b"-p".to_vec(),
                b"prompt:> ".to_vec(),
                b"VAR".to_vec(),
            ])
            .expect("parse -p separated");
            assert_eq!(opts.prompt.as_deref(), Some(b"prompt:> ".as_slice()));
            assert_eq!(vars, vec![b"VAR".to_vec()]);

            // Joined form.
            let (opts, vars) =
                parse_read_options(&[b"read".to_vec(), b"-phello> ".to_vec(), b"DEST".to_vec()])
                    .expect("parse -p joined");
            assert_eq!(opts.prompt.as_deref(), Some(b"hello> ".as_slice()));
            assert_eq!(vars, vec![b"DEST".to_vec()]);

            // Empty prompt in separated form is legal (matches bash).
            let (opts, _) = parse_read_options(&[
                b"read".to_vec(),
                b"-p".to_vec(),
                b"".to_vec(),
                b"X".to_vec(),
            ])
            .expect("parse -p empty");
            assert_eq!(opts.prompt.as_deref(), Some(b"".as_slice()));

            // Missing operand in separated form → usage error (None).
            assert!(parse_read_options(&[b"read".to_vec(), b"-p".to_vec()]).is_none());
        });
    }

    /// § 12.3: `read -p <prompt>` writes the prompt bytes verbatim
    /// to stderr before consuming any input. Verify via a faked
    /// read trace that the write happens exactly once, with the
    /// exact bytes supplied.
    #[test]
    fn read_with_input_dash_p_writes_prompt_to_stderr_before_read() {
        let reads = byte_reads(42, b"x\n");
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(b"<type>")) -> auto,
                ..reads,
            ],
            || {
                let mut shell = test_shell();
                let result = read_with_input(
                    &mut shell,
                    &[
                        b"read".to_vec(),
                        b"-p".to_vec(),
                        b"<type>".to_vec(),
                        b"VAR".to_vec(),
                    ],
                    42,
                );
                assert!(matches!(result, Ok(BuiltinOutcome::Status(0))));
                assert_eq!(shell.get_var(b"VAR"), Some(b"x".as_slice()));
            },
        );
    }

    #[test]
    fn parse_read_options_delimiter_multi_byte_none() {
        assert_no_syscalls(|| {
            assert!(
                parse_read_options(&[b"read".to_vec(), b"-d".to_vec(), b"ab".to_vec()]).is_none()
            );
        });
    }

    #[test]
    fn push_read_piece_merges_same_quoted() {
        assert_no_syscalls(|| {
            let mut pieces = vec![(b"hello".to_vec(), false)];
            let mut current = b" world".to_vec();
            push_read_piece(&mut pieces, &mut current, false);
            assert_eq!(pieces.len(), 1);
            assert_eq!(pieces[0].0, b"hello world");
        });
    }

    #[test]
    fn push_read_piece_different_quotedness_creates_new_entry() {
        assert_no_syscalls(|| {
            let mut pieces = vec![(b"unquoted".to_vec(), false)];
            let mut current = b"quoted".to_vec();
            push_read_piece(&mut pieces, &mut current, true);
            assert_eq!(pieces.len(), 2);
            assert_eq!(pieces[0], (b"unquoted".to_vec(), false));
            assert_eq!(pieces[1], (b"quoted".to_vec(), true));
        });
    }

    #[test]
    fn read_with_input_invalid_option_returns_diag() {
        let msg = diag(b"read: invalid usage");
        run_trace(
            trace_entries![
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let result = read_with_input(&mut shell, &[b"read".to_vec(), b"-z".to_vec()], 42);
                assert!(matches!(result, Ok(BuiltinOutcome::Status(2))));
            },
        );
    }

    #[test]
    fn read_with_input_read_error_returns_diag() {
        let eio_str = crate::sys::error::SysError::Errno(sys::constants::EIO).strerror();
        let mut diag_body = b"read: ".to_vec();
        diag_body.extend_from_slice(&eio_str);
        let msg = diag(&diag_body);
        let reads = vec![
            unseekable_probe(42),
            t(
                "read",
                vec![ArgMatcher::Fd(42), ArgMatcher::Any],
                TraceResult::Err(sys::constants::EIO),
            ),
        ];
        run_trace(
            trace_entries![
                ..reads,
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let result = read_with_input(&mut shell, &[b"read".to_vec(), b"VAR".to_vec()], 42);
                assert!(matches!(result, Ok(BuiltinOutcome::Status(2))));
            },
        );
    }

    #[test]
    fn read_with_input_readonly_var_returns_diag() {
        let msg = diag(b"read: readonly variable: DEST");
        let reads = byte_reads(42, b"hello\n");
        run_trace(
            trace_entries![
                ..reads,
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.mark_readonly(b"DEST");
                let result = read_with_input(&mut shell, &[b"read".to_vec(), b"DEST".to_vec()], 42);
                assert!(matches!(result, Ok(BuiltinOutcome::Status(2))));
            },
        );
    }

    #[test]
    fn read_logical_line_backslash_at_eof() {
        let reads = byte_reads_then_eof(42, b"trail\\");
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(!hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"trail\\");
        });
    }

    #[test]
    fn read_logical_line_backslash_newline_continues() {
        let reads = byte_reads(42, b"first\\\nsecond\n");
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"firstsecond");
        });
    }

    #[test]
    fn read_logical_line_backslash_other_quotes_char() {
        let reads = byte_reads(42, b"a\\bc\n");
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"abc");
            let has_quoted = pieces.iter().any(|(_, q)| *q);
            assert!(has_quoted, "backslash-escaped char should be quoted");
        });
    }

    #[test]
    fn read_logical_line_backslash_eof_after_escape() {
        let mut reads = byte_reads(42, b"x\\");
        reads.push(t(
            "read",
            vec![ArgMatcher::Fd(42), ArgMatcher::Any],
            TraceResult::Bytes(vec![b'\\']),
        ));
        reads.push(t(
            "read",
            vec![ArgMatcher::Fd(42), ArgMatcher::Any],
            TraceResult::Int(0),
        ));
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(!hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"x\\");
        });
    }

    #[test]
    fn read_logical_line_raw_mode_preserves_backslash() {
        let reads = byte_reads(42, b"a\\b\n");
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: true,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"a\\b");
        });
    }

    #[test]
    fn read_with_input_default_reply_variable() {
        let reads = byte_reads(42, b"hello\n");
        run_trace(trace_entries![..reads], || {
            let mut shell = test_shell();
            let result = read_with_input(&mut shell, &[b"read".to_vec()], 42);
            assert!(matches!(result, Ok(BuiltinOutcome::Status(0))));
            assert_eq!(shell.get_var(b"REPLY"), Some(b"hello".as_slice()));
        });
    }

    #[test]
    fn read_with_input_eof_returns_status_1() {
        let reads = byte_reads_then_eof(42, b"partial");
        run_trace(trace_entries![..reads], || {
            let mut shell = test_shell();
            let result = read_with_input(&mut shell, &[b"read".to_vec(), b"VAR".to_vec()], 42);
            assert!(matches!(result, Ok(BuiltinOutcome::Status(1))));
            assert_eq!(shell.get_var(b"VAR"), Some(b"partial".as_slice()));
        });
    }

    #[test]
    fn read_with_input_splits_into_multiple_vars() {
        let reads = byte_reads(42, b"alpha beta gamma\n");
        run_trace(trace_entries![..reads], || {
            let mut shell = test_shell();
            let result = read_with_input(
                &mut shell,
                &[b"read".to_vec(), b"A".to_vec(), b"B".to_vec()],
                42,
            );
            assert!(matches!(result, Ok(BuiltinOutcome::Status(0))));
            assert_eq!(shell.get_var(b"A"), Some(b"alpha".as_slice()));
            assert_eq!(shell.get_var(b"B"), Some(b"beta gamma".as_slice()));
        });
    }

    #[test]
    fn read_logical_line_backslash_newline_interactive_shows_ps2() {
        let before = byte_reads(42, b"first\\\n");
        let after = byte_reads_continuation(42, b"second\n");
        run_trace(
            trace_entries![
                ..before,
                write(fd(crate::sys::constants::STDERR_FILENO), bytes(b"> ")) -> auto,
                ..after,
            ],
            || {
                let mut shell = test_shell();
                shell.interactive = true;
                let options = ReadOptions {
                    raw: false,
                    delimiter: b'\n',
                    ..Default::default()
                };
                let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
                assert!(hit_delim);
                let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
                assert_eq!(text, b"firstsecond");
            },
        );
    }

    #[test]
    fn split_read_assignments_multibyte_ifs_c_locale() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            // In C locale, \xC3\xA9 is two bytes = two separate IFS chars.
            // Each byte acts as an independent delimiter.
            let result = split_read_assignments(
                &[(b"a\xc3\xa9b".to_vec(), false)],
                &[b"x".to_vec(), b"y".to_vec(), b"z".to_vec()],
                Some(b"\xc3\xa9".to_vec()),
            );
            assert_eq!(result, vec![b"a".to_vec(), Vec::new(), b"b".to_vec()]);
        });
    }

    #[test]
    fn split_read_assignments_multibyte_ifs_utf8_locale() {
        assert_no_syscalls(|| {
            set_test_locale_utf8();
            // In C.UTF-8, \xC3\xA9 is one character (U+00E9).
            // It acts as a single non-whitespace delimiter.
            let result = split_read_assignments(
                &[(b"a\xc3\xa9b".to_vec(), false)],
                &[b"x".to_vec(), b"y".to_vec()],
                Some(b"\xc3\xa9".to_vec()),
            );
            assert_eq!(result, vec![b"a".to_vec(), b"b".to_vec()]);
        });
    }

    #[test]
    fn split_read_assignments_quoted_ifs_delimiter_skipped() {
        assert_no_syscalls(|| {
            let result = split_read_assignments(
                &[
                    (b"a".to_vec(), false),
                    (b" ".to_vec(), true),
                    (b" b".to_vec(), false),
                ],
                &[b"X".to_vec(), b"Y".to_vec()],
                None,
            );
            assert_eq!(result, vec![b"a ".to_vec(), b"b".to_vec()]);
        });
    }

    #[test]
    fn read_logical_line_quoted_to_unquoted_transition() {
        let reads = byte_reads(42, b"\\ab\n");
        run_trace(trace_entries![..reads], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(hit_delim);
            assert!(pieces.len() >= 2, "should have quoted and unquoted pieces");
            let (_, first_quoted) = &pieces[0];
            let (_, second_quoted) = &pieces[1];
            assert!(*first_quoted);
            assert!(!*second_quoted);
        });
    }

    /// Seekable-fd happy path: a single `read()` returns a chunk that
    /// contains the delimiter plus trailing bytes; `ReadCursor::Drop`
    /// must `lseek` back by the unconsumed byte count so the caller's
    /// fd position lands exactly one byte past the delimiter, matching
    /// the byte-at-a-time semantics.
    #[test]
    fn read_logical_line_buffered_rewinds_past_delimiter() {
        let probe = t(
            "lseek",
            vec![
                ArgMatcher::Fd(42),
                ArgMatcher::Int(0),
                ArgMatcher::Int(sys::constants::SEEK_CUR as i64),
            ],
            TraceResult::Int(0),
        );
        let chunk = t(
            "read",
            vec![ArgMatcher::Fd(42), ArgMatcher::Any],
            TraceResult::Bytes(b"hello\nextra".to_vec()),
        );
        let rewind = t(
            "lseek",
            vec![
                ArgMatcher::Fd(42),
                ArgMatcher::Int(-5),
                ArgMatcher::Int(sys::constants::SEEK_CUR as i64),
            ],
            TraceResult::Int(6),
        );
        run_trace(trace_entries![..vec![probe, chunk, rewind]], || {
            let shell = test_shell();
            let options = ReadOptions {
                raw: false,
                delimiter: b'\n',
                ..Default::default()
            };
            let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
            assert!(hit_delim);
            let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            assert_eq!(text, b"hello");
        });
    }
}
