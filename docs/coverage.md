# Coverage

`meiksh` uses Rust's built-in source-based coverage instrumentation so exact coverage can be measured without adding any crate dependencies.

## Run Coverage

```sh
./scripts/coverage.sh
```

This command:

- runs the full `cargo test` suite with coverage instrumentation
- merges raw profiles into `target/coverage/meiksh.profdata`
- prints an `llvm-cov report` summary to stdout for repository source files under `src/`
- prints a production-only line-coverage summary that excludes inline `#[cfg(test)]` modules living in `src/`
- writes machine-readable summary data to `target/coverage/summary.json`
- writes LCOV data to `target/coverage/lcov.info`
- writes per-file annotated coverage text to `target/coverage/files.txt`

## Notes

- `llvm-tools-preview` must be installed in the active Rust toolchain
- the coverage script ignores standard library, registry, and `tests/` sources
- because several unit test modules live inline under `src/`, the script also computes a production-only line-coverage figure that excludes those `#[cfg(test)]` blocks from the final percentage
