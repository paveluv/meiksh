use super::*;

pub(super) fn umask(shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut symbolic_output = false;
    let mut index = 1usize;
    while let Some(arg) = argv.get(index) {
        match arg.as_slice() {
            b"-S" => {
                symbolic_output = true;
                index += 1;
            }
            b"--" => {
                index += 1;
                break;
            }
            _ if arg.first() == Some(&b'-') && arg != b"-" => {
                let msg = ByteWriter::new()
                    .bytes(b"umask: invalid option: ")
                    .bytes(arg)
                    .finish();
                return Ok(diag_status(shell, 1, &msg));
            }
            _ => break,
        }
    }

    let current = sys::current_umask() as u16;
    if index == argv.len() {
        if symbolic_output {
            write_stdout_line(&format_umask_symbolic(current));
        } else {
            let line = ByteWriter::new().octal_padded(current as u64, 4).finish();
            write_stdout_line(&line);
        }
        return Ok(BuiltinOutcome::Status(0));
    }
    if index + 1 != argv.len() {
        return Ok(diag_status(shell, 1, b"umask: too many arguments"));
    }

    let Some(mask) = parse_umask_mask(&argv[index], current) else {
        let msg = ByteWriter::new()
            .bytes(b"umask: invalid mask: ")
            .bytes(&argv[index])
            .finish();
        return Ok(diag_status(shell, 1, &msg));
    };
    sys::set_umask(mask as sys::FileModeMask);
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn parse_umask_mask(mask: &[u8], current_mask: u16) -> Option<u16> {
    if !mask.is_empty() && mask.iter().all(|&ch| matches!(ch, b'0'..=b'7')) {
        let mut val = 0u16;
        for &ch in mask {
            val = val * 8 + (ch - b'0') as u16;
        }
        return Some(val & 0o777);
    }
    parse_symbolic_umask(mask, current_mask)
}

pub(super) fn parse_symbolic_umask(mask: &[u8], current_mask: u16) -> Option<u16> {
    let mut allowed = (!current_mask) & 0o777;
    for clause in mask.split(|&b| b == b',') {
        if clause.is_empty() {
            return None;
        }
        let (targets, op, perms) = parse_symbolic_clause(clause)?;
        let perm_bits = symbolic_permission_bits(perms, targets, allowed)?;
        if op == b'+' {
            allowed |= perm_bits;
        } else if op == b'-' {
            allowed &= !perm_bits;
        } else {
            allowed = (allowed & !targets) | (perm_bits & targets);
        }
    }
    Some((!allowed) & 0o777)
}

pub(super) fn parse_symbolic_clause(clause: &[u8]) -> Option<(u16, u8, &[u8])> {
    let mut split_at = 0usize;
    for &ch in clause {
        if matches!(ch, b'u' | b'g' | b'o' | b'a') {
            split_at += 1;
        } else {
            break;
        }
    }
    let (who_text, rest) = clause.split_at(split_at);
    if rest.is_empty() {
        return None;
    }
    let op = rest[0];
    if !matches!(op, b'+' | b'-' | b'=') {
        return None;
    }
    let perms = &rest[1..];
    Some((parse_symbolic_targets(who_text), op, perms))
}

pub(super) fn parse_symbolic_targets(who_text: &[u8]) -> u16 {
    if who_text.is_empty() {
        return 0o777;
    }
    let mut targets = 0u16;
    for &ch in who_text {
        match ch {
            b'u' => targets |= 0o700,
            b'g' => targets |= 0o070,
            b'o' => targets |= 0o007,
            b'a' => targets |= 0o777,
            _ => {}
        }
    }
    targets
}

pub(super) fn symbolic_permission_bits(perms: &[u8], targets: u16, allowed: u16) -> Option<u16> {
    let mut bits = 0u16;
    for &ch in perms {
        bits |= match ch {
            b'r' => permission_bits_for_targets(targets, 0o444),
            b'w' => permission_bits_for_targets(targets, 0o222),
            b'x' => permission_bits_for_targets(targets, 0o111),
            b'X' => permission_bits_for_targets(targets, 0o111),
            b's' => 0,
            b'u' => copy_permission_bits(allowed, targets, 0o700),
            b'g' => copy_permission_bits(allowed, targets, 0o070),
            b'o' => copy_permission_bits(allowed, targets, 0o007),
            _ => return None,
        };
    }
    Some(bits)
}

pub(super) fn permission_bits_for_targets(targets: u16, mask: u16) -> u16 {
    let mut bits = 0u16;
    if targets & 0o700 != 0 {
        bits |= mask & 0o700;
    }
    if targets & 0o070 != 0 {
        bits |= mask & 0o070;
    }
    if targets & 0o007 != 0 {
        bits |= mask & 0o007;
    }
    bits
}

pub(super) fn copy_permission_bits(allowed: u16, targets: u16, source_class: u16) -> u16 {
    let source = match source_class {
        0o700 => (allowed & 0o700) >> 6,
        0o070 => (allowed & 0o070) >> 3,
        0o007 => allowed & 0o007,
        _ => 0,
    };
    let mut bits = 0u16;
    if targets & 0o700 != 0 {
        bits |= source << 6;
    }
    if targets & 0o070 != 0 {
        bits |= source << 3;
    }
    if targets & 0o007 != 0 {
        bits |= source;
    }
    bits
}

pub(super) fn format_umask_symbolic(mask: u16) -> Vec<u8> {
    ByteWriter::new()
        .bytes(b"u=")
        .bytes(&symbolic_permissions_for_class(mask, 0o700, 6))
        .bytes(b",g=")
        .bytes(&symbolic_permissions_for_class(mask, 0o070, 3))
        .bytes(b",o=")
        .bytes(&symbolic_permissions_for_class(mask, 0o007, 0))
        .finish()
}

pub(super) fn symbolic_permissions_for_class(mask: u16, class_mask: u16, shift: u16) -> Vec<u8> {
    let allowed = ((!mask) & class_mask) >> shift;
    let mut result = Vec::new();
    if allowed & 0b100 != 0 {
        result.push(b'r');
    }
    if allowed & 0b010 != 0 {
        result.push(b'w');
    }
    if allowed & 0b001 != 0 {
        result.push(b'x');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn umask_parsing_helpers() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask(b"077", 0o022), Some(0o077));
            assert_eq!(parse_umask_mask(b"g-w", 0o002), Some(0o022));
            assert_eq!(parse_umask_mask(b"u=rw,go=r", 0o022), Some(0o133));
            assert_eq!(parse_umask_mask(b"a+x", 0o777), Some(0o666));
            assert_eq!(parse_umask_mask(b"u=g", 0o022), Some(0o222));
            assert_eq!(parse_umask_mask(b"u=o", 0o022), Some(0o222));
            assert_eq!(format_umask_symbolic(0o022), b"u=rwx,g=rx,o=rx");
            assert_eq!(parse_symbolic_targets(b"z"), 0);
            assert_eq!(permission_bits_for_targets(0o070, 0o111), 0o010);
            assert_eq!(permission_bits_for_targets(0o007, 0o444), 0o004);
            assert_eq!(copy_permission_bits(0o754, 0o070, 0o070), 0o050);
            assert_eq!(copy_permission_bits(0o754, 0o007, 0o007), 0o004);
            assert_eq!(copy_permission_bits(0o754, 0o700, 0), 0);
        });
    }

    #[test]
    fn umask_display_octal() {
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"0022\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(&mut shell, &[b"umask".to_vec()]).expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn umask_display_symbolic() {
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                write(fd(crate::sys::STDOUT_FILENO), bytes(b"u=rwx,g=rx,o=rx\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"umask".to_vec(), b"-S".to_vec()]).expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn umask_double_dash_separator() {
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                umask(_) -> 0o77,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"umask".to_vec(), b"--".to_vec(), b"077".to_vec()],
                )
                .expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn umask_invalid_option() {
        let msg = diag(b"umask: invalid option: -x");
        run_trace(
            trace_entries![
                write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"umask".to_vec(), b"-x".to_vec()]).expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn umask_too_many_arguments() {
        let msg = diag(b"umask: too many arguments");
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome = invoke(
                    &mut shell,
                    &[b"umask".to_vec(), b"022".to_vec(), b"033".to_vec()],
                )
                .expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn umask_invalid_mask() {
        let msg = diag(b"umask: invalid mask: abc");
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"umask".to_vec(), b"abc".to_vec()]).expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn umask_set_octal() {
        run_trace(
            trace_entries![
                umask(_) -> 0o22,
                umask(_) -> 0o22,
                umask(_) -> 0o77,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"umask".to_vec(), b"077".to_vec()]).expect("umask");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn parse_symbolic_empty_clause_returns_none() {
        assert_no_syscalls(|| {
            assert_eq!(parse_symbolic_umask(b"u=rw,", 0o022), None);
            assert_eq!(parse_symbolic_umask(b",u=rw", 0o022), None);
        });
    }

    #[test]
    fn parse_symbolic_clause_no_operator() {
        assert_no_syscalls(|| {
            assert_eq!(parse_symbolic_clause(b"u"), None);
            assert_eq!(parse_symbolic_clause(b"ugo"), None);
        });
    }

    #[test]
    fn parse_symbolic_clause_invalid_operator() {
        assert_no_syscalls(|| {
            assert_eq!(parse_symbolic_clause(b"u!rw"), None);
            assert_eq!(parse_symbolic_clause(b"g?x"), None);
        });
    }

    #[test]
    fn parse_symbolic_targets_empty_defaults_to_all() {
        assert_no_syscalls(|| {
            assert_eq!(parse_symbolic_targets(b""), 0o777);
        });
    }

    #[test]
    fn symbolic_permission_s_is_noop() {
        assert_no_syscalls(|| {
            let result = symbolic_permission_bits(b"s", 0o700, 0o755);
            assert_eq!(result, Some(0));
        });
    }

    #[test]
    fn symbolic_permission_unknown_char_returns_none() {
        assert_no_syscalls(|| {
            assert_eq!(symbolic_permission_bits(b"z", 0o700, 0o755), None);
            assert_eq!(symbolic_permission_bits(b"rz", 0o700, 0o755), None);
        });
    }

    #[test]
    fn symbolic_permission_copy_from_user() {
        assert_no_syscalls(|| {
            let result = symbolic_permission_bits(b"u", 0o070, 0o754);
            assert_eq!(result, Some(0o070));
        });
    }

    #[test]
    fn copy_permission_bits_from_user() {
        assert_no_syscalls(|| {
            assert_eq!(copy_permission_bits(0o754, 0o070, 0o700), 0o070);
            assert_eq!(copy_permission_bits(0o754, 0o007, 0o700), 0o007);
            assert_eq!(copy_permission_bits(0o754, 0o777, 0o700), 0o777);
        });
    }

    #[test]
    fn symbolic_mask_with_implicit_all_targets() {
        assert_no_syscalls(|| {
            let result = parse_umask_mask(b"+x", 0o777);
            assert_eq!(result, Some(0o666));
        });
    }

    #[test]
    fn symbolic_mask_invalid_perm_char() {
        assert_no_syscalls(|| {
            assert_eq!(parse_umask_mask(b"u=z", 0o022), None);
        });
    }
}
