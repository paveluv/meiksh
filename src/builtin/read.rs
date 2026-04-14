use super::*;

#[derive(Clone, Copy)]
pub(super) struct ReadOptions {
    pub(super) raw: bool,
    pub(super) delimiter: u8,
}

pub(super) fn read(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    sys::ensure_blocking_read_fd(sys::STDIN_FILENO)
        .map_err(|e| shell.diagnostic(1, &e.strerror()))?;
    read_with_input(shell, argv, sys::STDIN_FILENO)
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
        if let Err(error) = shell.set_var(name, value) {
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
) -> sys::SysResult<(Vec<(Vec<u8>, bool)>, bool)> {
    let mut pieces = Vec::new();
    let mut current = Vec::new();
    let mut current_quoted = false;

    loop {
        let mut byte = [0u8; 1];
        let count = sys::read_fd(input_fd, &mut byte)?;
        if count == 0 {
            push_read_piece(&mut pieces, &mut current, current_quoted);
            return Ok((pieces, false));
        }
        let ch = byte[0];
        if !options.raw && ch == b'\\' {
            let count = sys::read_fd(input_fd, &mut byte)?;
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
                    let _ = sys::write_all_fd(sys::STDERR_FILENO, prompt);
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

    let ifs_ws: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|&ch| matches!(ch, b' ' | b'\t' | b'\n'))
        .collect();
    let ifs_other: Vec<u8> = ifs
        .iter()
        .copied()
        .filter(|&ch| !matches!(ch, b' ' | b'\t' | b'\n'))
        .collect();
    let chars = flatten_read_chars(pieces);
    if vars.len() == 1 {
        return vec![trim_read_ifs_whitespace(&chars, &ifs_ws)];
    }

    let mut values = Vec::new();
    let mut index = 0usize;
    skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
    while index < chars.len() && values.len() + 1 < vars.len() {
        let mut current = Vec::new();
        loop {
            if index >= chars.len() {
                values.push(current);
                break;
            }
            let (ch, quoted) = chars[index];
            if !quoted && ifs_other.contains(&ch) {
                values.push(current);
                index += 1;
                skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
                break;
            }
            if !quoted && ifs_ws.contains(&ch) {
                debug_assert!(
                    !current.is_empty(),
                    "leading IFS whitespace should already be skipped"
                );
                values.push(current);
                skip_read_ifs_whitespace(&chars, &ifs_ws, &mut index);
                break;
            }
            current.push(ch);
            index += 1;
        }
    }

    values.push(trim_read_ifs_whitespace(&chars[index..], &ifs_ws));
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

pub(super) fn flatten_read_chars(pieces: &[(Vec<u8>, bool)]) -> Vec<(u8, bool)> {
    let mut chars = Vec::new();
    for (text, quoted) in pieces {
        for &ch in text.iter() {
            chars.push((ch, *quoted));
        }
    }
    chars
}

pub(super) fn skip_read_ifs_whitespace(chars: &[(u8, bool)], ifs_ws: &[u8], index: &mut usize) {
    while *index < chars.len() && !chars[*index].1 && ifs_ws.contains(&chars[*index].0) {
        *index += 1;
    }
}

pub(super) fn trim_read_ifs_whitespace(chars: &[(u8, bool)], ifs_ws: &[u8]) -> Vec<u8> {
    let mut start = 0usize;
    let mut end = chars.len();
    while start < end && !chars[start].1 && ifs_ws.contains(&chars[start].0) {
        start += 1;
    }
    while end > start && !chars[end - 1].1 && ifs_ws.contains(&chars[end - 1].0) {
        end -= 1;
    }
    chars[start..end].iter().map(|(ch, _)| *ch).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;

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
}
