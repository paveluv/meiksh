use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn append_history(shell: &Shell, line: &[u8]) -> Result<(), ShellError> {
    let history = history_path(shell);
    let fd = match sys::fs::open_file(
        &history,
        sys::constants::O_WRONLY | sys::constants::O_CREAT | sys::constants::O_APPEND,
        0o644,
    ) {
        Ok(fd) => fd,
        Err(_) => return Ok(()),
    };
    let mut entry = line.to_vec();
    if entry.is_empty() || entry[entry.len() - 1] != b'\n' {
        entry.push(b'\n');
    }
    let _ = sys::fd_io::write_all_fd(fd, &entry);
    sys::fd_io::close_fd(fd).map_err(|e| shell.diagnostic_syserr(1, &e))?;
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
    use crate::interactive::test_support::test_shell;
    use crate::sys;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn append_history_writes_to_histfile() {
        run_trace(
            trace_entries![
                open(str("/tmp/history.txt"), _, _) -> fd(10),
                write(fd(10), bytes(b"echo hi\n")) -> auto,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/history.txt".to_vec());
                append_history(&shell, b"echo hi\n").expect("append history");
            },
        );
    }

    #[test]
    fn append_history_silently_ignores_open_error() {
        run_trace(
            trace_entries![
                open(str("/tmp/history-dir"), _, _) -> err(sys::constants::EISDIR),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HISTFILE".to_vec(), b"/tmp/history-dir".to_vec());
                append_history(&shell, b"echo hi\n").expect("should silently succeed");
            },
        );
    }

    #[test]
    fn append_history_uses_default_path_when_histfile_unset() {
        run_trace(
            trace_entries![
                open(str("/home/user/.sh_history"), _, _) -> fd(10),
                write(fd(10), bytes(b"echo default\n")) -> auto,
                close(fd(10)) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"HOME".to_vec(), b"/home/user".to_vec());
                append_history(&shell, b"echo default\n").expect("default history");
            },
        );
    }
}
