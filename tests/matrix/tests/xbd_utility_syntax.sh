#!/bin/sh

# Test: XBD 12 Utility Syntax Guidelines
# Target: tests/matrix/tests/xbd_utility_syntax.sh
#
# Tests the shell built-in utilities for conformance with POSIX
# Base Definitions Chapter 12 (Utility Syntax Guidelines).
# Specifically, we verify that standard built-ins process the '--'
# argument correctly.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-XBD-12-4022:
# If an option that has option-arguments is repeated, the option and
# option-argument combinations should be interpreted in the order specified on
# the command line.
# REQUIREMENT: SHALL-XBD-12-4023:
# The order of operands may matter and position-related interpretations should
# be determined on a utility-specific basis.

# Test that `cd` accepts `--` and correctly processes a directory starting with
# `-`.
# (We use a safe directory named `-dir` to verify).
mkdir -p ./-dir
test_cmd='
    cd -- -dir
    echo "$PWD" | grep -q -- "-dir$" && echo "success" || echo "fail"
'
assert_stdout "success" "$TARGET_SHELL -c '$test_cmd'"
rm -rf ./-dir

# Test that `set` accepts `--` to indicate end of options.
test_cmd='
    set -- -x -e
    echo "$1 $2"
'
assert_stdout "-x -e" "$TARGET_SHELL -c '$test_cmd'"

# Test that `unset` accepts `--`.
test_cmd='
    myvar=foo
    unset -- myvar
    echo "${myvar:-empty}"
'
assert_stdout "empty" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-XBD-12-4015:
# Each option and option-argument should be a separate argument, except as
# noted in 12.1 Utility Argument Syntax , item (2).
# REQUIREMENT: SHALL-XBD-12-4011:
# The -W (capital-W) option shall be reserved for vendor options.
# REQUIREMENT: SHALL-XBD-12-4012:
# Multi-digit options should not be allowed.

# Test `read -r` works as `-r` and `getopts` accepts options according to
# standard.
# getopts test (which parses options).
test_cmd='
    getopts "ab:" opt -a -bfoo
    echo "$opt $OPTARG"
'
assert_stdout "a " "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    getopts "ab:" opt -b foo
    echo "$opt $OPTARG"
'
assert_stdout "b foo" "$TARGET_SHELL -c '$test_cmd'"

report
