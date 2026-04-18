use std::sync::atomic::{AtomicUsize, Ordering};

use super::interface::sys_interface;

/// Cached `MB_CUR_MAX` for the currently installed locale. A value of `0`
/// is a sentinel meaning "not yet initialized"; valid values are `>= 1`.
///
/// The cache is refreshed on every `setup_locale` / `reinit_locale` so that
/// the ASCII-fast-path decision in `decode_char` / `count_chars` /
/// `first_char_len` remains correct across locale changes. In tests that do
/// not install a `SystemInterface`, the cache is bypassed entirely and the
/// functions fall back to single-byte behaviour (matching the pre-existing
/// no-interface fallback path).
static MB_CUR_MAX_CACHE: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn cache_mb_cur_max(value: usize) {
    let v = if value == 0 { 1 } else { value };
    MB_CUR_MAX_CACHE.store(v, Ordering::Relaxed);
}

/// Returns the cached `MB_CUR_MAX`, populating it on first use.
///
/// In `#[cfg(test)]`, if no `SystemInterface` is installed we return `1`
/// without touching the cache, so that functions like `count_chars` remain
/// usable in pure-logic tests (`assert_no_syscalls`) and honour
/// `set_test_locale_*` changes.
#[cfg(not(test))]
#[inline]
fn mb_cur_max_cached() -> usize {
    let v = MB_CUR_MAX_CACHE.load(Ordering::Relaxed);
    if v != 0 {
        return v;
    }
    let v = (sys_interface().mb_cur_max)();
    let v = if v == 0 { 1 } else { v };
    MB_CUR_MAX_CACHE.store(v, Ordering::Relaxed);
    v
}

#[cfg(test)]
#[inline]
fn mb_cur_max_cached() -> usize {
    if super::test_support::current_interface().is_none() {
        return 1;
    }
    // In tests the dispatch goes to `test_mb_cur_max`, which is cheap and
    // reflects live `set_test_locale_*` changes. Skipping the cache here
    // avoids stale reads when tests flip locale mid-run.
    (sys_interface().mb_cur_max)()
}

pub(crate) fn setup_locale() {
    (sys_interface().setup_locale)();
    let v = (sys_interface().mb_cur_max)();
    cache_mb_cur_max(v);
}

pub(crate) fn reinit_locale() {
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            return;
        }
    }
    (sys_interface().reinit_locale)();
    let v = (sys_interface().mb_cur_max)();
    cache_mb_cur_max(v);
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

/// Decode the first character of `bytes` as (wide char, byte length).
///
/// Caller contract: `bytes` is positioned at a character boundary — either
/// the start of a buffer, or at an offset previously returned by this
/// function. All internal callers satisfy this.
///
/// ASCII fast path: POSIX-conforming locales have ASCII as an invariant
/// encoding (the portable character set in POSIX.1-2017 2.2 maps each
/// ASCII byte to itself as a single-byte character in every supported
/// locale), so any `bytes[0] < 0x80` is unambiguously a single-byte
/// character. Likewise, a locale with `MB_CUR_MAX == 1` is single-byte by
/// definition, so the whole decode reduces to `(bytes[0] as u32, 1)`.
/// Both shortcuts avoid the `mbrtowc` FFI and — in production — the
/// `sys_interface()` indirect call that would otherwise dominate the
/// profile for ASCII-heavy inputs under `LC_ALL=C.UTF-8`.
pub(crate) fn decode_char(bytes: &[u8]) -> (u32, usize) {
    if bytes.is_empty() {
        return (0, 0);
    }
    let b0 = bytes[0];
    if b0 < 0x80 {
        return (b0 as u32, 1);
    }
    if mb_cur_max_cached() == 1 {
        return (b0 as u32, 1);
    }
    #[cfg(test)]
    {
        if super::test_support::current_interface().is_none() {
            return (b0 as u32, 1);
        }
    }
    (sys_interface().decode_char)(bytes)
}

pub(crate) fn encode_char(wc: u32) -> Vec<u8> {
    let mut buf = [0u8; 8];
    let n = (sys_interface().encode_char)(wc, &mut buf);
    buf[..n].to_vec()
}

/// Count multibyte characters in `bytes`, stopping at the first NUL byte.
///
/// Fast path: scan ASCII bytes (`<0x80`) in a tight loop without touching
/// `decode_char` / `sys_interface`. Only the first non-ASCII byte requires
/// a real decode — and even there, `decode_char` itself uses the cached
/// `MB_CUR_MAX` to short-circuit single-byte locales.
pub(crate) fn count_chars(bytes: &[u8]) -> u64 {
    let mut count = 0u64;
    let mut i = 0;
    let len = bytes.len();
    while i < len {
        let b = bytes[i];
        if b == 0 {
            break;
        }
        if b < 0x80 {
            count += 1;
            i += 1;
            continue;
        }
        let (_, clen) = decode_char(&bytes[i..]);
        if clen == 0 {
            break;
        }
        count += 1;
        i += clen;
    }
    count
}

pub(crate) fn first_char_len(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    if bytes[0] < 0x80 {
        return 1;
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

    /// Exercises the pure-ASCII hot path: no test interface is installed,
    /// so any call that reached `sys_interface()` would panic. Success
    /// means `count_chars` and `first_char_len` answered the ASCII bytes
    /// without dispatching at all.
    #[test]
    fn ascii_fast_path_avoids_dispatch() {
        assert_no_syscalls(|| {
            assert_eq!(count_chars(b"hello, world!"), 13);
            assert_eq!(first_char_len(b"h"), 1);
            assert_eq!(first_char_len(b"hello"), 1);
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
