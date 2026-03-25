# Meik Shell

`meiksh` (pronounced /maiksh/) is a new Unix shell written in Rust with no dependencies beyond `std` and `libc`.

Operating-system integration is concentrated in `src/sys.rs`, which uses low-level POSIX FFI bindings via `libc`.

## Project Goals

- implement a POSIX-conformant `sh`-style shell
- keep the implementation limited to `std` and `libc`
- use explicit, auditable Unix bindings instead of external abstraction crates
- maintain `100.00%` production-code line coverage as reported by `./scripts/coverage.sh`

The current semantic target is POSIX Issue 8, with Issue 7 behavior still tracked where existing validation suites are likely to care. The local `docs/posix/` mirror defined by `docs/posix-manifest.txt` is the only requirements source of truth used for conformance work.

## Current State

`meiksh` is already a working shell with substantial parser, expansion, execution, and builtin coverage, including:

- `-a`, `-c`, `-C`, `-f`, `-n`, `-u`, and `-s` startup handling for the implemented subset, including POSIX-style `command_name` / `$0`, named `-o` / `+o` forms for the same subset, lone `-` stdin handling, `$-` reporting for active flags, plain `nounset` expansion failures, and blocking-read correction for inherited non-blocking stdin
- simple commands, pipelines, `&&` / `||`, background execution, subshells, groups, functions, `if`, `case`, `for`, `while`, and `until`
- parser-time alias expansion, including blank-terminated alias chaining and same-source visibility across top-level and nested bodies
- context-sensitive `!` pipeline negation and POSIX grammar-sensitive reserved-word handling for `for`, `case`, brace groups, and linebreaks after `|`, `&&`, and `||`
- parameter expansion, including POSIX default/assign/error/alternate and pattern-removal operators, Issue 8 dollar-single-quotes, plus command substitution, arithmetic expansion, field splitting, pathname expansion, and here-documents
- current-shell redirections for builtins and compound commands, including numeric fd forms
- a growing set of POSIX builtins such as `alias`, `bg`, `break`, `cd`, `command`, `continue`, `.`, `eval`, `exec`, `exit`, `export`, `fg`, `jobs`, `pwd`, `read`, `readonly`, `return`, `set`, `shift`, `times`, `trap`, `umask`, `unalias`, `unset`, and `wait`
- utility-specific progress on recent builtin fidelity work and shell-language closure, including parser-aware alias behavior, grammar-faithful `for`/`case` reserved-word handling, brace-group reserved-word parsing, linebreak-sensitive pipelines and AND-OR lists, `${parameter%word}` / `${parameter##word}`-style pattern trimming, `command -p/-v/-V`, `cd -` / `OLDPWD` / `CDPATH`, `.` `PATH` search for readable slashless files, `jobs -p`, `pwd -L/-P`, `export -p`, `readonly -p`, `unalias -a`, `unset -f/-v`, intrinsic `read`, syscall-backed `times` and `umask`, `trap -p` plus EXIT and selected signal traps, and `wait` support for both `%job` and numeric pid operands
- interactive startup via parameter-expanded `ENV`, prompt handling, simple history in `HISTFILE` or `$HOME/.sh_history`, interactive command-error reporting without exiting the prompt loop, tracked background jobs, process-group-aware `fg`/`bg`, and best-effort tty foreground handoff for interactive descriptors

The project does **not** yet claim full POSIX conformance. Remaining gaps are tracked in `docs/spec-matrix.md` and `docs/requirements/gap-register.md`. The largest open areas are currently:

- the remaining `sh` utility startup details, especially broader option coverage and the remaining top-level exit-status/error-classification polish
- field-splitting, tilde, double-quote, and arithmetic-expansion edge cases
- subshell / command-substitution execution-environment fidelity
- broader builtin completion, including the still-open `set`, `read`, `trap`, `umask`, and missing mirrored utility pages such as `hash`, `getopts`, `ulimit`, and `fc`
- stopped-job accounting, `set -m`, and tty mode save/restore for job control

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

The implementation is driven by local POSIX reference material under `docs/posix/`, which is intentionally not committed for copyright reasons. The required local mirror is defined by `docs/posix-manifest.txt`; `docs/fetch-posix-docs.sh` populates it and `scripts/check-posix-docs.sh` validates completeness. See `docs/README.md` for the workflow and expected local layout.

## Coverage

Run production-only coverage with:

```sh
./scripts/coverage.sh
```

This prints the production-code coverage summary on stdout and writes detailed artifacts under `target/coverage/`.
