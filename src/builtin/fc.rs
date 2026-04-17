use super::{BuiltinOutcome, remove_file_bytes, write_stdout_line};
use crate::bstr::{self, BStrExt, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

fn find_on_char_boundary(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(needle) {
            return Some(i);
        }
        let (_, len) = crate::sys::locale::decode_char(&haystack[i..]);
        i += if len == 0 { 1 } else { len };
    }
    None
}

pub(super) fn fc_resolve_operand(history: &[Box<[u8]>], op: &[u8]) -> Option<usize> {
    if let Some(n) = bstr::parse_i64(op) {
        if n > 0 {
            let idx = (n as usize).saturating_sub(1);
            return if idx < history.len() { Some(idx) } else { None };
        }
        let offset = n.unsigned_abs() as usize;
        return history.len().checked_sub(offset);
    }
    history.iter().rposition(|entry| entry.starts_with(op))
}

pub(super) fn fc(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut list_mode = false;
    let mut suppress_numbers = false;
    let mut reverse = false;
    let mut reexec = false;
    let mut editor: Option<Vec<u8>> = None;
    let mut operands = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == b"--" {
            i += 1;
            operands.extend(argv[i..].iter().cloned());
            break;
        }
        if arg.first() == Some(&b'-')
            && arg.len() > 1
            && !arg[1..].first().map_or(false, |c| c.is_ascii_digit())
        {
            let mut j = 1;
            while j < arg.len() {
                let ch = arg[j];
                match ch {
                    b'l' => list_mode = true,
                    b'n' => suppress_numbers = true,
                    b'r' => reverse = true,
                    b's' => reexec = true,
                    b'e' => {
                        let rest = &arg[j + 1..];
                        if rest.is_empty() {
                            i += 1;
                            if i >= argv.len() {
                                return Err(shell.diagnostic(2, b"fc: -e requires an argument"));
                            }
                            editor = Some(argv[i].clone());
                        } else {
                            editor = Some(rest.to_vec());
                        }
                        break;
                    }
                    _ => {
                        let msg = ByteWriter::new()
                            .bytes(b"fc: invalid option: -")
                            .byte(ch)
                            .finish();
                        return Err(shell.diagnostic(2, &msg));
                    }
                }
                j += 1;
            }
        } else {
            operands.push(arg.clone());
        }
        i += 1;
    }

    let history = shell.history();
    if history.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }

    if reexec {
        let mut substitution: Option<(&[u8], &[u8])> = None;
        let mut first_operand: Option<&[u8]> = None;
        for op in &operands {
            if let Some((old, new)) = op.split_once_byte(b'=') {
                substitution = Some((old, new));
            } else {
                first_operand = Some(op);
            }
        }
        let idx = match first_operand {
            Some(op) => fc_resolve_operand(history, op).ok_or_else(|| {
                let msg = ByteWriter::new()
                    .bytes(b"fc: no command found matching '")
                    .bytes(op)
                    .bytes(b"'")
                    .finish();
                shell.diagnostic(1, &msg)
            })?,
            None => history.len() - 1,
        };
        let mut cmd = history[idx].to_vec();
        if let Some((old, new)) = substitution {
            if let Some(pos) = find_on_char_boundary(&cmd, old) {
                let mut replaced = cmd[..pos].to_vec();
                replaced.extend_from_slice(new);
                replaced.extend_from_slice(&cmd[pos + old.len()..]);
                cmd = replaced;
            }
        }
        shell.add_history(&cmd);
        let status = shell
            .execute_string(&cmd)
            .unwrap_or_else(|e| e.exit_status());
        shell.last_status = status;
        return Ok(BuiltinOutcome::Status(status));
    }

    if list_mode {
        let (first, last) = match operands.len() {
            0 => {
                let end = history.len().saturating_sub(1);
                let start = end.saturating_sub(15);
                (start, end)
            }
            1 => {
                let a = fc_resolve_operand(history, &operands[0])
                    .unwrap_or(history.len().saturating_sub(1));
                (a, history.len().saturating_sub(1))
            }
            _ => {
                let a = fc_resolve_operand(history, &operands[0])
                    .unwrap_or(history.len().saturating_sub(1));
                let b = fc_resolve_operand(history, &operands[1])
                    .unwrap_or(history.len().saturating_sub(1));
                (a, b)
            }
        };

        let (lo, hi) = if first <= last {
            (first, last)
        } else {
            (last, first)
        };

        let do_reverse = if first <= last { reverse } else { !reverse };

        if do_reverse {
            for idx in (lo..=hi).rev() {
                if suppress_numbers {
                    let line = ByteWriter::new().byte(b'\t').bytes(&history[idx]).finish();
                    write_stdout_line(&line);
                } else {
                    let line = ByteWriter::new()
                        .usize_val(idx + 1)
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                }
            }
        } else {
            for idx in lo..=hi {
                if suppress_numbers {
                    let line = ByteWriter::new().byte(b'\t').bytes(&history[idx]).finish();
                    write_stdout_line(&line);
                } else {
                    let line = ByteWriter::new()
                        .usize_val(idx + 1)
                        .byte(b'\t')
                        .bytes(&history[idx])
                        .finish();
                    write_stdout_line(&line);
                }
            }
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let idx = if operands.is_empty() {
        history.len() - 1
    } else {
        fc_resolve_operand(history, &operands[0])
            .ok_or_else(|| shell.diagnostic(1, b"fc: history specification out of range"))?
    };

    let editor_cmd = match editor {
        Some(ref e) => e.as_slice(),
        None => shell.get_var(b"FCEDIT").unwrap_or(b"ed"),
    };

    let tmp_path = ByteWriter::new()
        .bytes(b"/tmp/fc_edit_")
        .i64_val(sys::process::current_pid() as i64)
        .finish();
    let cmd_text = &history[idx];
    let fd = sys::fs::open_file(
        &tmp_path,
        sys::constants::O_WRONLY | sys::constants::O_CREAT | sys::constants::O_TRUNC,
        0o600,
    )
    .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"fc: ", &e))?;
    let _ = sys::fd_io::write_all_fd(fd, cmd_text);
    let _ = sys::fd_io::write_all_fd(fd, b"\n");
    let _ = sys::fd_io::close_fd(fd);

    let edit_cmd = ByteWriter::new()
        .bytes(editor_cmd)
        .byte(b' ')
        .bytes(&tmp_path)
        .finish();
    let edit_status = shell
        .execute_string(&edit_cmd)
        .unwrap_or_else(|e| e.exit_status());
    if edit_status != 0 {
        remove_file_bytes(&tmp_path);
        return Ok(BuiltinOutcome::Status(edit_status));
    }

    let edited = sys::fs::read_file(&tmp_path)
        .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"fc: ", &e))?;
    remove_file_bytes(&tmp_path);

    let edited = edited.trim_trailing_newlines();
    if !edited.is_empty() {
        shell.add_history(edited);
        let status = shell
            .execute_string(edited)
            .unwrap_or_else(|e| e.exit_status());
        shell.last_status = status;
        return Ok(BuiltinOutcome::Status(status));
    }

    Ok(BuiltinOutcome::Status(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_on_char_boundary_returns_none_when_not_found() {
        crate::sys::test_support::assert_no_syscalls(|| {
            assert_eq!(find_on_char_boundary(b"hello", b"xyz"), None);
            assert_eq!(find_on_char_boundary(b"hello", b"lo"), Some(3));
        });
    }

    #[test]
    fn fc_resolve_operand_covers_positive_negative_and_string() {
        let h: Vec<Box<[u8]>> = vec![
            b"alpha".to_vec().into(),
            b"beta".to_vec().into(),
            b"gamma".to_vec().into(),
        ];
        assert_eq!(fc_resolve_operand(&h, b"1"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"3"), Some(2));
        assert_eq!(fc_resolve_operand(&h, b"99"), None);
        assert_eq!(fc_resolve_operand(&h, b"-1"), Some(2));
        assert_eq!(fc_resolve_operand(&h, b"-3"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"al"), Some(0));
        assert_eq!(fc_resolve_operand(&h, b"be"), Some(1));
        assert_eq!(fc_resolve_operand(&h, b"zzz"), None);
    }
}
