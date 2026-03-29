#!/bin/sh

# Library for POSIX compliance test suite
# Sourced by individual test scripts

if [ -z "$TEST_NAME" ]; then
    TEST_NAME="$(basename "$0")"
fi

TEST_FAILED=0
TEST_PASSED=0

fail() {
    echo "FAIL: $TEST_NAME: $*" >&2
    TEST_FAILED=$((TEST_FAILED + 1))
}

pass() {
    TEST_PASSED=$((TEST_PASSED + 1))
}

assert_exit_code() {
    expected="$1"
    cmd="$2"
    
    # Run the command and capture exit code
    eval "$cmd" >/dev/null 2>&1
    actual="$?"
    
    if [ "$actual" -eq "$expected" ]; then
        pass
    else
        fail "Expected exit code $expected for '$cmd', got $actual"
    fi
}

assert_exit_code_non_zero() {
    cmd="$1"
    
    # Run the command and capture exit code
    eval "$cmd" >/dev/null 2>&1
    actual="$?"
    
    if [ "$actual" -ne 0 ]; then
        pass
    else
        fail "Expected non-zero exit code for '$cmd', got $actual"
    fi
}

assert_stdout() {
    expected="$1"
    cmd="$2"
    
    actual=$(eval "$cmd" 2>/dev/null)
    
    if [ "$actual" = "$expected" ]; then
        pass
    else
        fail "Expected stdout '$expected' for '$cmd', got '$actual'"
    fi
}

assert_stderr_empty() {
    cmd="$1"

    # We redirect stdout to /dev/null so only stderr is captured
    actual_err=$(eval "$cmd" 2>&1 >/dev/null)

    if [ -z "$actual_err" ]; then
        pass
    else
        fail "Expected empty stderr for '$cmd', got: $actual_err"
    fi
}

assert_stderr_contains() {
    expected_substr="$1"
    cmd="$2"
    
    actual_err=$(eval "$cmd" 2>&1 >/dev/null)
    
    case "$actual_err" in
        *"$expected_substr"*)
            pass
            ;;
        *)
            fail "Expected stderr to contain '$expected_substr' for '$cmd', got '$actual_err'"
            ;;
    esac
}

assert_pty_script() {
    _script="$1"
    _expanded=$(printf '%s\n' "$_script" | sed "s|\$TARGET_SHELL|$TARGET_SHELL|g; s|^spawn |spawn PS1=\$\\\\s |")
    _output=$(printf '%s\n' "$_expanded" | "$MATRIX_DIR/expect_pty" 2>&1)
    _rc=$?
    if [ "$_rc" -eq 0 ]; then
        pass
    else
        fail "PTY script failed (exit $_rc): $_output"
    fi
}

report() {
    if [ "$TEST_FAILED" -gt 0 ]; then
        echo "$TEST_NAME: FAILED ($TEST_FAILED failures, $TEST_PASSED passes)"
        exit 1
    else
        echo "$TEST_NAME: PASSED ($TEST_PASSED assertions)"
        exit 0
    fi
}
