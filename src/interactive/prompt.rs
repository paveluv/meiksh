use crate::bstr;
use crate::expand::word;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn write_prompt(prompt_str: &[u8]) -> sys::error::SysResult<()> {
    loop {
        match sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, prompt_str) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn read_line() -> sys::error::SysResult<Option<Vec<u8>>> {
    let mut line = Vec::<u8>::new();
    let mut byte = [0u8; 1];
    loop {
        match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(line) }),
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    return Ok(Some(line));
                }
            }
            Err(e) if e.is_eintr() => {
                let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, b"\n");
                return Ok(Some(Vec::new()));
            }
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn expand_prompt(shell: &mut Shell, var: &[u8], default: &[u8]) -> Vec<u8> {
    let raw = shell.get_var(var).unwrap_or(default).to_vec();
    let histnum = shell.history_number();
    let expanded = word::expand_parameter_text(shell, &raw).unwrap_or_else(|_| raw.clone());
    expand_prompt_exclamation(&expanded, histnum)
}

pub(super) fn expand_prompt_exclamation(s: &[u8], histnum: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'!' {
            i += 1;
            if i < s.len() && s[i] == b'!' {
                result.push(b'!');
                i += 1;
            } else if i < s.len() {
                bstr::push_u64(&mut result, histnum as u64);
                result.push(s[i]);
                i += 1;
            } else {
                bstr::push_u64(&mut result, histnum as u64);
            }
        } else {
            result.push(s[i]);
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn prompt_prefers_ps1() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"$ ");
            shell.env.insert(b"PS1".to_vec(), b"custom> ".to_vec());
            assert_eq!(expand_prompt(&mut shell, b"PS1", b"$ "), b"custom> ");
        });
    }

    #[test]
    fn read_line_propagates_non_eintr_error() {
        run_trace(
            trace_entries![read(fd(sys::constants::STDIN_FILENO), _) -> err(sys::constants::EBADF)],
            || {
                let err = read_line().expect_err("should propagate EBADF");
                assert!(!err.is_eintr());
            },
        );
    }

    #[test]
    fn read_line_returns_empty_on_eintr() {
        run_trace(
            trace_entries![
                read(fd(sys::constants::STDIN_FILENO), _) -> err(sys::constants::EINTR),
                write(fd(sys::constants::STDERR_FILENO), bytes(b"\n")) -> auto,
            ],
            || {
                let result = read_line().expect("should not fail on EINTR");
                assert_eq!(result, Some(Vec::new()));
            },
        );
    }

    #[test]
    fn expand_prompt_exclamation_covers_all_branches() {
        assert_no_syscalls(|| {
            assert_eq!(expand_prompt_exclamation(b"!!", 42), b"!");
            assert_eq!(expand_prompt_exclamation(b"!x", 42), b"42x");
            assert_eq!(expand_prompt_exclamation(b"!", 42), b"42");
            assert_eq!(expand_prompt_exclamation(b"no bang", 42), b"no bang");
        });
    }
}
