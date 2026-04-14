use crate::bstr;
use crate::shell::Shell;
use crate::sys;

pub(crate) fn check_mail(shell: &mut Shell) {
    let has_mail = shell.get_var(b"MAIL").is_some();
    let has_mailpath = shell.get_var(b"MAILPATH").is_some();
    if !has_mail && !has_mailpath {
        return;
    }

    let check_interval: u64 = shell
        .get_var(b"MAILCHECK")
        .and_then(|v| bstr::parse_i64(v).map(|n| n as u64))
        .unwrap_or(600);
    let now = sys::monotonic_clock_ns() / 1_000_000_000;
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
        let size = sys::stat_path(&path).map(|st| st.size).unwrap_or(0);
        let prev = shell.mail_sizes.get(path.as_slice()).copied().unwrap_or(0);
        if size > prev {
            let msg = custom_msg.unwrap_or_else(|| b"you have mail".to_vec());
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            let _ = sys::write_all_fd(sys::STDERR_FILENO, b"\n");
        }
        shell.mail_sizes.insert(path.into(), size);
    }
}

pub(crate) fn command_is_fc(line: &[u8]) -> bool {
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
