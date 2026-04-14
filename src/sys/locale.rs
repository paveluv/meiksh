use super::interface::sys_interface;

pub fn setup_locale() {
    (sys_interface().setup_locale)()
}

pub fn classify_byte(class: &[u8], byte: u8) -> bool {
    (sys_interface().classify_byte)(class, byte)
}

#[cfg(test)]
mod tests {
    use libc::{c_char, c_int, c_long, mode_t};
    use std::collections::HashMap;
    use std::ffi::CString;

    use crate::sys::test_support;
    use crate::sys::types::ClockTicks;

    use super::*;
    use crate::sys::*;

    #[test]
    fn trace_setup_locale_is_noop() {
        test_support::run_trace(vec![], || {
            setup_locale();
        });
    }
}
