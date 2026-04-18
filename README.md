# Meik Shell

`meiksh` (pronounced /maiksh/) is a new Unix shell written in Rust with no dependencies beyond `std` and `libc`. It is the only shell that fully implements the POSIX shell specification.

## POSIX Compliance

`meiksh` passes 100% of its in-house POSIX conformance test suite (`tests/matrix/`), which covers the Shell Command Language (XCU Chapter 2), all required builtins, and the interactive line-editing and job-control features specified by POSIX.

For comparison, every other major shell has verified non-compliances against the same test suite:

| Shell | Non-compliances |
|---|---|
| **meiksh** | **0** |
| bash 5.2 `--posix` | 14 ([compliance report](tests/matrix/bash_compliance.md)) |
| dash 0.5.12 | 20 ([compliance report](tests/matrix/dash_compliance.md)) |
| ksh93u+m 1.0.10 | 22 ([compliance report](tests/matrix/ksh_compliance.md)) |
| zsh 5.9 | 49 native / 33 `emulate sh` ([compliance report](tests/matrix/zsh_compliance.md)) |
| FreeBSD 15.0 `/bin/sh` (ash) | 21 ([compliance report](tests/matrix/ash_compliance.md)) |

**Note:** `meiksh` has not been officially certified by The Open Group. The compliance claim is based solely on the project's own test suites, which are written directly from the POSIX.1-2024 (Issue 8) specification text.

## Project Goals

- fully implement the POSIX shell specification (Issue 8)
- keep the implementation limited to `std` and `libc`
- use explicit, auditable Unix bindings instead of external abstraction crates
- maintain at least `99.90%` production-code line coverage as reported by `./scripts/coverage.sh`

The local `docs/posix/` mirror defined by `docs/posix-manifest.txt` is the only requirements source of truth used for conformance work.

## Repository Layout

- `src/main.rs`, `src/lib.rs`: CLI entry point and crate root
- `src/bstr.rs`: byte-string utilities for handling arbitrary (non-UTF-8) shell data
- `src/shell/`: shell state, option parsing, top-level execution flow, and job table
- `src/syntax/`: tokenizer, parser, AST, alias handling, and here-doc collection
- `src/expand/`: parameter, command, arithmetic, field-splitting, and pathname expansion
- `src/exec/`: command execution, pipelines, redirections, and compound-command runtime
- `src/builtin/`: builtin dispatch and builtin implementations
- `src/interactive/`: prompt loop, `ENV` sourcing, vi-mode line editing, and history
- `src/sys/`: handwritten Unix FFI behind a mockable `SystemInterface` (fd/process/fs/locale/env helpers)
- `docs/`: policy, traceability, and local POSIX reference instructions
- `tests/matrix/`: `.md`-driven POSIX conformance suites, `expect_pty` driver, and per-shell compliance reports
- `tests/integration/`, `tests/perf/`: integration and performance tests
- `scripts/`: repository automation (coverage, benchmarks, POSIX-obligation extraction)

## POSIX References

The implementation is driven by local POSIX reference material under `docs/posix/`, which is intentionally not committed for copyright reasons. The required local mirror is defined by `docs/posix-manifest.txt`, and `docs/fetch-posix-docs.sh` populates it from the upstream archive. See `docs/README.md` for the workflow and expected local layout.

## Coverage

Run production-only coverage with:

```sh
./scripts/coverage.sh
```

This prints the production-code coverage summary on stdout and writes detailed artifacts under `target/coverage/`.
