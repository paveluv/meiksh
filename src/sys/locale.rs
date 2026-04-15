use super::interface::sys_interface;

pub(crate) fn setup_locale() {
    (sys_interface().setup_locale)()
}

pub(crate) fn classify_byte(class: &[u8], byte: u8) -> bool {
    (sys_interface().classify_byte)(class, byte)
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
