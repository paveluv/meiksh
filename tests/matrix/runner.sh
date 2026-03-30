#!/bin/sh

# Test runner for POSIX compliance suite
# Usage: TARGET_SHELL=/path/to/sh ./tests/matrix/runner.sh [test-dir]
#
# Every test runs inside a clean environment (via env -i) so that host
# settings—PS1, LANG, aliases, shell rc files—cannot affect results.

MATRIX_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$MATRIX_DIR/../.." && pwd)"
TEST_DIR="${1:-$MATRIX_DIR/tests}"
FAILED_TESTS=0
PASSED_TESTS=0

TARGET_SHELL="${TARGET_SHELL:-/bin/sh}"

# Resolve the POSIX shell that is running this script.  We reuse it
# (via env -i) to execute each test in a pristine environment.
RUNNER_SHELL="$(command -v sh)"

echo "Building PTY helpers..."
if ! cargo build --quiet --bin pty --bin expect_pty --manifest-path "$REPO_ROOT/Cargo.toml" 2>/dev/null; then
    echo "Failed to build PTY helpers."
    exit 1
fi

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
PTY_BIN="$CARGO_TARGET_DIR/debug/pty"
EXPECT_PTY_BIN="$CARGO_TARGET_DIR/debug/expect_pty"

echo "Running POSIX Compliance Test Suite..."

if [ ! -d "$TEST_DIR" ]; then
    echo "Test directory $TEST_DIR not found."
    exit 1
fi

for test_script in "$TEST_DIR"/*.sh; do
    [ -f "$test_script" ] || continue

    test_script_abs="$(cd "$(dirname "$test_script")" && pwd)/$(basename "$test_script")"

    echo "--- Running $test_script ---"

    TEST_TMP=$(mktemp -d)

    env -i \
        PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin" \
        HOME="$TEST_TMP" \
        TMPDIR="$TEST_TMP" \
        TERM="xterm" \
        LANG="C" \
        LC_ALL="C" \
        PS1='$ ' \
        PS2='> ' \
        ENV="" \
        HISTFILE="/dev/null" \
        TARGET_SHELL="$TARGET_SHELL" \
        MATRIX_DIR="$MATRIX_DIR" \
        PTY_BIN="$PTY_BIN" \
        EXPECT_PTY_BIN="$EXPECT_PTY_BIN" \
        TEST_TMP="$TEST_TMP" \
        "$RUNNER_SHELL" -c '
            cd "$TEST_TMP" || exit 1
            . "$MATRIX_DIR/lib.sh"
            . "$1"
            report
        ' runner "$test_script_abs"

    EXIT_CODE=$?

    rm -rf "$TEST_TMP"

    if [ $EXIT_CODE -eq 0 ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
done

echo ""
echo "=== Test Summary ==="
echo "Passed: $PASSED_TESTS"
echo "Failed: $FAILED_TESTS"

if [ $FAILED_TESTS -gt 0 ]; then
    exit 1
else
    exit 0
fi
