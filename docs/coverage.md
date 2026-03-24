# Coverage

`meiksh` uses Rust's built-in source-based coverage instrumentation so exact coverage can be measured without adding any crate dependencies.

## Run Coverage

```sh
./scripts/coverage.sh
```

This command:

- runs the full `cargo test` suite with coverage instrumentation
- merges raw profiles into `target/coverage/meiksh.profdata`
- prints an `llvm-cov report` summary to stdout
- writes machine-readable summary data to `target/coverage/summary.json`
- writes per-file annotated coverage text to `target/coverage/files.txt`

## Notes

- `llvm-tools-preview` must be installed in the active Rust toolchain
- the coverage script ignores standard library and registry sources so the reported percentage focuses on this repository
