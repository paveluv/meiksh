#!/bin/sh

# Test runner for POSIX compliance suite
# Usage: ./tests/matrix/runner.sh [test-dir]

# Get the absolute path of the matrix directory
MATRIX_DIR="$(cd "$(dirname "$0")" && pwd)"
TEST_DIR="${1:-$MATRIX_DIR/tests}"
FAILED_TESTS=0
PASSED_TESTS=0

echo "Running POSIX Compliance Test Suite..."

if [ ! -d "$TEST_DIR" ]; then
    echo "Test directory $TEST_DIR not found."
    exit 1
fi

for test_script in "$TEST_DIR"/*.sh; do
    if [ ! -f "$test_script" ]; then
        continue
    fi
    
    # test_script could be relative if passed as $1. Make it absolute:
    test_script_abs="$(cd "$(dirname "$test_script")" && pwd)/$(basename "$test_script")"
    
    echo "--- Running $test_script ---"
    
    # Create isolated temporary directory
    TEST_TMP=$(mktemp -d)
    
    # Run test script in isolated subshell
    (
        cd "$TEST_TMP" || exit 1
        
        # Clear environment mostly
        export PATH="/bin:/usr/bin"
        
        # We can pass the target shell to the test scripts
        export TARGET_SHELL="${TARGET_SHELL:-/bin/sh}"
        export MATRIX_DIR="$MATRIX_DIR"
        
        # Source library
        . "$MATRIX_DIR/lib.sh"
        
        # Run test
        . "$test_script_abs"
        
        # Call report if test script forgot to
        report
    )
    
    EXIT_CODE=$?
    
    # Clean up isolated environment
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
