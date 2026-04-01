# Meik Shell

`meiksh` (pronounced /maiksh/) is a new Unix shell written in Rust with no dependencies beyond `std` and `libc`.

Operating-system integration is concentrated in `src/sys.rs`, which uses low-level POSIX FFI bindings via `libc`.

## Project Goals

- implement a POSIX-conformant `sh`-style shell
- keep the implementation limited to `std` and `libc`
- use explicit, auditable Unix bindings instead of external abstraction crates
- maintain at least `99.90%` production-code line coverage as reported by `./scripts/coverage.sh`

The current semantic target is POSIX Issue 8, with Issue 7 behavior still tracked where existing validation suites are likely to care. The local `docs/posix/` mirror defined by `docs/posix-manifest.txt` is the only requirements source of truth used for conformance work.

## Current State

`meiksh` is already a working shell with substantial parser, expansion, execution, and builtin coverage, including:

- `-a`, `-b`, `-c`, `-C`, `-e`, `-f`, `-h`, `-m`, `-n`, `-s`, `-u`, `-v`, and `-x` startup handling, including combined flags with `-c` (e.g. `sh -ec '...'`), POSIX-style `command_name` / `$0`, named `-o` / `+o` forms for all 11 option names (`allexport`, `errexit`, `hashall`, `monitor`, `noclobber`, `noglob`, `noexec`, `notify`, `nounset`, `verbose`, `xtrace`), lone `-` stdin handling, `$-` reporting for all active flags (with `i` fixed at startup per POSIX), verbose input echoing, plain `nounset` expansion failures, and blocking-read correction for inherited non-blocking stdin
- `set -e` (errexit) with full POSIX exception rules: suppressed in `if`/`while`/`until`/`elif` conditions, negated pipelines, and non-final AND-OR commands; per-subshell tracking; exit on non-zero status
- `set -x` (xtrace) with `PS4` parameter expansion; trace output to stderr after expansion and before execution
- simple commands, pipelines, `&&` / `||`, background execution (including AND-OR lists with `&` in a subshell, stdin from `/dev/null`, and `[%d] %d\n` job messages), subshells (with POSIX trap reset and state isolation), groups, functions, `if`, `case`, `for`, `while`, and `until` — all compound command bodies execute directly from the parsed AST without render+reparse
- parser-time alias expansion, including blank-terminated alias chaining and same-source visibility across top-level list items
- context-sensitive `!` pipeline negation and POSIX grammar-sensitive reserved-word handling for `for`, `case`, brace groups, and linebreaks after `|`, `&&`, and `||`
- parameter expansion, including POSIX default/assign/error/alternate and pattern-removal operators, Issue 8 dollar-single-quotes, command substitution (`$(cmd)` and `` `cmd` ``), full arithmetic expansion (all POSIX operators, variable references, hex/octal, assignment side effects), tilde expansion (`~`, `~user` via `getpwnam`, tilde after `:` in assignments), POSIX-compliant double-quote backslash escaping, quote-aware `${...}` brace scanning, `"$@"` separate-field semantics, `"$*"` IFS joining, field splitting, pathname expansion, and here-documents; assignment values skip field splitting and pathname expansion per POSIX 2.9.1.1
- current-shell redirections for builtins and compound commands, including numeric fd forms
- POSIX command search with `X_OK` executable checking, correct `argv[0]` (command name as typed), exit 126/127 distinction for EACCES vs ENOENT, and ENOEXEC fallback setting `$0`
- temporary prefix variable assignments for non-special builtins and functions (save/restore), with permanent assignments for special builtins per POSIX 2.9.1.2
- a growing set of POSIX builtins such as `alias`, `bg`, `break`, `cd`, `command`, `continue`, `.`, `eval`, `exec`, `exit`, `export`, `fg`, `jobs`, `kill`, `pwd`, `read`, `readonly`, `return`, `set`, `shift`, `times`, `trap`, `umask`, `unalias`, `unset`, and `wait`
- utility-specific progress on recent builtin fidelity work and shell-language closure, including parser-aware alias behavior, grammar-faithful `for`/`case` reserved-word handling, brace-group reserved-word parsing, linebreak-sensitive pipelines and AND-OR lists, `${parameter%word}` / `${parameter##word}`-style pattern trimming, `command -p/-v/-V`, `cd -L/-P/-e` with full POSIX 10-step algorithm (`CDPATH`, `-`, `OLDPWD`, logical path canonicalization), `.` `PATH` search for readable slashless files, `jobs -l/-p` with `+`/`-` markers, `pwd -L/-P`, `export -p`, `readonly -p`, `unalias -a`, `unset -f/-v`, `read` with `REPLY` default / `-r` / `-d` / IFS splitting, syscall-backed `times` and `umask` (full symbolic mode including `s`/`X`), `trap -p` with 18 signals / SIG prefix / ignored-on-entry tracking, `kill` with `-l`/`-s`/numeric shorthand/`%job`/`--`, and `wait` support for both `%job` and numeric pid operands
- interactive startup via parameter-expanded `ENV`, prompt handling, simple history in `HISTFILE` or `$HOME/.sh_history`, interactive command-error reporting without exiting the prompt loop, POSIX-compliant interactive signal handling (SIGQUIT/SIGTERM ignored, SIGINT discards current line), full POSIX job control with `set -m` (shell process group setup, terminal foreground ownership, `WUNTRACED`/`wifstopped` stopped-job detection, terminal attribute save/restore via `tcgetattr`/`tcsetattr`, complete job-id grammar, `fg`/`bg` with `SIGCONT` and terminal handoff, `kill` builtin, async signal inheritance)

The project does **not** yet claim full POSIX conformance. Remaining gaps are tracked in `docs/spec-matrix.md`. The largest open areas are currently:

- missing mirrored utility pages such as `hash`, `getopts`, `ulimit`, and `fc`
- interactive editing (vi-mode) and command history list semantics

## Repository Layout

- `src/main.rs`: CLI entry point
- `src/shell.rs`: shell state, option parsing, top-level execution flow, and job table
- `src/syntax.rs`: tokenizer, parser, AST, alias handling, and here-doc collection
- `src/expand.rs`: parameter, command, arithmetic, field-splitting, and pathname expansion
- `src/exec.rs`: command execution, pipelines, redirections, and compound-command runtime
- `src/builtin.rs`: builtin dispatch and builtin implementations
- `src/interactive.rs`: prompt loop, `ENV` sourcing, and history helpers
- `src/sys.rs`: handwritten Unix FFI and wait/fd helpers
- `docs/`: policy, traceability, and local POSIX reference instructions
- `tests/`: spec-oriented test suites
- `scripts/`: repository automation such as coverage reporting

## POSIX References

The implementation is driven by local POSIX reference material under `docs/posix/`, which is intentionally not committed for copyright reasons. The required local mirror is defined by `docs/posix-manifest.txt`, and `docs/fetch-posix-docs.sh` populates it from the upstream archive. See `docs/README.md` for the workflow and expected local layout.

## Coverage

Run production-only coverage with:

```sh
./scripts/coverage.sh
```

This prints the production-code coverage summary on stdout and writes detailed artifacts under `target/coverage/`.
