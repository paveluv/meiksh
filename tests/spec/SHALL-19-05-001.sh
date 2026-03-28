# SHALL-19-05-001
# "Parameters can contain arbitrary byte sequences, except for the null byte.
#  The shell shall process their values as characters only when performing
#  operations that are described in this standard in terms of characters."
# Verify parameters can hold arbitrary (non-null) bytes.

fail=0

# Store a string with various bytes and retrieve it unchanged
val='hello world	tab'
[ "$val" = 'hello world	tab' ] || { printf '%s\n' "FAIL: simple string mangled" >&2; fail=1; }

# Newline in variable
nl='line1
line2'
first=$(printf '%s\n' "$nl" | head -1)
[ "$first" = "line1" ] || { printf '%s\n' "FAIL: newline in var: '$first'" >&2; fail=1; }

# High bytes (if locale allows)
val=$(printf '\200\201\202')
result=$(printf '%s' "$val" | wc -c | tr -d ' ')
[ "$result" = "3" ] || { printf '%s\n' "FAIL: high bytes lost, got $result bytes" >&2; fail=1; }

exit "$fail"
