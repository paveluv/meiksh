use super::interface::sys_interface;

pub(crate) fn setup_locale() {
    (sys_interface().setup_locale)()
}

pub(crate) fn reinit_locale() {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            return;
        }
    }
    (sys_interface().reinit_locale)()
}

#[cfg(test)]
pub(crate) fn classify_byte(class: &[u8], byte: u8) -> bool {
    (sys_interface().classify_byte)(class, byte)
}

pub(crate) fn classify_char(class: &[u8], wc: u32) -> bool {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            if wc > 0x7f {
                return false;
            }
            return classify_byte(class, wc as u8);
        }
    }
    (sys_interface().classify_char)(class, wc)
}

pub(crate) fn decode_char(bytes: &[u8]) -> (u32, usize) {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            if bytes.is_empty() {
                return (0, 0);
            }
            return (bytes[0] as u32, 1);
        }
    }
    (sys_interface().decode_char)(bytes)
}

pub(crate) fn encode_char(wc: u32) -> Vec<u8> {
    let mut buf = [0u8; 8];
    let n = (sys_interface().encode_char)(wc, &mut buf);
    buf[..n].to_vec()
}

pub(crate) fn count_chars(bytes: &[u8]) -> u64 {
    let mut count = 0u64;
    let mut i = 0;
    while i < bytes.len() {
        let (_, len) = decode_char(&bytes[i..]);
        if len == 0 {
            break;
        }
        count += 1;
        i += len;
    }
    count
}

pub(crate) fn first_char_len(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let (_, len) = decode_char(bytes);
    if len == 0 { 1 } else { len }
}

#[cfg(test)]
pub(crate) fn mb_cur_max() -> usize {
    (sys_interface().mb_cur_max)()
}

pub(crate) fn to_upper(wc: u32) -> u32 {
    (sys_interface().to_upper)(wc)
}

pub(crate) fn to_lower(wc: u32) -> u32 {
    (sys_interface().to_lower)(wc)
}

pub(crate) fn char_width(wc: u32) -> usize {
    (sys_interface().char_width)(wc)
}

pub(crate) fn strcoll(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            return a.cmp(b);
        }
    }
    (sys_interface().strcoll)(a, b)
}

pub(crate) fn decimal_point() -> u8 {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            return b'.';
        }
    }
    (sys_interface().decimal_point)()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{
        self, ArgMatcher, TraceResult, assert_no_syscalls, set_test_locale_c, set_test_locale_utf8,
        t,
    };
    use crate::trace_entries;

    #[test]
    fn trace_setup_locale_is_noop() {
        test_support::run_trace(trace_entries![], || {
            setup_locale();
        });
    }

    #[test]
    fn count_chars_c_vs_utf8() {
        assert_no_syscalls(|| {
            // U+00E9 = 0xC3 0xA9
            set_test_locale_c();
            assert_eq!(count_chars(b"\xc3\xa9"), 2);

            set_test_locale_utf8();
            assert_eq!(count_chars(b"\xc3\xa9"), 1);
        });
    }

    #[test]
    fn count_chars_stops_at_nul() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(count_chars(b"ab\x00cd"), 2);
        });
    }

    #[test]
    fn first_char_len_c_vs_utf8() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(first_char_len(b"\xc3\xa9"), 1);

            set_test_locale_utf8();
            assert_eq!(first_char_len(b"\xc3\xa9"), 2);
        });
    }

    #[test]
    fn first_char_len_empty() {
        assert_no_syscalls(|| {
            assert_eq!(first_char_len(b""), 0);
        });
    }

    #[test]
    fn mb_cur_max_c_vs_utf8() {
        assert_no_syscalls(|| {
            set_test_locale_c();
            assert_eq!(mb_cur_max(), 1);

            set_test_locale_utf8();
            assert_eq!(mb_cur_max(), 4);
        });
    }

    fn getenv_entry(key: &[u8], result: TraceResult) -> test_support::TraceEntry {
        t("getenv", vec![ArgMatcher::Str(key.to_vec())], result)
    }

    #[test]
    fn reinit_locale_reads_lc_all_utf8() {
        test_support::run_trace(
            trace_entries![
                ..vec![getenv_entry(
                    b"LC_ALL",
                    TraceResult::StrVal(b"C.UTF-8".to_vec()),
                )],
            ],
            || {
                reinit_locale();
                assert_eq!(mb_cur_max(), 4);
            },
        );
    }

    #[test]
    fn reinit_locale_reads_lc_all_c() {
        test_support::run_trace(
            trace_entries![..vec![getenv_entry(b"LC_ALL", TraceResult::StrVal(b"C".to_vec()))],],
            || {
                reinit_locale();
                assert_eq!(mb_cur_max(), 1);
            },
        );
    }

    #[test]
    fn reinit_locale_falls_back_to_lang() {
        test_support::run_trace(
            trace_entries![
                ..vec![
                    getenv_entry(b"LC_ALL", TraceResult::NullStr),
                    getenv_entry(b"LANG", TraceResult::StrVal(b"en_US.UTF8".to_vec())),
                ],
            ],
            || {
                reinit_locale();
                assert_eq!(mb_cur_max(), 4);
            },
        );
    }

    #[test]
    fn reinit_locale_defaults_to_c_when_unset() {
        test_support::run_trace(
            trace_entries![
                ..vec![
                    getenv_entry(b"LC_ALL", TraceResult::NullStr),
                    getenv_entry(b"LANG", TraceResult::NullStr),
                ],
            ],
            || {
                reinit_locale();
                assert_eq!(mb_cur_max(), 1);
            },
        );
    }
}
