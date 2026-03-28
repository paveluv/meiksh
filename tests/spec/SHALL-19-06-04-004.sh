# Test: SHALL-19-06-04-004
# Obligation: "Only signed long integer arithmetic is required."
# Verifies: shell arithmetic handles signed long integer values.

# Basic positive and negative arithmetic
result=$(( -5 + 10 ))
if [ "$result" != "5" ]; then
    printf '%s\n' "FAIL: \$((-5+10)) gave '$result', expected '5'" >&2
    exit 1
fi

# Large values (at least 32-bit signed long)
result2=$((2147483647))
if [ "$result2" != "2147483647" ]; then
    printf '%s\n' "FAIL: max 32-bit signed: got '$result2'" >&2
    exit 1
fi

result3=$((-2147483647))
if [ "$result3" != "-2147483647" ]; then
    printf '%s\n' "FAIL: min 32-bit signed: got '$result3'" >&2
    exit 1
fi

exit 0
