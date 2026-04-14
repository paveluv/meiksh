use crate::shell::{Shell, ShellError};
use crate::sys;

pub(super) fn append_history(shell: &Shell, line: &[u8]) -> Result<(), ShellError> {
    let history = history_path(shell);
    let fd = match sys::open_file(
        &history,
        sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND,
        0o644,
    ) {
        Ok(fd) => fd,
        Err(_) => return Ok(()),
    };
    let mut entry = line.to_vec();
    if entry.is_empty() || entry[entry.len() - 1] != b'\n' {
        entry.push(b'\n');
    }
    let _ = sys::write_all_fd(fd, &entry);
    sys::close_fd(fd).map_err(|e| shell.diagnostic_syserr(1, &e))?;
    Ok(())
}

pub(super) fn history_path(shell: &Shell) -> Vec<u8> {
    shell
        .get_var(b"HISTFILE")
        .map(|s| s.to_vec())
        .or_else(|| {
            shell.get_var(b"HOME").map(|home| {
                let mut path = home.to_vec();
                path.extend_from_slice(b"/.sh_history");
                path
            })
        })
        .unwrap_or_else(|| b".sh_history".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::*;

    #[test]
    fn append_history_writes_to_histfile() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/history.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Bytes(b"echo hi\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert(b"HISTFILE".to_vec(), b"/tmp/history.txt".to_vec());
                append_history(&shell, b"echo hi\n").expect("append history");
            },
        );
    }

    #[test]
    fn append_history_silently_ignores_open_error() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/history-dir".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Err(sys::EISDIR),
            )],
            || {
                let mut shell = test_shell();
                shell
                    .env
                    .insert(b"HISTFILE".to_vec(), b"/tmp/history-dir".to_vec());
                append_history(&shell, b"echo hi\n").expect("should silently succeed");
            },
        );
    }

    #[test]
    fn append_history_uses_default_path_when_histfile_unset() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/home/user/.sh_history".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(10),
                        ArgMatcher::Bytes(b"echo default\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
                append_history(&shell, b"echo default\n").expect("default history");
            },
        );
    }
}
