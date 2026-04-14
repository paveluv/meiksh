use crate::arena::ByteArena;
use crate::expand;
use crate::shell::{Shell, ShellError};
use crate::sys;

pub(super) fn load_env_file(shell: &mut Shell) -> Result<(), ShellError> {
    if !sys::has_same_real_and_effective_ids() {
        return Ok(());
    }
    let env_value = shell.get_var(b"ENV").map(|s| s.to_vec());
    let arena = ByteArena::new();
    let env_file = env_value
        .map(|value| expand::expand_parameter_text(shell, &value, &arena).map(|s| s.to_vec()))
        .transpose()
        .map_err(|e| shell.expand_to_err(e))?;
    if let Some(path) = env_file {
        let is_absolute = !path.is_empty() && path[0] == b'/';
        if is_absolute && sys::file_exists(&path) {
            let _ = shell.source_path(&path)?;
        }
    }
    Ok(())
}
