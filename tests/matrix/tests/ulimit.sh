#!/bin/sh

# Test: ulimit intrinsic utility
# Target: tests/matrix/tests/ulimit.sh
# Tests the POSIX 'ulimit' built-in utility.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-ULIMIT-1072: The ulimit utility shall conform to XBD 12.2
# Utility Syntax Guidelines , except that: The order in which options other than
# -H , -S , and -a are specified may be significant.
# REQUIREMENT: SHALL-ULIMIT-1302: Conforming applications shall specify each
# option separately; that is, grouping option letters (for example, -fH ) need
# not be recognized by all implementations.
# REQUIREMENT: SHALL-ULIMIT-1304: If neither the -H nor -S option is specified:
# If the newlimit operand is present, it shall be used as the new value for both
# the hard and soft limits.
# REQUIREMENT: SHALL-ULIMIT-1305: If the newlimit operand is not present, -S
# shall be the default.
# REQUIREMENT: SHALL-ULIMIT-1306: If no options other than -H or -S are
# specified, the behavior shall be as if the -f option was (also) specified.
# REQUIREMENT: SHALL-ULIMIT-1310: The standard output shall be used when no
# newlimit operand is present.
# REQUIREMENT: SHALL-ULIMIT-1076: If the -a option is specified, the output
# written for each resource shall consist of one line that indicates the
# resource and its limit.

test_cmd='
    ulimit -f unlimited
    echo "pass"
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    ulimit -S -f >/dev/null
    echo "pass"
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# test -a option
test_cmd='
    ulimit -a >/dev/null
    echo $?
'
assert_stdout "0" "$TARGET_SHELL -c '$test_cmd'"

report
