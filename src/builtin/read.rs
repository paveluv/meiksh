use super::{BuiltinOutcome, diag_status, var_error_msg};
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

#[derive(Clone, Copy)]
pub(super) struct ReadOptions {
    pub(super) raw: bool,
    pub(super) delimiter: u8,
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

    let (pieces, hit_delimiter) = match read_logical_line(shell, options, input_fd) {
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

pub(super) fn parse_read_options(argv: &[Vec<u8>]) -> Option<(ReadOptions, Vec<Vec<u8>>)> {
    let mut options = ReadOptions {
        raw: false,
        delimiter: b'\n',
    };
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
            _ if arg.first() == Some(&b'-') && arg != b"-" => return None,
            _ => break,
        }
    }
    Some((options, argv[index..].to_vec()))
}

pub(super) fn read_logical_line(
    shell: &Shell,
    options: ReadOptions,
    input_fd: i32,
) -> sys::error::SysResult<(Vec<(Vec<u8>, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = Vec::new();
    let mut current_quoted = false;

    loop {
        let mut byte = [0u8; 1];
        let count = sys::fd_io::read_fd(input_fd, &mut byte)?;
        if count == 0 {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, false));
        }
        let ch = byte[0];
        if !options.raw && ch == b'\\' {
            let count = sys::fd_io::read_fd(input_fd, &mut byte)?;
            if count == 0 {
                current.push(b'\\');
                push_read_piece(&mut pieces, &mut current, current_quoted);
                return Ok((pieces, false));
            }
            let escaped = byte[0];
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
    use crate::sys::test_support::{ArgMatcher, TraceResult, assert_no_syscalls, run_trace, t};
    use crate::trace_entries;

    fn byte_reads(fd: i32, input: &[u8]) -> Vec<crate::sys::test_support::TraceEntry> {
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
        let eio_str = crate::sys::error::SysError::Errno(libc::EIO).strerror();
        let mut diag_body = b"read: ".to_vec();
        diag_body.extend_from_slice(&eio_str);
        let msg = diag(&diag_body);
        let reads = vec![t(
            "read",
            vec![ArgMatcher::Fd(42), ArgMatcher::Any],
            TraceResult::Err(libc::EIO),
        )];
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
        let after = byte_reads(42, b"second\n");
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
                };
                let (pieces, hit_delim) = read_logical_line(&shell, options, 42).expect("read");
                assert!(hit_delim);
                let text: Vec<u8> = pieces.iter().flat_map(|(p, _)| p.iter().copied()).collect();
                assert_eq!(text, b"firstsecond");
            },
        );
    }

    #[test]
    fn split_read_assignments_multibyte_ifs_char() {
        // decompose_read_ifs + find_read_ifs_at are pure-logic functions that
        // call decode_char.  In the ASCII test interface decode_char returns 1
        // for every byte, so multi-byte IFS chars appear as separate entries.
        // Verify the algorithm directly: two-byte IFS "\xC3\xA9" must be kept
        // as one entry by decompose_read_ifs when decode_char reports len=2.
        assert_no_syscalls(|| {
            let ifs = decompose_read_ifs(b"\xc3\xa9");
            // ASCII fallback: each byte is its own char → 2 entries
            assert_eq!(ifs.len(), 2);
        });

        // Functional test via the matrix suite covers the real UTF-8 path.
        // Here we unit-test the split logic assuming correct decomposition:
        // construct a ReadIfsChar manually and verify find + split.
        assert_no_syscalls(|| {
            let ifs_chars = vec![ReadIfsChar {
                byte_seq: b"\xc3\xa9".to_vec().into(),
                is_ws: false,
            }];
            // find_read_ifs_at should match the two-byte sequence
            assert!(find_read_ifs_at(&ifs_chars, b"\xc3\xa9b").is_some());
            assert!(find_read_ifs_at(&ifs_chars, b"\xc3").is_none());
            assert!(find_read_ifs_at(&ifs_chars, b"a").is_none());

            // Build flat bytes and verify splitting
            let bytes: Vec<(u8, bool)> = b"a\xc3\xa9b".iter().map(|&b| (b, false)).collect();
            let mut idx = 0;
            // 'a' is not IFS
            assert!(find_read_ifs_at(&ifs_chars, &unquoted_tail(&bytes, idx)).is_none());
            idx += 1;
            // '\xc3\xa9' matches the multi-byte IFS char
            let (seq, is_ws) = find_read_ifs_at(&ifs_chars, &unquoted_tail(&bytes, idx)).unwrap();
            assert_eq!(seq.len(), 2);
            assert!(!is_ws);
            idx += seq.len();
            // 'b' is not IFS
            assert!(find_read_ifs_at(&ifs_chars, &unquoted_tail(&bytes, idx)).is_none());
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
}
