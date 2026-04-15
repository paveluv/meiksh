use super::{BuiltinOutcome, write_stdout_line};
use crate::bstr::{self, ByteWriter};
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn ulimit_resource_for_option(ch: u8) -> Option<(i32, &'static [u8], u64)> {
    match ch {
        b'c' => Some((sys::constants::RLIMIT_CORE, b"core file size (blocks)", 512)),
        b'd' => Some((sys::constants::RLIMIT_DATA, b"data seg size (kbytes)", 1024)),
        b'f' => Some((sys::constants::RLIMIT_FSIZE, b"file size (blocks)", 512)),
        b'n' => Some((sys::constants::RLIMIT_NOFILE, b"open files", 1)),
        b's' => Some((sys::constants::RLIMIT_STACK, b"stack size (kbytes)", 1024)),
        b't' => Some((sys::constants::RLIMIT_CPU, b"cpu time (seconds)", 1)),
        b'v' => Some((sys::constants::RLIMIT_AS, b"virtual memory (kbytes)", 1024)),
        _ => None,
    }
}

pub(super) fn format_limit(val: u64) -> Vec<u8> {
    if val == sys::constants::RLIM_INFINITY {
        b"unlimited".to_vec()
    } else {
        bstr::u64_to_bytes(val)
    }
}

pub(super) fn ulimit(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut use_hard = false;
    let mut use_soft = false;
    let mut report_all = false;
    let mut resource_opt: Option<u8> = None;
    let mut new_limit: Option<&[u8]> = None;

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg.first() == Some(&b'-') && arg.len() > 1 {
            for &ch in &arg[1..] {
                match ch {
                    b'H' => use_hard = true,
                    b'S' => use_soft = true,
                    b'a' => report_all = true,
                    b'c' | b'd' | b'f' | b'n' | b's' | b't' | b'v' => resource_opt = Some(ch),
                    _ => {
                        let msg = ByteWriter::new()
                            .bytes(b"ulimit: invalid option: -")
                            .byte(ch)
                            .finish();
                        return Err(shell.diagnostic(2, &msg));
                    }
                }
            }
        } else {
            new_limit = Some(arg);
        }
        i += 1;
    }

    if !use_hard && !use_soft {
        use_soft = true;
    }

    if report_all {
        for &opt in &[b'c', b'd', b'f', b'n', b's', b't', b'v'] {
            let (resource, desc, unit) = ulimit_resource_for_option(opt).unwrap();
            let (soft, hard) = sys::process::getrlimit(resource)
                .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
            let val = if use_hard { hard } else { soft };
            let display = if val == sys::constants::RLIM_INFINITY {
                b"unlimited".to_vec()
            } else {
                bstr::u64_to_bytes(val / unit)
            };
            let line = ByteWriter::new()
                .byte(b'-')
                .byte(opt)
                .bytes(b": ")
                .bytes(desc)
                .bytes(&vec![b' '; 40usize.saturating_sub(desc.len())])
                .byte(b' ')
                .bytes(&display)
                .finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }

    let opt = resource_opt.unwrap_or(b'f');
    let (resource, _desc, unit) = ulimit_resource_for_option(opt).unwrap();

    if let Some(val_str) = new_limit {
        let (soft, hard) = sys::process::getrlimit(resource)
            .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
        let raw_val = if val_str == b"unlimited" {
            sys::constants::RLIM_INFINITY
        } else {
            let Some(n) = bstr::parse_i64(val_str).filter(|&v| v >= 0) else {
                let msg = ByteWriter::new()
                    .bytes(b"ulimit: invalid limit: ")
                    .bytes(val_str)
                    .finish();
                return Err(shell.diagnostic(2, &msg));
            };
            n as u64 * unit
        };
        let new_soft = if use_soft { raw_val } else { soft };
        let new_hard = if use_hard { raw_val } else { hard };
        sys::process::setrlimit(resource, new_soft, new_hard)
            .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
        return Ok(BuiltinOutcome::Status(0));
    }

    let (soft, hard) = sys::process::getrlimit(resource)
        .map_err(|e| shell.diagnostic_prefixed_syserr(1, b"ulimit: ", &e))?;
    let val = if use_hard { hard } else { soft };
    let display = format_limit(if val == sys::constants::RLIM_INFINITY {
        val
    } else {
        val / unit
    });
    write_stdout_line(&display);
    Ok(BuiltinOutcome::Status(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys;

    #[test]
    fn format_limit_covers_both_branches() {
        assert_eq!(format_limit(sys::constants::RLIM_INFINITY), b"unlimited");
        assert_eq!(format_limit(42), b"42");
    }

    #[test]
    fn ulimit_resource_for_option_covers_all_and_unknown() {
        for ch in [b'c', b'd', b'f', b'n', b's', b't', b'v'] {
            assert!(ulimit_resource_for_option(ch).is_some());
        }
        assert!(ulimit_resource_for_option(b'z').is_none());
    }
}
