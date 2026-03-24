# Meik Shell

`meiksh` (pronounced /maiksh/) is a new Unix shell written in Rust with no dependencies beyond `std`.

All operating-system integration is implemented with handwritten POSIX FFI bindings. `meiksh` does not depend on any third-party Rust crates.

## Dependency Policy

- Rust standard library only
- no external crates
- handwritten POSIX FFI for Unix interfaces

## Repository Layout

- `src/syntax/`: parser and shell grammar representation
- `src/expand/`: shell word expansion
- `src/exec/`: command execution and pipelines
- `src/builtin/`: shell builtins
- `src/interactive/`: REPL, prompts, and interactive helpers
- `src/sys/`: low-level POSIX bindings
- `docs/`: standards references, policy notes, and benchmark plans
- `tests/`: spec, differential, and performance harnesses
- `scripts/`: repository automation such as coverage reporting

## Current State

The repository already boots as a working shell binary with:

- `-c` command execution
- `-n` syntax checking
- simple pipelines and `&&` / `||`
- variable assignment and basic expansion
- a first batch of shell builtins
- a basic interactive prompt and background job tracking

The implementation is still early and does not yet claim complete POSIX conformance.

## Coverage

Run exact source-based coverage with:

```sh
./scripts/coverage.sh
```

This produces a summary on stdout plus detailed outputs under `target/coverage/`.
