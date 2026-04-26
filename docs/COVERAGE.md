# Coverage

`meiksh` measures Rust source coverage with LLVM's built-in
source-based instrumentation. The project-wide coverage requirements are
defined in [`IMPLEMENTATION_POLICY.md`](IMPLEMENTATION_POLICY.md); this file
documents how to run the measurement and how it relates to the test suites
described in [`TEST_WRITING_GUIDE.md`](TEST_WRITING_GUIDE.md).

## Policy

Production-code line coverage must stay at or above **99.5%** as measured by:

```sh
./scripts/coverage.sh
```

The authoritative rule is in [`IMPLEMENTATION_POLICY.md`](IMPLEMENTATION_POLICY.md)
under "Coverage Policy". In particular:

- The threshold applies to **production code**, not inline `#[cfg(test)]` test
  modules.
- Unit tests and integration tests both contribute to the measured production
  coverage.
- Matrix tests under `tests/matrix/` are black-box POSIX conformance tests and
  do **not** contribute to the coverage number.
- There is no line-level exemption mechanism. If production code is unreachable
  and only exists to satisfy a coverage exception, prefer deleting or reshaping
  it.

## Running Coverage

```sh
./scripts/coverage.sh
```

The script requires the active Rust toolchain to include `llvm-tools-preview`
because it calls `llvm-profdata` and `llvm-cov` from the toolchain sysroot.

The script currently:

1. Clears `target/coverage/`.
2. Builds the library and `meiksh` binary with `-Cinstrument-coverage --cfg coverage`.
3. Runs:

   ```sh
   cargo test --lib --test integration_basic
   ```

4. Merges the generated `.profraw` files into
   `target/coverage/meiksh.profdata`.
5. Prints the full `llvm-cov report` summary for repository source files.
6. Computes and prints the production-only line coverage figure from the real
   `target/debug/meiksh` binary, excluding `#[cfg(test)]` code.

## Artifacts

Coverage output is written under `target/coverage/`:

- `summary.json` — summary-only JSON export from `llvm-cov`.
- `lcov.info` — LCOV for the instrumented test run.
- `prod-lcov.info` — LCOV for the production binary object.
- `files.txt` — annotated per-file coverage from `llvm-cov show`.
- `production-line-summary.txt` — production-only line coverage in human and
  JSON form.

## Test Strategy

Use [`TEST_WRITING_GUIDE.md`](TEST_WRITING_GUIDE.md) when deciding where to add
coverage:

- Prefer colocated unit tests for pure logic, internal helpers, exact syscall
  traces, and error paths that are best expressed through
  `crate::sys::test_support`.
- Prefer integration tests for behavior that needs the real binary, real file
  descriptors, PTYs, signals, child processes, or end-to-end shell semantics.
- Keep tests focused on real behavior. The policy deliberately leaves room for
  a small amount of unreachable defensive code rather than encouraging
  contorted tests.

The coverage script is a production-coverage gate, not a replacement for the
full verification suite. Before merging broad changes, also run the ordinary
Rust tests and, when POSIX behavior is affected, the Markdown matrix tests
documented in [`tests/matrix/MD_TEST_SUITES.md`](../tests/matrix/MD_TEST_SUITES.md).

## Latest Baseline

The most recent run in this workspace reported:

```text
Production-only line coverage: 99.72% (18536/18588)
```

Treat this as an informational baseline only; the value can change with any
production or test edit. The required gate remains the policy threshold in
[`IMPLEMENTATION_POLICY.md`](IMPLEMENTATION_POLICY.md).
