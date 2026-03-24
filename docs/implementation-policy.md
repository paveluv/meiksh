# Meiksh Implementation Policy

This document records `meiksh` behavior where POSIX leaves room for implementation-defined or unspecified choices, and it also records temporary project decisions while the shell is still under active development.

## Project Constraints

- Language: Rust
- Dependency policy: `std` only, plus handwritten `extern "C"` bindings for required POSIX interfaces
- Source policy: no reuse of existing shell source code
- Semantic target: Issue 8 first, with Issue 7 compatibility notes when needed for validation

## Current Policy Decisions

## Parser

- `meiksh` preserves raw quoting inside parsed words and defers most semantic interpretation to expansion time.
- Alias expansion is not implemented yet; current behavior is equivalent to aliases being disabled.
- Here-document bodies are attached during parsing; `<<-` strips leading tab characters while reading, and expansions run only when the delimiter is unquoted.
- `if`, `while`, `until`, `for`, and `case` are parsed as compound commands, but reserved-word coverage is still incomplete for the full POSIX grammar.
- A standalone `!` token is currently treated as pipeline negation only; using `!` as an ordinary simple-command argument is not yet fully distinguished from that grammar role.

## Expansion

- Variable values are currently stored as `String` values in shell state even though the long-term target is byte-oriented storage.
- Command substitution is currently executed by recursively invoking the current `meiksh` binary with `-c`.
- Arithmetic expansion currently supports integer literals and `+`, `-`, `*`, `/`, and `%`.
- Parameter expansion supports plain substitutions, `${#parameter}` length, the default/assign/error/alternate forms (`:-`, `-`, `:=`, `=`, `:?`, `?`, `:+`, `+`), and multi-digit positional references such as `${10}`.
- Unquoted field splitting now distinguishes IFS whitespace from non-whitespace delimiters, and pathname expansion applies after field splitting with dotfile suppression unless the pattern segment starts with `.`.
- `set -f` and shell startup `-f` disable pathname expansion while preserving the rest of word expansion.

## Execution

- Builtins mutate the current shell state only when they execute outside a pipeline/background context.
- `return`, `break`, and `continue` execute in the current shell and propagate through function and loop boundaries using current-shell control flow rather than subshell emulation.
- External commands currently use Rust process spawning plus explicit environment handoff; deeper process-group and tty controls will move more of the hot path into `src/sys/`.
- Executable text files that fail with an `ENOEXEC`-equivalent error are retried via `sh` with the resolved script pathname as the first operand.
- Simple, builtin, function, and compound-command execution supports numeric descriptor prefixes for `<`, `>`, `>|`, `>>`, `<<`, `<&`, `>&`, and `<>`; `set -C` enables noclobber for plain `>` while `>|` forces truncation.

## Interactive Behavior

- `ENV` is only sourced when it expands to an absolute path that exists.
- Prompting defaults to `meiksh$ ` unless `PS1` is set.
- History currently appends plain input lines to `HISTFILE` or `.meiksh_history`.
- Job control is still partial: `wait`, `fg`, and `bg` operate on the shell's current job table, but tty foreground handoff and some POSIX-required output/details are not implemented yet.

## Error Handling

- `meiksh` currently prefers explicit shell errors over emulating implementation quirks from historical shells.
- Unsupported grammar or runtime features should fail with a diagnostic rather than silently degrade.
- Special builtin argument/context errors currently surface as shell errors and terminate non-interactive execution instead of being ignored.

## Performance Policy

- Optimize shell-owned overhead first: startup, parsing, expansion, builtin dispatch, command lookup, and pipeline construction.
- Prefer clearer, auditable low-level bindings over opaque abstractions when the syscall path materially affects shell semantics or latency.

## Pending Policy Items

- byte-oriented shell value storage
- reserved-word and alias interaction
- issue-7 versus issue-8 behavior toggles for certification-era compatibility
- signal and trap policy details
- tty handoff policy for `fg`, `bg`, and interactive pipelines
