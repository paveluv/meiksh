# Meiksh Benchmark Plan

`meiksh` should not make broad speed claims without a published workload definition, comparison set, and reproducible methodology. This document defines the categories the repository will benchmark as the implementation matures.

## Benchmark Categories

## Startup

- `meiksh -c :`
- `meiksh -c true`
- `meiksh -n script.sh`

## Expansion Heavy

- tight loops using parameter expansion
- arithmetic expansion
- command substitution

## Builtin Heavy

- `cd`, `pwd`, `export`, `unset`, `eval`, and shell-only loops

## Process Launch

- single external command launch
- short pipelines
- redirection-heavy command execution
- background launch overhead

## Real Scripts

- POSIX-oriented configure/build scripts
- portable maintenance scripts
- shell-heavy project tooling

## Competitors

- `dash`
- `bash --posix`
- `yash`
- `mksh`
- platform `sh`

## Metrics

- median wall-clock time
- spread across repeated runs
- user/system CPU time where available
- peak RSS where available
- syscall and branch/cpu-counter data where available

## Methodology

- use fixed benchmark inputs checked into the repository
- separate warm-cache and cold-cache measurements where practical
- keep shell-heavy and external-command-heavy results distinct
- publish raw result files, not just summaries

## Immediate Harness Targets

- shell smoke benchmark scripts under `tests/perf/`
- one driver script that runs `meiksh` against a baseline shell set
- `cargo test` and `cargo run` hooks for local verification
