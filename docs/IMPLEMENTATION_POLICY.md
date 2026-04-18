# Meiksh Implementation Policy

This document records project rules, POSIX implementation-defined choices, and temporary decisions that apply while the shell is under active development.

## Project Constraints

- Language: Rust
- Dependency policy: keep dependencies minimal; low-level POSIX interface access lives in `src/sys/`
- FFI boundary policy: `libc` is permitted only in `src/sys/`, with a narrow documented exception for `tests/integration/basic.rs`; all other modules must go through shell-owned helpers exposed from that layer instead of importing `libc` directly
- Portability policy: do not introduce `#[cfg(target_os = ...)]` switches as a normal implementation technique; platform differences should be absorbed through POSIX-facing helpers in `src/sys/`, preferably by relying on `libc`-provided types and constants rather than open-coding per-OS values
- Source policy: no reuse of existing shell source code
- Semantic target: Issue 8 first, with Issue 7 compatibility notes when needed for documentation review
- Conformance policy: POSIX behavior decisions must be based on the local POSIX reference documents in `docs/posix/`, not on probing whatever `/bin/sh` happens to do on the host system

## Low-Level Interface Boundary

- `src/sys/` is the only production module that may depend on `libc` directly.
- `tests/integration/basic.rs` may also depend on `libc` for test-only setup of inherited file-descriptor state where using `src/sys/` helpers is not practical inside `pre_exec`.
- Code outside `src/sys/` should express OS needs in terms of shell-owned helper functions, data types, and constants from `src/sys/`.
- If a required interface or constant is missing, extend `src/sys/` instead of importing `libc` elsewhere.
- New platform-specific `target_os` branching is not an acceptable default approach for production code or tests.
- Do not copy the old test-local `target_os` pattern into new code; use `libc`-provided constants instead.

### Banned standard library usage

The following `std` types, macros, and methods are banned from production code (enforced via `clippy.toml`). Each has a corresponding `sys::` wrapper or byte-correct alternative:

- **Types**: `std::fs::{File, OpenOptions, DirEntry, ReadDir, Metadata}`, `std::process::{Command, Child, Stdio, ExitStatus}`, `std::io::{Error, Result}`, `std::string::String`, `std::path::{Path, PathBuf}`, `std::ffi::{OsStr, OsString}` — use `Vec<u8>` / `&[u8]` for strings and paths (byte-correct on Unix), and `sys::` wrappers for I/O and process types
- **Macros**: `std::{println, eprintln, format, write, writeln}` — use `sys::write_all_fd` for output and `bstr::ByteWriter` for byte-correct formatting
- **Methods**: `std::env::{var, vars, set_var, remove_var, args_os, args, set_current_dir, current_dir, current_exe}`, `std::fs::{read_to_string, write, metadata, read_dir, create_dir, remove_file}`, `std::path::Path::{exists, is_file, is_dir, metadata, canonicalize}`, `std::io::{Error::last_os_error, stdin, stdout, stderr}`, `std::process::exit`
- **Errno constants**: production code must use `crate::sys::constants::ENOENT`, `crate::sys::constants::ENOEXEC`, etc. instead of `libc::ENOENT`, `libc::ENOEXEC`, etc.

### Hash tables for byte-string keys

- `HashMap<Vec<u8>, _>`, `HashMap<Box<[u8]>, _>`, and `HashSet<Vec<u8>>` etc. must use `crate::hash::{ShellMap, ShellSet}`, which are `HashMap` / `HashSet` aliases backed by `ShellHasher` (a fast, fixed-seed non-cryptographic hasher tuned for short byte keys). The default `std::collections::HashMap` uses `SipHash-1-3`, which is DoS-resistant but ~4x slower on our workloads and unnecessary for shell-internal state.
- Integer-keyed maps (`HashMap<Pid, _>`, `HashMap<usize, _>`, etc.) and other non-byte keys keep the `std` default hasher; `ShellHasher` is specifically optimized for byte slices and provides no benefit for primitive keys.
- `ShellHasher` is not adversary-safe; do not use it for keys derived from untrusted network input. The shell does not currently hash such input, so this is theoretical.

### Custom error types

- `sys::error::SysError` replaces `std::io::Error` everywhere. Variants: `SysError::Errno(c_int)` for raw errno values, `SysError::NulInPath` for paths containing NUL bytes.
- `sys::error::SysResult<T>` is the standard result alias (`Result<T, SysError>`).
- Errno handling is fully mockable: `sys::interface::set_errno` / `sys::interface::last_error` replace direct `libc::__errno_location` access. Tests use a thread-local `TEST_ERRNO`.

### Environment and process control

- All environment and process-control operations route through `SystemInterface` function pointers so they can be mocked in tests.
- `sys::process::exit_process` wraps `libc::_exit`; `std::process::exit` is banned.

## Import and Visibility Conventions

### Visibility

Every entity uses the narrowest visibility that compiles:

| Visibility | When to use |
|---|---|
| private | Item used only within its own file |
| `pub(super)` | Item used only by sibling submodules within the same parent |
| `pub(crate)` | Item used from other top-level modules |
| `pub` | Item accessible from external crates — **only** `shell::run::run_from_env` and `sys::process::exit_process` |

Module declarations in `mod.rs` follow the same principle: `mod` (private) if only the parent accesses the submodule, `pub(super) mod` for sibling-only access, `pub(crate) mod` for crate-wide access, `pub mod` only along the path to the two `pub` items above.

### No re-exports

`pub use`, `pub(crate) use`, and `pub(super) use` are prohibited for types, functions, constants, and traits. Every `use` statement must point to the module that defines the item. This eliminates the illusion that an entity belongs to a parent module when it actually lives in a submodule.

The only exception is `pub(crate) use` for `macro_rules!` macros in `lib.rs`, which is the standard Rust mechanism for making macros importable as items.

### Path convention

The `use` path prefix signals the item's visibility at a glance:

| Path prefix | Visibility of target item |
|---|---|
| `use meiksh::` | `pub` — used only in `main.rs` |
| `use crate::` | `pub(crate)` — cross-module access within the crate |
| `use super::` | `pub(super)` — sibling access within the same parent module, or access to items defined in the parent `mod.rs` |

### Wildcard imports

Wildcard imports (`use X::*`) are prohibited in production code.

In colocated `mod tests` blocks, `use super::*;` is permitted (and required for consistency) since it imports from the same file. All other wildcard imports are prohibited in test code as well — imports from test helpers or other modules must be explicit.

### Test module import pattern

Every `mod tests` block begins with `use super::*;` followed by any explicit imports from other modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    // ...
}
```

## POSIX Implementation-Defined Choices

- Variable values are stored as byte arrays (`Vec<u8>`, `Box<[u8]>`), not Rust `String`.
- `ENV` is only sourced when it expands to an absolute path that exists.
- Default prompt is `meiksh$ ` unless `PS1` is set.
- `umask` symbolic mode accepts `s` (setuid/setgid) and `X` (conditional execute); `s` contributes zero bits since `umask` only manages the 0o777 permission mask, accepted without error per POSIX's "unspecified" clause.
- `exec_replace(file, argv)` takes a separate `file` (resolved path) and `argv` vector (where `argv[0]` is the command name as typed), per POSIX 2.9.1.6.
- Executable text files that fail with `ENOEXEC` are interpreted by cloning the shell and sourcing the script, without depending on `/bin/sh`.
- The `interactive` property is determined once at startup (from `-i` flag or terminal detection) and not recomputed dynamically.

## Error Handling Policy

- Prefer explicit shell errors over emulating implementation quirks from historical shells.
- Unsupported grammar or runtime features should fail with a diagnostic rather than silently degrade.
- Special builtin argument/context errors surface as shell errors and terminate non-interactive execution.

## Test Policy

- Unit tests and integration tests must not spawn `/bin/sh` or any other system shell as an oracle for expected behavior.
- Integration tests live in `tests/integration/`; unit tests live alongside production code in `#[cfg(test)]` modules.
- `tests/matrix/` is a separate conformance test suite that runs the built shell binary as a black box. It does not contribute to production-code line coverage and is not counted by `scripts/coverage.sh`.

### Syscall trace model

All unit tests that exercise OS-interacting code paths use the **trace model** in `sys::test_support`. Every OS interaction goes through the `SystemInterface` function-pointer table, which tests replace with a trace-validating mock:

- **`run_trace(trace, closure)`**: installs a sequence of expected syscall entries. Each invocation consumes the next entry, validating name and arguments. Panics on mismatch or unconsumed entries. Fork entries with child traces generate all parent/child execution paths automatically.
- **`assert_no_syscalls(closure)`**: installs a table that panics on any invocation. Used for pure-logic tests to prove they issue no OS calls.

### Test isolation rules

- Tests must be **isolated and in-memory**: no reading/writing real files, no spawning real processes, no dependency on host filesystem layout.
- Each test should verify **one concern**.

## Coverage Policy

- Production-code line coverage must be **100.00%** as measured by `./scripts/coverage.sh`. The script instruments the real (non-test) binary, so `#[cfg(test)]` code is excluded automatically.
- There is no escape-hatch mechanism for exempting individual lines; every instrumented production line must be covered by tests.
- Every production code change must be accompanied by tests that maintain 100% coverage; this threshold must not be lowered.

## Performance Policy

- Optimize shell-owned overhead first: startup, parsing, expansion, builtin dispatch, command lookup, and pipeline construction.
- Prefer clearer, auditable low-level bindings over opaque abstractions when the syscall path materially affects shell semantics or latency.

## Pending Policy Items

- Issue-7 versus Issue-8 behavior toggles for certification-era compatibility
