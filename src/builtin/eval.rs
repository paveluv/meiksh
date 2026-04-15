use super::BuiltinOutcome;
use crate::bstr;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;

pub(super) fn eval(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let source = bstr::join_bstrings(&argv[1..].to_vec(), b" ");
    let status = shell.execute_string(&source)?;
    Ok(BuiltinOutcome::Status(status))
}
