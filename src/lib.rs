pub mod builtin;
pub mod exec;
pub mod expand;
pub mod interactive;
pub mod shell;
pub mod syntax;
pub mod sys;

pub use shell::run_from_env;

#[cfg(test)]
pub(crate) mod test_utils {
    use std::sync::{Mutex, OnceLock};
    use std::path::PathBuf;

    pub(crate) fn meiksh_bin_path() -> PathBuf {
        let exe = std::env::current_exe().expect("current exe");
        exe.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("meiksh"))
            .expect("meiksh path")
    }

    pub(crate) fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
