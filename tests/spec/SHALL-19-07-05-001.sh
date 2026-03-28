# Test: SHALL-19-07-05-001
# Obligation: "[n]<&word shall duplicate one input file descriptor from
#   another, or shall close one. If word evaluates to one or more digits,
#   the file descriptor denoted by n ... shall be made to be a copy of the
#   file descriptor denoted by word; ... If word evaluates to '-', file
#   descriptor n ... shall be closed."
# Verifies: input fd duplication and fd closing via <&.

# Duplicate stdin from fd 3
printf '%s\n' "from_fd3" > "$TMPDIR/fd3_test"
result=$(exec 3< "$TMPDIR/fd3_test"; cat 0<&3)
if [ "$result" != "from_fd3" ]; then
    printf '%s\n' "FAIL: 0<&3 did not duplicate fd 3 to stdin" >&2
    exit 1
fi

# Close stdin with <&-
# This should not error
(exec 0<&-) 2>/dev/null
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: 0<&- should not error" >&2
    exit 1
fi

# Closing an already-closed fd should not be an error
(exec 9<&-) 2>/dev/null
# No error expected

# Duplicating from a non-open fd should produce a redirection error
if (exec 0<&9) 2>/dev/null; then
    printf '%s\n' "FAIL: duplicating from non-open fd 9 should fail" >&2
    exit 1
fi

exit 0
