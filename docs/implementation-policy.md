# Meiksh Implementation Policy

This document records `meiksh` behavior where POSIX leaves room for implementation-defined or unspecified choices, and it also records temporary project decisions while the shell is still under active development.

It is not the primary conformance ledger. Requirement status belongs in `docs/spec-matrix.md` and the supporting files under `docs/requirements/`.

## Project Constraints

- Language: Rust
- Dependency policy: keep dependencies minimal; low-level POSIX interface access lives in `src/sys.rs`
- FFI boundary policy: `libc` is permitted only in `src/sys.rs`, with a narrow documented exception for `tests/integration/basic.rs`; all other modules must go through shell-owned helpers exposed from that layer instead of importing `libc` directly
- Portability policy: do not introduce `#[cfg(target_os = ...)]` switches as a normal implementation technique; platform differences should be absorbed through POSIX-facing helpers in `src/sys.rs`, preferably by relying on `libc`-provided types and constants rather than open-coding per-OS values
- Source policy: no reuse of existing shell source code
- Semantic target: Issue 8 first, with Issue 7 compatibility notes when needed for documentation review
- Conformance policy: POSIX behavior decisions must be based on the local POSIX reference documents in `docs/posix/`, not on probing whatever `/bin/sh` happens to do on the host system

## Low-Level Interface Boundary

- `src/sys.rs` is the only production module that may depend on `libc` directly.
- `tests/integration/basic.rs` may also depend on `libc` for test-only setup of inherited file-descriptor state where using `src/sys.rs` helpers is not practical inside `pre_exec`.
- Code outside `src/sys.rs` should express OS needs in terms of shell-owned helper functions, data types, and constants from `src/sys.rs`.
- If a required interface or constant is missing, extend `src/sys.rs` instead of importing `libc` elsewhere.
- New platform-specific `target_os` branching is not an acceptable default approach for production code or tests.
- Do not copy the old test-local `target_os` pattern into new code; use `libc`-provided constants instead.

### Banned standard library usage

The following `std` types and methods are banned from production code (enforced via `clippy.toml`). Each has a corresponding `sys::` wrapper that routes through the mockable syscall layer:

- **Types**: `std::fs::{File, OpenOptions, DirEntry, ReadDir, Metadata}`, `std::process::{Command, Child, Stdio, ExitStatus}`, `std::io::{Error, Result}`
- **Methods**: `std::env::{var, vars, set_var, remove_var, args_os, args, set_current_dir, current_dir, current_exe}`, `std::fs::{read_to_string, write, metadata, read_dir, create_dir, remove_file}`, `std::path::Path::{exists, is_file, is_dir, metadata, canonicalize}`, `std::io::{Error::last_os_error, stdin, stdout, stderr}`, `std::process::exit`
- **Errno constants**: production code must use `sys::ENOENT`, `sys::ENOEXEC`, etc. instead of `libc::ENOENT`, `libc::ENOEXEC`, etc.

### Custom error types

- `sys::SysError` replaces `std::io::Error` everywhere. Variants: `SysError::Errno(c_int)` for raw errno values, `SysError::NulInPath` for paths containing NUL bytes.
- `sys::SysResult<T>` is the standard result alias (`Result<T, SysError>`).
- Errno handling is fully mockable: `sys::set_errno` / `sys::last_error` replace direct `libc::__errno_location` access. Tests use a thread-local `TEST_ERRNO`.

### Environment and process control

- `sys::env_set_var`, `sys::env_unset_var`, `sys::env_var`, and `sys::env_vars` route through `SystemInterface` function pointers whose default implementations call `libc::setenv`, `libc::unsetenv`, `libc::getenv`, and iterate the `environ` array directly.
- `sys::env_args_os` wraps `std::env::args_os` as the sole authorised `std::env` caller.
- `sys::exit_process` wraps `libc::_exit`; `std::process::exit` is banned.

## Parser

- `meiksh` preserves raw quoting inside parsed words and defers most semantic interpretation to expansion time.
- The tokenizer recognizes `$(...)`, `$((...))`, `${...}`, and `` `...` `` as word-level constructs, keeping them as single word tokens even when unquoted. Nested delimiter tracking respects single-quotes, double-quotes, backslash escapes, and recursive `$`-constructs via dedicated scanner helpers (`scan_dollar_construct`, `scan_paren_body`, `scan_brace_body`, `scan_backtick_body`, `scan_dquote_body`).
- Alias expansion now runs at parser time for aliases already present in shell state before a parse begins. Top-level source execution reparses later list items after earlier ones execute, so aliases defined earlier in the same top-level source can affect later top-level commands. Nested program bodies are also reparsed with the updated alias table when they execute, including bodies that contain here-documents. Aliases ending in blank can expose the next simple-command word to alias substitution.
- Here-document bodies are attached during parsing; `<<-` strips leading tab characters while reading, and expansions run only when the delimiter is unquoted.
- `if`, `while`, `until`, `for`, and `case` are parsed as compound commands. Exact reserved words are no longer accepted as function names, but reserved-word coverage is still incomplete for the full POSIX grammar.
- A standalone `!` is treated as pipeline negation only at pipeline start. A bare `!` in later command-start positions now fails as a syntax error instead of being parsed as a simple command name.
- Self-referential aliases are not expanded indefinitely, but alias recursion does not yet have a dedicated POSIX-fidelity diagnostic model.

## Expansion

- Variable values are currently stored as `String` values in shell state even though the long-term target is byte-oriented storage.
- Command substitution executes in a forked child that inherits the shell state and executes the already-parsed AST directly. Both `$(cmd)` and `` `cmd` `` forms are supported. A non-zero child exit status sets `$?` but does not make the substitution fail; the captured output is always returned. Backtick escaping follows POSIX rules: `\$`, `` \` ``, and `\\` are special outside double-quotes; `\"` and `\newline` are additionally special inside double-quotes.
- Arithmetic expansion currently supports integer literals and `+`, `-`, `*`, `/`, and `%`.
- Parameter expansion supports plain substitutions, `${#parameter}` length, the default/assign/error/alternate forms (`:-`, `-`, `:=`, `=`, `:?`, `?`, `:+`, `+`), and multi-digit positional references such as `${10}`. The `${...}` brace scanner (`scan_to_closing_brace`) correctly handles `}` inside single-quotes, double-quotes (including `\}`), backslash escapes, and nested `${...}`, `$(...)`, `$((...))`, and backtick constructs.
- The expansion pipeline uses a `Segment` enum (`Text(String, bool)`, `AtBreak`, `AtEmpty`) to represent intermediate expansion results. `expand_dollar` returns an `Expansion` enum: `One(String)` for all parameters except quoted `$@`, and `AtFields(Vec<String>)` for quoted `$@`. The `expand_word` function dispatches to three paths: (A) has `$@` expansion → split at `AtBreak` markers; (B) all segments are quoted → flatten to one field; (C) has unquoted content → field-split then pathname-expand.
- `"$@"` (quoted) produces separate fields, one per positional parameter. With zero positionals it produces zero fields, including when embedded in a word like `"pre$@suf"`. `"$*"` (quoted) joins positionals with IFS[0] (space if IFS unset, empty if IFS is empty). Unquoted `$@` and `$*` both produce a single string that undergoes normal field splitting.
- The `Context` trait provides `positional_params() -> Vec<String>` for direct access to the positional parameter list, used by both `$@` and `$*` expansion.
- Unquoted field splitting now distinguishes IFS whitespace from non-whitespace delimiters, and pathname expansion applies after field splitting with dotfile suppression unless the pattern segment starts with `.`.
- `set -f` and shell startup `-f` disable pathname expansion while preserving the rest of word expansion.

## Execution

- Builtins mutate the current shell state only when they execute outside a pipeline/background context.
- `return`, `break`, and `continue` execute in the current shell and propagate through function and loop boundaries using current-shell control flow rather than subshell emulation.
- External commands are executed via fork+exec in `exec.rs`. The child process inherits redirections, environment, and process group settings, then calls `sys::exec_replace`. Subshells, pipelines, and command substitution also use fork, with the child inheriting a cloned `Shell` instance and executing the already-parsed AST directly.
- `exec_replace(file, argv)` takes a separate `file` (the resolved path for `execvp`) and full `argv` vector (where `argv[0]` is the command name as typed by the user, not the resolved path), per POSIX 2.9.1.6.
- Command PATH search (`resolve_command_path`) checks both `is_regular_file()` and `is_executable()` via `stat_path`. Pre-exec access checks use `F_OK` then `X_OK` separately to distinguish not-found (ENOENT → exit 127) from found-but-not-executable (EACCES → exit 126).
- Executable text files that fail with `ENOEXEC` are handled in the child process by cloning the shell, setting `shell_name` to the original command name (for `$0`), and interpreting the script via `source_path`, without depending on `/bin/sh`.
- Prefix variable assignments before non-special builtins and functions are temporary: `save_vars` snapshots the variable value and export status before the command, and `restore_vars` reverts them after. Special-builtin prefix assignments remain permanent per POSIX 2.9.1.2.
- Assignment values are expanded via `expand_word_text` which performs tilde, parameter, command substitution, arithmetic expansion, and quote removal — but not field splitting or pathname expansion, per POSIX 2.9.1.1 step 4.
- Background (`&`) commands redirect stdin from `/dev/null` via `stdin_override` threaded through `spawn_and_or` → `spawn_pipeline`. AND-OR lists terminated by `&` (e.g. `cmd1 && cmd2 &`) execute the full AND-OR list asynchronously in a forked subshell. The job start message prints `[%d] %d\n` (job id and last PID).
- Simple, builtin, function, and compound-command execution supports numeric descriptor prefixes for `<`, `>`, `>|`, `>>`, `<<`, `<&`, `>&`, and `<>`; `set -C` enables noclobber for plain `>` while `>|` forces truncation.

## Interactive Behavior

- `ENV` is only sourced when it expands to an absolute path that exists.
- Prompting defaults to `meiksh$ ` unless `PS1` is set.
- History currently appends plain input lines to `HISTFILE` or `$HOME/.sh_history`.
- Interactive shells ignore SIGQUIT and SIGTERM at startup, and install a SIGINT handler; SIGINT during line input discards the current line and re-prompts.
- The `interactive` property is determined once at startup (from `-i` flag or terminal detection) and stored as a field, not recomputed dynamically.
- Job control is still partial: `wait`, `fg`, and `bg` operate on the shell's current job table, but tty foreground handoff and some POSIX-required output/details are not implemented yet.

## Error Handling

- `meiksh` currently prefers explicit shell errors over emulating implementation quirks from historical shells.
- Unsupported grammar or runtime features should fail with a diagnostic rather than silently degrade.
- Special builtin argument/context errors currently surface as shell errors and terminate non-interactive execution instead of being ignored.

## Test Policy

- Unit tests and integration tests must not spawn `/bin/sh` or any other system shell as an oracle for expected behavior.
- Integration tests live in `tests/integration/`; unit tests live alongside production code in `#[cfg(test)]` modules.

### Syscall trace model

All unit tests that exercise OS-interacting code paths use the **trace model** implemented in `sys::test_support`. Every OS interaction in both production and test code goes through the `sys::SystemInterface` function-pointer table, which tests replace with a trace-validating mock:

- **`run_trace(trace, closure)`**: installs a sequence of expected `TraceEntry` values (syscall name, argument matchers, canned result). Each syscall invocation consumes the next entry, validating name and arguments. Panics on mismatch or unconsumed entries. When the trace contains `fork` entries with child traces (`t_fork`), `run_trace` uses `enumerate_fork_paths` to generate all parent/child execution paths and runs the closure once per path. Child paths intercept `exit_process` via a `ChildExitPanic` payload. A runtime assertion enforces that every successful fork (pid > 0) has an explicit child trace.
- **`assert_no_syscalls(closure)`**: installs a `SystemInterface` table that panics on any invocation. Used for pure-logic tests (Category B) to prove they issue no OS calls.
- **`ArgMatcher`** supports `Exact`, `Str`, `Fd`, `Int`, `Pid`, `Bytes`, and `Any` for flexible argument validation with wildcards.

### Test isolation rules

- Tests must be **isolated and in-memory**: no reading/writing real files, no spawning real processes, no dependency on host filesystem layout.
- Each test should verify **one concern**. Tests that called `run_trace` multiple times or tested unrelated behaviors in a single function have been split into separate focused tests.
- Tests that previously tested separate functionality in a single function must be split into multiple focused tests.

## Performance Policy

- Optimize shell-owned overhead first: startup, parsing, expansion, builtin dispatch, command lookup, and pipeline construction.
- Prefer clearer, auditable low-level bindings over opaque abstractions when the syscall path materially affects shell semantics or latency.
- Production-code line coverage must remain at 100.00% as measured by `./scripts/coverage.sh`, using the repository's production-only metric that excludes inline `#[cfg(test)]` modules from the final percentage.

## Pending Policy Items

- byte-oriented shell value storage
- reserved-word and alias interaction
- issue-7 versus issue-8 behavior toggles for certification-era compatibility
- signal and trap policy details
- tty handoff policy for `fg`, `bg`, and interactive pipelines
