use crate::bstr;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn check_mail(shell: &mut Shell) {
    let has_mail = shell.get_var(b"MAIL").is_some();
    let has_mailpath = shell.get_var(b"MAILPATH").is_some();
    if !has_mail && !has_mailpath {
        return;
    }

    let check_interval: u64 = shell
        .get_var(b"MAILCHECK")
        .and_then(|v| bstr::parse_i64(v).map(|n| n as u64))
        .unwrap_or(600);
    let now = sys::time::monotonic_clock_ns() / 1_000_000_000;
    if shell.mail_last_check != 0 && now.saturating_sub(shell.mail_last_check) < check_interval {
        return;
    }
    shell.mail_last_check = now;

    let entries: Vec<(Vec<u8>, Option<Vec<u8>>)> =
        if let Some(mp) = shell.get_var(b"MAILPATH").map(|s| s.to_vec()) {
            let mut result = Vec::new();
            for entry in mp.split(|&b| b == b':') {
                match entry.iter().position(|&b| b == b'%') {
                    Some(pos) => {
                        result.push((entry[..pos].to_vec(), Some(entry[pos + 1..].to_vec())));
                    }
                    None => {
                        result.push((entry.to_vec(), None));
                    }
                }
            }
            result
        } else {
            let m = shell.get_var(b"MAIL").unwrap().to_vec();
            vec![(m, None)]
        };

    for (path, custom_msg) in entries {
        if path.is_empty() {
            continue;
        }
        let size = sys::fs::stat_path(&path).map(|st| st.size).unwrap_or(0);
        let prev = shell
            .mail_sizes()
            .get(path.as_slice())
            .copied()
            .unwrap_or(0);
        if size > prev {
            let msg = custom_msg.unwrap_or_else(|| b"you have mail".to_vec());
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, &msg);
            let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, b"\n");
        }
        shell.mail_sizes_mut().insert(path.into(), size);
    }
}

pub(super) fn command_is_fc(line: &[u8]) -> bool {
    let mut rest = line;
    loop {
        while !rest.is_empty() && rest[0].is_ascii_whitespace() {
            rest = &rest[1..];
        }
        if rest.is_empty() {
            return false;
        }
        if let Some(eq_pos) = rest.iter().position(|&b| b == b'=') {
            let before_eq = &rest[..eq_pos];
            if !before_eq.is_empty()
                && !before_eq.iter().any(|b| b.is_ascii_whitespace())
                && before_eq
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                let after_eq = &rest[eq_pos + 1..];
                let skip = if !after_eq.is_empty() && after_eq[0] == b'\'' {
                    after_eq[1..]
                        .iter()
                        .position(|&b| b == b'\'')
                        .map(|i| i + 2)
                } else if !after_eq.is_empty() && after_eq[0] == b'"' {
                    after_eq[1..].iter().position(|&b| b == b'"').map(|i| i + 2)
                } else {
                    after_eq.iter().position(|b| b.is_ascii_whitespace())
                };
                match skip {
                    Some(n) => {
                        rest = &after_eq[n..];
                        continue;
                    }
                    None => return false,
                }
            }
        }
        return rest == b"fc"
            || (rest.len() > 3 && &rest[..3] == b"fc ")
            || (rest.len() > 3 && &rest[..3] == b"fc\t");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::test_support::test_shell;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn command_is_fc_tests() {
        assert_no_syscalls(|| {
            assert!(command_is_fc(b"fc"));
            assert!(command_is_fc(b"fc -l"));
            assert!(command_is_fc(b"fc\t-l"));
            assert!(command_is_fc(b"FCEDIT=true fc -e true"));
            assert!(command_is_fc(b"A=1 B=2 fc"));
            assert!(command_is_fc(b"X='val' fc -s"));
            assert!(command_is_fc(b"X=\"val\" fc -s"));
            assert!(!command_is_fc(b"echo fc"));
            assert!(!command_is_fc(b""));
            assert!(!command_is_fc(b"echo hello"));
            assert!(!command_is_fc(b"FCEDIT=true"));
        });
    }

    #[test]
    fn check_mail_noop_when_no_mail_set() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            check_mail(&mut shell);
        });
    }

    #[test]
    fn check_mail_detects_new_mail() {
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                stat(str("/tmp/test_mail"), _) -> stat_file_size(42),
                write(fd(2), bytes(b"you have mail")) -> auto,
                write(fd(2), bytes(b"\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"MAIL", b"/tmp/test_mail");
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_with_mailpath_and_custom_message() {
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                stat(str("/tmp/box1"), _) -> stat_file_size(10),
                write(fd(2), bytes(b"New mail!")) -> auto,
                write(fd(2), bytes(b"\n")) -> auto,
                stat(str("/tmp/box2"), _) -> stat_file_size(0),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"MAILPATH", b"/tmp/box1%New mail!:/tmp/box2");
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_skips_empty_path() {
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                stat(str("/tmp/box"), _) -> stat_file_size(0),
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"MAILPATH", b":/tmp/box");
                check_mail(&mut shell);
            },
        );
    }

    #[test]
    fn check_mail_respects_interval() {
        use crate::sys::test_support::run_trace;
        use crate::trace_entries;
        run_trace(
            trace_entries![
                monotonic_clock_ns() -> 1_000_000_000,
                stat(str("/tmp/mbox"), _) -> stat_file_size(0),
                monotonic_clock_ns() -> 2_000_000_000,
            ],
            || {
                let mut shell = test_shell();
                let _ = shell.set_var(b"MAIL", b"/tmp/mbox");
                check_mail(&mut shell);
                check_mail(&mut shell);
            },
        );
    }
}
