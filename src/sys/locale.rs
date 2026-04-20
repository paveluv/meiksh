use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(test))]
use libc::c_int;

/// Cached `MB_CUR_MAX` for the currently installed locale. A value of `0`
/// is a sentinel meaning "not yet initialized"; valid values are `>= 1`.
///
/// The cache is refreshed on every `setup_locale` / `reinit_locale` so that
/// the ASCII-fast-path decision in `decode_char` / `count_chars` /
/// `first_char_len` remains correct across locale changes. In `#[cfg(test)]`
/// the cache is bypassed entirely so `set_test_locale_*` flips take effect
/// immediately.
static MB_CUR_MAX_CACHE: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn cache_mb_cur_max(value: usize) {
    let v = if value == 0 { 1 } else { value };
    MB_CUR_MAX_CACHE.store(v, Ordering::Relaxed);
}

/// Returns the cached `MB_CUR_MAX`, populating it on first use.
///
/// In `#[cfg(test)]` we bypass the cache entirely so that `set_test_locale_*`
/// flips take effect immediately and pure-logic tests (`assert_no_syscalls`)
/// remain usable.
#[cfg(not(test))]
#[inline]
fn mb_cur_max_cached() -> usize {
    // `setup_locale` (called once at startup from `Shell::from_env`)
    // primes the cache with a non-zero value, and every later
    // `reinit_locale` refreshes it. Callers never observe 0 here.
    MB_CUR_MAX_CACHE.load(Ordering::Relaxed)
}

#[cfg(test)]
#[inline]
fn mb_cur_max_cached() -> usize {
    // In tests the implementation is cheap and reflects live
    // `set_test_locale_*` changes. Skipping the cache here avoids stale
    // reads when tests flip locale mid-run.
    mb_cur_max()
}

// ------------------------------------------------------------------
// Production libc helpers. Kept module-local so callers never see
// `libc::` directly; the `sys::` boundary guard test in `sys::mod`
// depends on this.
// ------------------------------------------------------------------

#[cfg(not(test))]
fn libc_mb_cur_max() -> usize {
    #[cfg(target_os = "linux")]
    {
        unsafe extern "C" {
            fn __ctype_get_mb_cur_max() -> usize;
        }
        unsafe { __ctype_get_mb_cur_max() }
    }
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            static __mb_cur_max: c_int;
        }
        unsafe { __mb_cur_max as usize }
    }
    #[cfg(target_os = "freebsd")]
    {
        // On FreeBSD the public `MB_CUR_MAX` macro expands to
        // `((size_t)___mb_cur_max())` (three leading underscores). The
        // two-underscore `__mb_cur_max` symbol is an `extern int` data
        // object, not a function, so calling it as a function jumps
        // into the bytes of that `int` and segfaults. Always go
        // through the triple-underscore function accessor here.
        unsafe extern "C" {
            fn ___mb_cur_max() -> c_int;
        }
        unsafe { ___mb_cur_max() as usize }
    }
}

#[cfg(not(test))]
fn classify_wchar_wctype(class: &[u8], wc: u32) -> bool {
    unsafe extern "C" {
        fn wctype(name: *const libc::c_char) -> usize;
        fn iswctype(wc: u32, desc: usize) -> c_int;
    }
    let c_class = crate::bstr::to_cstring(class).unwrap_or_default();
    let desc = unsafe { wctype(c_class.as_ptr()) };
    if desc == 0 {
        false
    } else {
        unsafe { iswctype(wc, desc) != 0 }
    }
}

#[cfg(not(test))]
fn classify_wchar(class: &[u8], wc: u32) -> bool {
    unsafe extern "C" {
        fn iswalnum(wc: u32) -> c_int;
        fn iswalpha(wc: u32) -> c_int;
        fn iswblank(wc: u32) -> c_int;
        fn iswcntrl(wc: u32) -> c_int;
        fn iswdigit(wc: u32) -> c_int;
        fn iswgraph(wc: u32) -> c_int;
        fn iswlower(wc: u32) -> c_int;
        fn iswprint(wc: u32) -> c_int;
        fn iswpunct(wc: u32) -> c_int;
        fn iswspace(wc: u32) -> c_int;
        fn iswupper(wc: u32) -> c_int;
        fn iswxdigit(wc: u32) -> c_int;
    }
    unsafe {
        match class {
            b"alnum" => iswalnum(wc) != 0,
            b"alpha" => iswalpha(wc) != 0,
            b"blank" => iswblank(wc) != 0,
            b"cntrl" => iswcntrl(wc) != 0,
            b"digit" => iswdigit(wc) != 0,
            b"graph" => iswgraph(wc) != 0,
            b"lower" => iswlower(wc) != 0,
            b"print" => iswprint(wc) != 0,
            b"punct" => iswpunct(wc) != 0,
            b"space" => iswspace(wc) != 0,
            b"upper" => iswupper(wc) != 0,
            b"xdigit" => iswxdigit(wc) != 0,
            _ => classify_wchar_wctype(class, wc),
        }
    }
}

// ------------------------------------------------------------------
// Public locale API. Each function is cfg-split: `#[cfg(not(test))]`
// goes straight to libc, `#[cfg(test)]` is a pure-logic fake driven
// by the `TEST_LOCALE` thread-local maintained in `test_support`.
// These functions are not traced — only true syscalls live in the
// `trace_*` tables maintained by `test_support`.
// ------------------------------------------------------------------

#[cfg(not(test))]
pub(crate) fn setup_locale() {
    unsafe {
        libc::setlocale(libc::LC_ALL, b"\0".as_ptr().cast());
    }
    cache_mb_cur_max(libc_mb_cur_max());
}

#[cfg(test)]
pub(crate) fn setup_locale() {
    super::test_support::set_test_locale_c();
    cache_mb_cur_max(mb_cur_max());
}

#[cfg(not(test))]
pub(crate) fn reinit_locale() {
    unsafe {
        libc::setlocale(libc::LC_ALL, b"\0".as_ptr().cast());
    }
    cache_mb_cur_max(libc_mb_cur_max());
}

#[cfg(test)]
pub(crate) fn reinit_locale() {
    // The test fake consults the traced `getenv` so the test author
    // can script the exact lookup sequence via `trace_entries!`.
    let val = super::interface::getenv(b"LC_ALL").or_else(|| super::interface::getenv(b"LANG"));
    let is_utf8 = match val {
        Some(v) => {
            let upper: Vec<u8> = v.iter().map(|b| b.to_ascii_uppercase()).collect();
            upper.windows(5).any(|w| w == b"UTF-8") || upper.windows(4).any(|w| w == b"UTF8")
        }
        None => false,
    };
    if is_utf8 {
        super::test_support::set_test_locale_utf8();
    } else {
        super::test_support::set_test_locale_c();
    }
    cache_mb_cur_max(mb_cur_max());
}

#[cfg(not(test))]
pub(crate) fn classify_char(class: &[u8], wc: u32) -> bool {
    classify_wchar(class, wc)
}

#[cfg(test)]
pub(crate) fn classify_char(class: &[u8], wc: u32) -> bool {
    if wc <= 0x7f {
        let byte = wc as u8;
        return byte.is_ascii_alphabetic() && class == b"alpha"
            || byte.is_ascii_alphanumeric() && class == b"alnum"
            || byte.is_ascii_digit() && class == b"digit"
            || byte.is_ascii_lowercase() && class == b"lower"
            || byte.is_ascii_uppercase() && class == b"upper"
            || (byte == b' ' || byte == b'\t') && class == b"blank"
            || byte.is_ascii_whitespace() && class == b"space"
            || byte.is_ascii_hexdigit() && class == b"xdigit"
            || byte.is_ascii_punctuation() && class == b"punct"
            || byte.is_ascii_graphic() && class == b"graph"
            || (byte.is_ascii_graphic() || byte == b' ') && class == b"print"
            || byte.is_ascii_control() && class == b"cntrl";
    }
    if !super::test_support::test_locale_is_utf8() {
        return false;
    }
    if let Some(ch) = char::from_u32(wc) {
        match class {
            b"alpha" => ch.is_alphabetic(),
            b"alnum" => ch.is_alphanumeric(),
            b"digit" => ch.is_ascii_digit(),
            b"lower" => ch.is_lowercase(),
            b"upper" => ch.is_uppercase(),
            b"blank" => ch == ' ' || ch == '\t',
            b"space" => ch.is_whitespace(),
            b"xdigit" => ch.is_ascii_hexdigit(),
            b"punct" => !ch.is_alphanumeric() && !ch.is_whitespace() && !ch.is_control(),
            b"graph" => !ch.is_whitespace() && !ch.is_control(),
            b"print" => !ch.is_control(),
            b"cntrl" => ch.is_control(),
            _ => false,
        }
    } else {
        false
    }
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
/// Both shortcuts avoid the `mbrtowc` FFI that would otherwise dominate
/// the profile for ASCII-heavy inputs under `LC_ALL=C.UTF-8`.
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
    decode_char_impl(bytes)
}

#[cfg(not(test))]
fn decode_char_impl(bytes: &[u8]) -> (u32, usize) {
    // `decode_char` already short-circuits on `bytes.is_empty()` and on
    // any ASCII lead byte, so here `bytes[0] >= 0x80`. `mbrtowc(3)`
    // returns 0 only when the decoded character is the null wide
    // character, which is unreachable for a non-zero lead byte, so we
    // don't bother with a 0-length arm.
    #[repr(C, align(8))]
    struct MbState([u8; 128]);
    unsafe extern "C" {
        fn mbrtowc(pwc: *mut libc::wchar_t, s: *const u8, n: usize, ps: *mut MbState) -> usize;
    }
    unsafe {
        let mut wc: libc::wchar_t = 0;
        let mut ps: MbState = std::mem::zeroed();
        let n = mbrtowc(&mut wc, bytes.as_ptr(), bytes.len(), &mut ps);
        if n == usize::MAX || n == usize::MAX - 1 {
            (bytes[0] as u32, 1)
        } else {
            (wc as u32, n)
        }
    }
}

#[cfg(test)]
fn decode_char_impl(bytes: &[u8]) -> (u32, usize) {
    if bytes.is_empty() || bytes[0] == 0 {
        return (0, 0);
    }
    if !super::test_support::test_locale_is_utf8() {
        return (bytes[0] as u32, 1);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            if let Some(ch) = s.chars().next() {
                (ch as u32, ch.len_utf8())
            } else {
                (0, 0)
            }
        }
        Err(e) => {
            let valid_up_to = e.valid_up_to();
            if valid_up_to > 0 {
                let s = &bytes[..valid_up_to];
                let ch = std::str::from_utf8(s).unwrap().chars().next().unwrap();
                (ch as u32, ch.len_utf8())
            } else {
                (bytes[0] as u32, 1)
            }
        }
    }
}

#[cfg(not(test))]
pub(crate) fn encode_char(wc: u32) -> Vec<u8> {
    let mut buf = [0u8; 8];
    #[repr(C, align(8))]
    struct MbState([u8; 128]);
    unsafe extern "C" {
        fn wcrtomb(s: *mut u8, wc: libc::wchar_t, ps: *mut MbState) -> usize;
    }
    let n = unsafe {
        let mut ps: MbState = std::mem::zeroed();
        let n = wcrtomb(buf.as_mut_ptr(), wc as libc::wchar_t, &mut ps);
        if n == usize::MAX { 0 } else { n }
    };
    buf[..n].to_vec()
}

#[cfg(test)]
pub(crate) fn encode_char(wc: u32) -> Vec<u8> {
    if !super::test_support::test_locale_is_utf8() {
        if wc <= 0x7f {
            return vec![wc as u8];
        }
        return Vec::new();
    }
    if let Some(ch) = char::from_u32(wc) {
        let mut tmp = [0u8; 4];
        let s = ch.encode_utf8(&mut tmp);
        s.as_bytes().to_vec()
    } else {
        Vec::new()
    }
}

/// Count multibyte characters in `bytes`, stopping at the first NUL byte.
///
/// Fast path: scan ASCII bytes (`<0x80`) in a tight loop without touching
/// `decode_char`. Only the first non-ASCII byte requires a real decode —
/// and even there, `decode_char` itself uses the cached `MB_CUR_MAX` to
/// short-circuit single-byte locales.
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
        // `bytes[i]` is non-zero (checked above) and non-empty, so
        // `decode_char` always returns `clen >= 1`; no degenerate
        // break arm is needed here.
        let (_, clen) = decode_char(&bytes[i..]);
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
    if super::test_support::test_locale_is_utf8() {
        4
    } else {
        1
    }
}

#[cfg(not(test))]
pub(crate) fn to_upper(wc: u32) -> u32 {
    unsafe extern "C" {
        fn towupper(wc: u32) -> u32;
    }
    unsafe { towupper(wc) }
}

#[cfg(test)]
pub(crate) fn to_upper(wc: u32) -> u32 {
    if !super::test_support::test_locale_is_utf8() {
        if wc >= b'a' as u32 && wc <= b'z' as u32 {
            return wc - 32;
        }
        return wc;
    }
    char::from_u32(wc)
        .map(|c| {
            let mut it = c.to_uppercase();
            it.next().unwrap_or(c) as u32
        })
        .unwrap_or(wc)
}

#[cfg(not(test))]
pub(crate) fn to_lower(wc: u32) -> u32 {
    unsafe extern "C" {
        fn towlower(wc: u32) -> u32;
    }
    unsafe { towlower(wc) }
}

#[cfg(test)]
pub(crate) fn to_lower(wc: u32) -> u32 {
    if !super::test_support::test_locale_is_utf8() {
        if wc >= b'A' as u32 && wc <= b'Z' as u32 {
            return wc + 32;
        }
        return wc;
    }
    char::from_u32(wc)
        .map(|c| {
            let mut it = c.to_lowercase();
            it.next().unwrap_or(c) as u32
        })
        .unwrap_or(wc)
}

#[cfg(not(test))]
pub(crate) fn char_width(wc: u32) -> usize {
    unsafe extern "C" {
        fn wcwidth(wc: u32) -> c_int;
    }
    let w = unsafe { wcwidth(wc) };
    if w < 0 { 0 } else { w as usize }
}

#[cfg(test)]
pub(crate) fn char_width(wc: u32) -> usize {
    if !super::test_support::test_locale_is_utf8() {
        if wc < 0x20 || wc == 0x7f { 0 } else { 1 }
    } else if let Some(ch) = char::from_u32(wc) {
        if ch.is_control() { 0 } else { 1 }
    } else {
        0
    }
}

#[cfg(not(test))]
pub(crate) fn strcoll(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let ca = crate::bstr::to_cstring(a).unwrap_or_default();
    let cb = crate::bstr::to_cstring(b).unwrap_or_default();
    let r = unsafe { libc::strcoll(ca.as_ptr(), cb.as_ptr()) };
    r.cmp(&0)
}

#[cfg(test)]
pub(crate) fn strcoll(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    // In pure-logic tests we deliberately bypass libc: the expected
    // ordering is byte-lexicographic, which matches `strcoll` for the
    // ASCII-only test fixtures and keeps results host-locale
    // independent.
    a.cmp(b)
}

#[cfg(not(test))]
pub(crate) fn decimal_point() -> u8 {
    let dp = unsafe { *(*libc::localeconv()).decimal_point };
    if dp == 0 { b'.' } else { dp as u8 }
}

#[cfg(test)]
pub(crate) fn decimal_point() -> u8 {
    b'.'
}

/// Sort `entries` in the current locale's collating sequence, without
/// allocating on every comparison.
///
/// `readdir` already hands back NUL-terminated `d_name` buffers, which we
/// keep in `CString` form end-to-end through the glob pipeline. That lets
/// the comparator call `strcoll(3)` directly on the prebuilt C strings.
/// glibc's own `__strcoll_l` has a fast path that degrades to `strcmp` in
/// C-category locales (including `C.UTF-8`), so we do not need a separate
/// bytewise short-circuit here — the libc call is already cheap, as long
/// as we are not wrapping it in a fresh `CString` on every invocation.
///
/// Under `cfg(test)` we bypass libc entirely to keep the unit-test suite
/// deterministic and host-locale-independent; the byte order is identical
/// to `strcoll` for all ASCII test fixtures.
#[inline]
pub(crate) fn sort_cstrings(entries: &mut [CString]) {
    #[cfg(not(test))]
    {
        entries.sort_by(|a, b| {
            let r = unsafe { libc::strcoll(a.as_ptr(), b.as_ptr()) };
            r.cmp(&0)
        });
    }
    #[cfg(test)]
    {
        entries.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    }
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

    /// Exercises the pure-ASCII hot path under `assert_no_syscalls`: any
    /// syscall that escaped to a trace dispatcher would panic. Success
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
