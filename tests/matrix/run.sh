#!/bin/sh
# Run matrix test suites via expect_pty.
# With no .md file arguments, runs all suites in tests/matrix/tests/.
# All arguments are forwarded to expect_pty.
# If --shell is not provided, defaults to the debug build.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
tests_dir="$repo_root/tests/matrix/tests"

has_shell=false
has_files=false
for arg in "$@"; do
    case "$arg" in
        --shell)  has_shell=true ;;
        *.md)     has_files=true ;;
    esac
done

if ! "$has_shell"; then
    cargo build --quiet --manifest-path "$repo_root/Cargo.toml"
    set -- --shell "$repo_root/target/debug/meiksh" "$@"
fi

if ! "$has_files"; then
    set -- "$@" "$tests_dir"/*.md
fi

exec cargo run --quiet --manifest-path "$repo_root/Cargo.toml" \
    --bin expect_pty -- "$@"
