use super::*;

pub(super) fn echo_builtin(_shell: &Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let mut out: Vec<u8> = Vec::new();
    for (i, arg) in argv[1..].iter().enumerate() {
        if i > 0 {
            out.push(b' ');
        }
        out.extend_from_slice(arg);
    }
    out.push(b'\n');
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, &out);
    Ok(BuiltinOutcome::Status(0))
}

// ---------------------------------------------------------------------------
// printf builtin
// ---------------------------------------------------------------------------
