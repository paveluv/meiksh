# Gap Register

This register turns broad partial-conformance areas into named backlog items that can be moved milestone by milestone.

## Closed Structural Gaps

| Gap ID | Linked requirement area | Resolution |
| --- | --- | --- |
| `GAP-DOCS-001` | `REQ-DOCS-MIRROR-*` | Closed. The local standards mirror is now complete relative to `docs/posix-manifest.txt`, and `scripts/check-posix-docs.sh` validates it mechanically. |
| `GAP-SH-002` | `REQ-SH-STARTUP-*`, `REQ-SH-OPERANDS-*` | Closed (Milestone 1). `$-` `i` flag is now fixed at startup in the `interactive` field, not recomputed dynamically from terminal state. |
| `GAP-SH-004` | `REQ-SH-INTERACTIVE-*`, `REQ-JOBS-SIGNALS-*` | Closed (Milestone 1). Interactive shells now ignore SIGQUIT and SIGTERM at startup, install SIGINT handler, and SIGINT during `read_line` discards the current input and re-prompts. |
| `GAP-EXPAND-004` | `REQ-EXPAND-CMDSUB-*` | Closed (Milestone 1). `capture_output` no longer returns `Err` on non-zero child exit. It always returns `Ok(text)` and sets `self.last_status` to the child's exit code. |
| `GAP-EXEC-005` | `REQ-EXEC-STATUS-*` | Closed (Milestone 1). Removed the erroneous immediate `remove` after `insert` in `wait_on_job_index`. Job status is now correctly consumed on the first subsequent `wait` call. |
| `GAP-EXPAND-001` | `REQ-EXPAND-FIELDS-*`, `REQ-EXPAND-PARAMS-*` | Closed (Milestone 2). `"$@"` now produces separate fields via the `Segment::AtBreak` / `Expansion::AtFields` model. Zero positionals produce zero fields. Prefix/suffix joining works correctly. `"$*"` uses `positional_params()` and joins with IFS[0]. |
| `GAP-EXPAND-003` | `REQ-EXPAND-PARAMS-*` | Closed (Milestone 2). `${...}` brace scanning now respects quotes, backslash escapes, and nested `${...}`, `$(...)`, `$((...))`, and backtick constructs via `scan_to_closing_brace`. Backtick command substitution (`` `cmd` ``) is recognized by both the tokenizer and expander, including inside double-quotes. The tokenizer also correctly keeps `$(...)`, `$((...))`, and `${...}` as single word tokens when unquoted. |
| `GAP-EXEC-001` | `REQ-EXEC-ERRORS-*` | Closed (Milestone 3). Prefix variable assignments before non-special builtins and functions are now temporary (save/restore via `save_vars`/`restore_vars`). Special-builtin assignments remain permanent per POSIX. Assignment values are expanded via `expand_word_text` which performs tilde, parameter, command substitution, arithmetic expansion, and quote removal — but no field splitting or pathname expansion. |
| `GAP-EXEC-003` | `REQ-EXEC-SEARCH-*`, `REQ-EXEC-UTILITY-*` | Closed (Milestone 3). `resolve_command_path` now checks both `is_regular_file()` and `is_executable()` via `stat_path`. Pre-exec access check uses both `F_OK` and `X_OK`. `exec_replace` takes separate `file` and `argv` parameters so `argv[0]` is the command name as typed. Exec failure exit codes: ENOENT → 127, EACCES/other → 126. ENOEXEC fallback sets `child_shell.shell_name` to the original command name. |
| `GAP-EXEC-004` | `REQ-EXEC-ASYNC-*` | Closed (Milestone 3). Background `&` commands redirect stdin from `/dev/null` via `stdin_override` threaded through `spawn_and_or` → `spawn_pipeline`. Job start message now prints `[%d] %d\n` (job id and last PID). AND-OR lists with `&` (e.g. `cmd1 && cmd2 &`) execute the full AND-OR list asynchronously in a forked subshell via `execute_and_or`. |

## High-Priority Open Gaps

| Gap ID | Linked requirement area | Current gap |
| --- | --- | --- |
| `GAP-SH-001` | `REQ-SH-OPTIONS-*` | `sh` and `set` now cover `-a`, `-C`, `-f`, `-n`, `-u`, `-v`, and named `-o`/`+o` forms for the implemented subset, but critical options are still missing: `-e` (errexit), `-x` (xtrace), `-b` (notify), `-m` (monitor), `-h` (hashall). Any script using `set -e` fails. Combined option flags with `-c` (e.g. `sh -ac '...'`) also fail because `-c` is not handled inside the multi-char option loop. |
| `GAP-SH-003` | `REQ-SH-INTERACTIVE-*` | Command history list semantics are still simplified, and command-line / vi-mode editing remain unimplemented. |
| `GAP-EXPAND-002` | `REQ-EXPAND-TILDE-*`, `REQ-EXPAND-QUOTE-*`, `REQ-EXPAND-ARITH-*` | Tilde expansion only handles bare `~` at position 0; `~user` (via `getpwnam`) and tilde after `:` in assignment values are missing. Backslash inside double quotes escapes all characters instead of only `$`, `` ` ``, `"`, `\`, and newline. Arithmetic expansion supports only `+ - * / %` with integer literals; missing: comparison, bitwise, logical, ternary, assignment operators, variable references, and parameter expansion within `$((...))`. |
| `GAP-EXEC-002` | `REQ-EXEC-ENV-*`, `REQ-EXPAND-CMDSUB-*`, `REQ-EXEC-GROUP-*` | Subshell and command substitution use fork with inherited shell state. Remaining gaps: subshell trap reset, fine-grained environment isolation, and `execute_nested_program` (`exec.rs:1057`) round-trips the parsed AST through render+reparse which risks quoting, heredoc, and alias fidelity loss. |
| `GAP-BUILTIN-001` | `REQ-BUILTIN-CD-*` | `cd` still lacks `-L`/`-P`/`-e` and full logical-path fidelity after the new `CDPATH` coverage. |
| `GAP-BUILTIN-002` | `REQ-BUILTIN-SET-*` | `set` now has a stronger core subset, including `-a`, `-u`, `-v`, named `-o`/`+o`, option-state reporting, and plain `nounset` expansion failures, but most remaining Issue 8 options and their exact semantics are still missing. |
| `GAP-BUILTIN-003` | `REQ-BUILTIN-READ-*` | `read` still needs tighter multibyte, prompt, and corner-case conformance. |
| `GAP-BUILTIN-004` | `REQ-BUILTIN-TRAP-*` | Trap coverage still lacks ignored-on-entry semantics, broader signal names, and subshell/command-substitution exceptions. |
| `GAP-BUILTIN-005` | `REQ-BUILTIN-UMASK-*` | Full chmod-style symbolic `umask` operands remain incomplete. |
| `GAP-BUILTIN-006` | `REQ-BUILTIN-*`, `REQ-SYS-ACCOUNTING-*` | Mirrored utility pages such as `hash`, `getopts`, `ulimit`, and `fc` still need either implementation or an explicit conformance decision before a full POSIX claim is credible. |
| `GAP-JOBS-001` | `REQ-JOBS-CONTROL-*` | Job control still lacks `set -m`, stopped-job tracking, tty mode restore, and complete job-id grammar. |
| `GAP-JOBS-002` | `REQ-JOBS-SIGNALS-*` | Async-list signal inheritance and some signal-interruption rules remain open. |
| `GAP-JOBS-003` | `REQ-SYS-JOBCONTROL-*`, `REQ-JOBS-CONTROL-*` | Terminal mode save/restore through the mirrored termios interfaces is still missing, so foreground job handoff is only partially compliant. |

## Milestone Mapping

Each milestone targets roughly equal implementation effort.

- **Milestone 0** (complete): structural mirror and matrix work; `GAP-DOCS-001` closed.
- **Milestone 1** (complete): quick correctness fixes; `GAP-SH-002`, `GAP-SH-004`, `GAP-EXPAND-004`, `GAP-EXEC-005` closed.
- **Milestone 2** (complete): core expansion fixes; `GAP-EXPAND-001`, `GAP-EXPAND-003` closed. `"$@"` separate-field semantics via `Segment`/`Expansion` enums, quote-aware `${...}` brace scanning via `scan_to_closing_brace`, backtick command substitution in tokenizer and expander, `$*` IFS join via `positional_params()`.
- **Milestone 3** (complete): execution model fixes; `GAP-EXEC-001`, `GAP-EXEC-003`, `GAP-EXEC-004` closed. Temporary prefix assignments via `save_vars`/`restore_vars`, assignment expansion without field splitting via `expand_word_text`, command search `X_OK`/`is_executable()`, separate `file`/`argv` in `exec_replace` for correct `argv[0]`, exit 126/127 distinction, ENOEXEC `$0` fix, background stdin from `/dev/null`, job message `[%d] %d\n`, AND-OR with `&` via subshell fork.
- **Milestone 4** — Shell options (errexit, xtrace): `GAP-SH-001`, `GAP-BUILTIN-002`. Add `-e`, `-x`, `-b`, `-h` and remaining `set` option surface. Errexit is pervasive and needs careful integration across all execution paths.
- **Milestone 5** — Expansion completeness: `GAP-EXPAND-002`. Full arithmetic expression parser, `~user` via `getpwnam`, double-quote backslash fix, and parameter expansion within `$((...))`.
- **Milestone 6** — Execution environment and subshells: `GAP-EXEC-002`. Eliminate render+reparse round-trip, subshell trap reset, and fine-grained environment isolation.
- **Milestone 7** — Builtin completeness: `GAP-BUILTIN-001`, `GAP-BUILTIN-003`, `GAP-BUILTIN-004`, `GAP-BUILTIN-005`. `cd -L/-P/-e`, `read` edge cases, trap ignored-on-entry and signal names, symbolic `umask`.
- **Milestone 8** — Job control: `GAP-JOBS-001`, `GAP-JOBS-002`, `GAP-JOBS-003`, `GAP-SH-001` (`-m` portion). `set -m`, stopped-job tracking, signal inheritance, termios save/restore, complete job-id grammar.
- **Milestone 9** — Missing builtins and interactive editing: `GAP-BUILTIN-006`, `GAP-SH-003`. Implement `hash`, `getopts`, `ulimit`, `fc`, command history semantics, and vi-mode line editing.
