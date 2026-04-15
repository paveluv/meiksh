use crate::bstr::ByteWriter;

#[derive(Debug)]
pub(crate) enum ShellError {
    Status(i32),
}

impl ShellError {
    pub(crate) fn exit_status(&self) -> i32 {
        let ShellError::Status(s) = self;
        *s
    }

    pub(crate) fn message_bytes(&self) -> Vec<u8> {
        crate::bstr::ByteWriter::new()
            .bytes(b"exit status ")
            .i64_val(self.exit_status() as i64)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) enum VarError {
    Readonly(Box<[u8]>),
}

pub(super) fn var_error_message(e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(name)
            .bytes(b": readonly variable")
            .finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::expand::core::ExpandError;
    use crate::syntax;
    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    use crate::shell::test_support::{t_stderr, test_shell};

    #[test]
    fn shell_error_converts_from_parse_and_expand_errors() {
        run_trace(
            trace_entries![
                ..vec![
                    t_stderr("meiksh: line 1: unterminated single quote"),
                    t_stderr("meiksh: expand"),
                ],
            ],
            || {
                let shell = test_shell();
                let parse_err = syntax::parse(b"echo 'unterminated").expect_err("parse");
                let shell_err = shell.parse_to_err(parse_err);
                assert_eq!(shell_err.exit_status(), 2);

                let expand_err = shell.expand_to_err(ExpandError {
                    message: (*b"expand").into(),
                });
                assert_eq!(expand_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn shell_error_status_helpers_work() {
        run_trace(
            trace_entries![..vec![t_stderr("meiksh: missing script")]],
            || {
                let shell = test_shell();
                let error = shell.diagnostic(127, b"missing script");
                assert_eq!(error.exit_status(), 127);

                let silent = ShellError::Status(42);
                assert_eq!(silent.exit_status(), 42);
            },
        );
    }

    #[test]
    fn shell_error_message_bytes_and_exit_status() {
        let err = ShellError::Status(42);
        assert_eq!(err.message_bytes(), b"exit status 42");
        assert_eq!(err.exit_status(), 42);
        assert!(err.message_bytes().windows(2).any(|w| w == b"42"));
    }
}
