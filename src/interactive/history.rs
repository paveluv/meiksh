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
