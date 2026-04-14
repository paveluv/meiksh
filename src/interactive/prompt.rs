use crate::arena::ByteArena;
use crate::bstr;
use crate::expand;
use crate::shell::Shell;
use crate::sys;

pub(super) fn write_prompt(prompt_str: &[u8]) -> sys::SysResult<()> {
    loop {
        match sys::write_all_fd(sys::STDERR_FILENO, prompt_str) {
            Ok(()) => return Ok(()),
            Err(e) if e.is_eintr() => continue,
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn read_line() -> sys::SysResult<Option<Vec<u8>>> {
    let mut line = Vec::<u8>::new();
    let mut byte = [0u8; 1];
    loop {
        match sys::read_fd(sys::STDIN_FILENO, &mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(line) }),
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    return Ok(Some(line));
                }
            }
            Err(e) if e.is_eintr() => {
                let _ = sys::write_all_fd(sys::STDERR_FILENO, b"\n");
                return Ok(Some(Vec::new()));
            }
            Err(e) => return Err(e),
        }
    }
}

pub(super) fn expand_prompt(shell: &mut Shell, var: &[u8], default: &[u8]) -> Vec<u8> {
    let raw = shell.get_var(var).unwrap_or(default).to_vec();
    let histnum = shell.history_number();
    let arena = ByteArena::new();
    let expanded = expand::expand_parameter_text(shell, &raw, &arena).unwrap_or(&raw);
    expand_prompt_exclamation(expanded, histnum)
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
