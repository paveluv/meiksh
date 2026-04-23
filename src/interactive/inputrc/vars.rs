//! Recognized inputrc variables (spec § 5).

#![allow(dead_code)]

/// Typed snapshot of the inputrc variables meiksh honors. Defaults
/// match spec § 5.1.
///
/// `input_meta` and `output_meta` are accepted by the parser so that
/// shipped `/etc/inputrc` files load without spurious "unknown
/// variable" warnings, but meiksh's editor does not act on them: meta
/// is only recognized in its ESC-prefix form (spec § 4). They are
/// stored here so that future work (see Appendix B, Package 12 of
/// [emacs-editing-mode.md](../../../docs/features/emacs-editing-mode.md))
/// can tie them to real behavior without another parser change.
#[derive(Clone, Debug)]
pub(crate) struct InputrcVars {
    pub bell_style: BellStyle,
    pub completion_ignore_case: bool,
    pub completion_map_case: bool,
    pub show_all_if_ambiguous: bool,
    pub show_all_if_unmodified: bool,
    pub enable_bracketed_paste: bool,
    pub editing_mode: EditingMode,
    pub history_size: u32,
    pub mark_symlinked_directories: bool,
    pub colored_stats: bool,
    pub keyseq_timeout_ms: u32,
    pub comment_begin: Vec<u8>,
    pub input_meta: bool,
    pub output_meta: bool,
}

impl Default for InputrcVars {
    fn default() -> Self {
        Self {
            bell_style: BellStyle::Audible,
            completion_ignore_case: false,
            completion_map_case: false,
            show_all_if_ambiguous: false,
            show_all_if_unmodified: false,
            enable_bracketed_paste: true,
            editing_mode: EditingMode::Emacs,
            history_size: 500,
            mark_symlinked_directories: false,
            colored_stats: false,
            keyseq_timeout_ms: 500,
            comment_begin: b"#".to_vec(),
            // Readline defaults: both off. Meiksh ignores these
            // values at runtime; see the struct-level docstring.
            input_meta: false,
            output_meta: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BellStyle {
    None,
    Audible,
    Visible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EditingMode {
    Emacs,
    Vi,
}

/// Parse `<name> <value>` from the line after the leading `set `.
pub(crate) fn parse_assignment(line: &[u8], vars: &mut InputrcVars) -> Result<(), String> {
    let (name, rest) = split_name_value(line);
    if name.is_empty() {
        return Err("expected variable name after `set`".to_string());
    }
    let val = rest.unwrap_or(b"");
    match name {
        b"bell-style" => {
            vars.bell_style = match val {
                b"none" => BellStyle::None,
                b"audible" => BellStyle::Audible,
                b"visible" => BellStyle::Visible,
                other => {
                    return Err(format!(
                        "invalid value for bell-style: {}",
                        String::from_utf8_lossy(other)
                    ));
                }
            };
            Ok(())
        }
        b"completion-ignore-case" => set_bool(val, &mut vars.completion_ignore_case, name),
        b"completion-map-case" => set_bool(val, &mut vars.completion_map_case, name),
        b"show-all-if-ambiguous" => set_bool(val, &mut vars.show_all_if_ambiguous, name),
        b"show-all-if-unmodified" => set_bool(val, &mut vars.show_all_if_unmodified, name),
        b"enable-bracketed-paste" => set_bool(val, &mut vars.enable_bracketed_paste, name),
        b"mark-symlinked-directories" => set_bool(val, &mut vars.mark_symlinked_directories, name),
        b"colored-stats" => set_bool(val, &mut vars.colored_stats, name),
        b"editing-mode" => {
            vars.editing_mode = match val {
                b"emacs" => EditingMode::Emacs,
                b"vi" => EditingMode::Vi,
                other => {
                    return Err(format!(
                        "invalid editing-mode: {}",
                        String::from_utf8_lossy(other)
                    ));
                }
            };
            Ok(())
        }
        b"history-size" => set_u32(val, &mut vars.history_size, name),
        b"keyseq-timeout" => set_u32(val, &mut vars.keyseq_timeout_ms, name),
        b"comment-begin" => {
            vars.comment_begin = val.to_vec();
            Ok(())
        }
        // `input-meta` and `output-meta` are accepted for inputrc
        // compatibility. Meiksh only recognizes meta in its
        // ESC-prefix form, so these values are stored but currently
        // have no runtime effect.
        b"input-meta" | b"meta-flag" => set_bool(val, &mut vars.input_meta, name),
        b"output-meta" => set_bool(val, &mut vars.output_meta, name),
        other => Err(format!(
            "unknown variable: {}",
            String::from_utf8_lossy(other)
        )),
    }
}

fn split_name_value(line: &[u8]) -> (&[u8], Option<&[u8]>) {
    let mut i = 0;
    while i < line.len() && !matches!(line[i], b' ' | b'\t') {
        i += 1;
    }
    let name = &line[..i];
    while i < line.len() && matches!(line[i], b' ' | b'\t') {
        i += 1;
    }
    let rest = &line[i..];
    let end = {
        let mut e = rest.len();
        while e > 0 && matches!(rest[e - 1], b' ' | b'\t') {
            e -= 1;
        }
        e
    };
    if end == 0 {
        (name, None)
    } else {
        (name, Some(&rest[..end]))
    }
}

fn set_bool(val: &[u8], slot: &mut bool, name: &[u8]) -> Result<(), String> {
    match val.to_ascii_lowercase().as_slice() {
        b"on" | b"true" | b"yes" | b"1" => {
            *slot = true;
            Ok(())
        }
        b"off" | b"false" | b"no" | b"0" => {
            *slot = false;
            Ok(())
        }
        other => Err(format!(
            "invalid boolean for {}: {}",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(other)
        )),
    }
}

fn set_u32(val: &[u8], slot: &mut u32, name: &[u8]) -> Result<(), String> {
    let s = std::str::from_utf8(val).map_err(|_| {
        format!(
            "invalid integer for {}: non-utf8",
            String::from_utf8_lossy(name)
        )
    })?;
    let n: u32 = s.trim().parse().map_err(|_| {
        format!(
            "invalid integer for {}: {}",
            String::from_utf8_lossy(name),
            s
        )
    })?;
    *slot = n;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn bool_values_accepted() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"completion-ignore-case on", &mut v).unwrap();
            assert!(v.completion_ignore_case);
            parse_assignment(b"completion-ignore-case off", &mut v).unwrap();
            assert!(!v.completion_ignore_case);
            parse_assignment(b"completion-ignore-case yes", &mut v).unwrap();
            assert!(v.completion_ignore_case);
        });
    }

    #[test]
    fn bell_style_enum() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"bell-style none", &mut v).unwrap();
            assert_eq!(v.bell_style, BellStyle::None);
        });
    }

    #[test]
    fn unknown_variable_returns_error() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            let err = parse_assignment(b"whatever 1", &mut v).unwrap_err();
            assert!(err.contains("unknown variable"));
        });
    }

    #[test]
    fn integer_value_parsed() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"history-size 1000", &mut v).unwrap();
            assert_eq!(v.history_size, 1000);
        });
    }

    #[test]
    fn comment_begin_takes_literal_string() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"comment-begin //", &mut v).unwrap();
            assert_eq!(v.comment_begin, b"//");
        });
    }

    #[test]
    fn input_meta_and_meta_flag_accepted() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"input-meta on", &mut v).unwrap();
            assert!(v.input_meta);
            // `meta-flag` is a readline alias for `input-meta`.
            let mut v2 = InputrcVars::default();
            parse_assignment(b"meta-flag yes", &mut v2).unwrap();
            assert!(v2.input_meta);
            parse_assignment(b"input-meta off", &mut v2).unwrap();
            assert!(!v2.input_meta);
        });
    }

    #[test]
    fn output_meta_accepted() {
        assert_no_syscalls(|| {
            let mut v = InputrcVars::default();
            parse_assignment(b"output-meta on", &mut v).unwrap();
            assert!(v.output_meta);
            parse_assignment(b"output-meta off", &mut v).unwrap();
            assert!(!v.output_meta);
        });
    }
}
