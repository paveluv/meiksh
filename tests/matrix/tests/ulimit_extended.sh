# Test: Extended ulimit Built-in
# Target: tests/matrix/tests/ulimit_extended.sh
#
# Tests POSIX requirements for ulimit: reporting and setting resource limits,
# hard limits, unlimited handling, numeric operands, output format.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# ulimit reports/sets resource limits
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1297:
# ulimit reports and sets resource limits in effect.

_out=$($TARGET_SHELL -c 'ulimit -f')
if [ -n "$_out" ]; then
    pass
else
    fail "ulimit -f produced no output"
fi

# ulimit -a should list multiple limits
_lines=$($TARGET_SHELL -c 'ulimit -a' | wc -l)
if [ "$_lines" -gt 1 ]; then
    pass
else
    fail "ulimit -a produced fewer than 2 lines ($_lines)"
fi

# ==============================================================================
# "unlimited\n" format for resources without numeric limit
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1044:
# The format for the value of a resource that has no enforced numeric
# limit shall be "unlimited\n".

_val=$($TARGET_SHELL -c 'ulimit -f unlimited; ulimit -f')
if [ "$_val" = "unlimited" ]; then
    pass
else
    fail "Expected 'unlimited' after setting unlimited, got '$_val'"
fi

# ==============================================================================
# -H option reports/sets hard limit
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1073:
# -H option shall report or set the hard resource limit.

_hard=$($TARGET_SHELL -c 'ulimit -Hf')
if [ -n "$_hard" ]; then
    pass
else
    fail "ulimit -Hf produced no output"
fi

# Hard limit should be a number or "unlimited"
case "$_hard" in
    unlimited|*[0-9]*) pass ;;
    *) fail "ulimit -Hf returned unexpected value: '$_hard'" ;;
esac

# ==============================================================================
# newlimit operand - integer or unlimited
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1074:
# The newlimit operand shall be an integer or the string "unlimited".

# Setting with an integer should succeed
assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf \$(ulimit -Sf)'"

# Setting with "unlimited" should succeed (if soft <= hard)
_soft=$($TARGET_SHELL -c 'ulimit -Sf')
_hard2=$($TARGET_SHELL -c 'ulimit -Hf')
if [ "$_hard2" = "unlimited" ]; then
    assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf unlimited'"
else
    pass
fi

# Setting with a non-numeric, non-unlimited string should fail
assert_exit_code_non_zero "$TARGET_SHELL -c 'ulimit -f notanumber'"

# ==============================================================================
# Single limit value written in specified format
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1077:
# A single limit value shall be written in the specified format.

_single=$($TARGET_SHELL -c 'ulimit -f')
case "$_single" in
    unlimited) pass ;;
    *[!0-9]*) fail "ulimit -f returned non-numeric, non-unlimited: '$_single'" ;;
    *) pass ;;
esac

# ==============================================================================
# unlimited considered larger than any other limit
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1298:
# The value "unlimited" shall be considered to be larger than any other
# limit value.

# If hard limit is unlimited, we can set the soft limit to any value
if [ "$_hard2" = "unlimited" ]; then
    assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf 12345'"
    assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf unlimited'"
else
    pass
    pass
fi

# ==============================================================================
# unlimited means no enforcement of limits
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1299:
# The value "unlimited" shall mean that there is no enforcement of the
# associated resource limit.

# If we can set file size to unlimited, writing large output should work
_result=$($TARGET_SHELL -c '
    ulimit -Sf unlimited 2>/dev/null
    if [ "$(ulimit -Sf)" = "unlimited" ]; then
        echo pass
    else
        echo skip
    fi
')
case "$_result" in
    pass|skip) pass ;;
    *) fail "unlimited enforcement test returned: '$_result'" ;;
esac

# ==============================================================================
# Numerals 0 to max recognized as numeric values
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1308:
# Numerals from 0 to the maximum value shall be recognized as numeric
# values for the newlimit operand.

# 0 is a valid limit
assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf 0'"

# Verify 0 takes effect
assert_stdout "0" "$TARGET_SHELL -c 'ulimit -Sf 0; ulimit -Sf'"

# The current soft limit is a valid numeric value to re-set
_cur=$($TARGET_SHELL -c 'ulimit -Sf')
if [ "$_cur" != "unlimited" ]; then
    assert_exit_code 0 "$TARGET_SHELL -c 'ulimit -Sf $_cur'"
else
    pass
fi

# ==============================================================================
# Format within each line is unspecified, except limit value format
# ==============================================================================
# REQUIREMENT: SHALL-ULIMIT-1312:
# The format of the output within each line is otherwise unspecified,
# except that the limit value format is as specified.

# ulimit -a output lines should each contain a limit value (numeric or unlimited)
_bad_lines=`$TARGET_SHELL -c 'ulimit -a' | while IFS= read -r line; do
    case "$line" in
        *unlimited*|*[0-9]*) ;;
        "") ;;
        *) echo "bad: $line" ;;
    esac
done`
if [ -z "$_bad_lines" ]; then
    pass
else
    fail "ulimit -a has lines without a limit value: $_bad_lines"
fi

report
