# Test: SHALL-19-06-04-003
# Obligation: "Next, the shell shall treat this as an arithmetic expression and
#   substitute the value of the expression."
# Verifies: arithmetic expression is evaluated and its value substituted.

result=$((2 + 3 * 4))
if [ "$result" != "14" ]; then
    printf '%s\n' "FAIL: \$((2+3*4)) gave '$result', expected '14'" >&2
    exit 1
fi

# Parentheses for grouping
result2=$(( (2 + 3) * 4 ))
if [ "$result2" != "20" ]; then
    printf '%s\n' "FAIL: \$(((2+3)*4)) gave '$result2', expected '20'" >&2
    exit 1
fi

exit 0
