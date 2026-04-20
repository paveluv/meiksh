pub(crate) mod constants;
pub(crate) mod env;
pub(crate) mod error;
pub(crate) mod fd_io;
pub(crate) mod fs;
pub(super) mod interface;
pub(crate) mod locale;
pub mod process;
pub(crate) mod time;
pub(crate) mod tty;
pub(crate) mod types;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]
mod boundary_tests {
    //! Enforce that `libc::` is only referenced inside `src/sys/`.
    //!
    //! All other crates must go through the thin wrappers in `sys::*`
    //! (constants, types, and helper modules). This keeps the
    //! libc/syscall dependency surface auditable in one place, and
    //! forces new code to either reuse an existing `sys` wrapper or
    //! add one deliberately.
    //!
    //! This auditor intentionally uses `std::fs`, `std::path::Path`
    //! and `format!` directly: it must stay independent of the very
    //! wrappers it is policing, so the `disallowed_*` clippy lints
    //! that apply to production code are silenced at the module
    //! boundary.

    use std::fs;
    use std::path::{Path, PathBuf};

    fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if path.is_dir() {
                if file_name == "sys" || file_name == "target" {
                    continue;
                }
                collect_rust_files(&path, out);
            } else if file_name.ends_with(".rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn libc_is_only_used_inside_sys_module() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let src_root = Path::new(manifest_dir).join("src");
        let mut files = Vec::new();
        collect_rust_files(&src_root, &mut files);

        let mut offenders: Vec<(PathBuf, usize, String)> = Vec::new();
        for file in files {
            let content = match fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                    continue;
                }
                if line.contains("libc::") {
                    offenders.push((file.clone(), idx + 1, line.to_string()));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "`libc::` must only be referenced inside src/sys/; offenders:\n{}",
            offenders
                .iter()
                .map(|(p, l, s)| format!("  {}:{}: {}", p.display(), l, s.trim()))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}
