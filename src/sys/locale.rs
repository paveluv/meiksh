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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

/// Iterate over multi-byte character boundaries in `bytes`.
/// Yields the byte offset of each character start (excluding 0).
#[allow(dead_code)]
pub(crate) fn char_boundaries(bytes: &[u8]) -> Vec<usize> {
    let mut boundaries = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let (_, len) = decode_char(&bytes[i..]);
        let step = if len == 0 { 1 } else { len };
        i += step;
        if i <= bytes.len() {
            boundaries.push(i);
        }
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support;
    use crate::trace_entries;

    #[test]
    fn trace_setup_locale_is_noop() {
        test_support::run_trace(trace_entries![], || {
            setup_locale();
        });
    }
}
