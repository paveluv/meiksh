# Test: SHALL-19-06-04-002
# Obligation: "The expression shall be treated as if it were in double-quotes,
#   except that a double-quote inside the expression is not treated specially.
#   The shell shall expand all tokens in the expression for parameter expansion,
#   command substitution, and quote removal."
# Verifies: parameter expansion and command substitution inside arithmetic.

x=10
result=$((x + 5))
if [ "$result" != "15" ]; then
    printf '%s\n' "FAIL: \$((x+5)) with x=10: got '$result'" >&2
    exit 1
fi

# Command substitution inside arithmetic
result2=$(( $(printf '%s' 3) + 4 ))
if [ "$result2" != "7" ]; then
    printf '%s\n' "FAIL: \$((\$(printf 3)+4)): got '$result2'" >&2
    exit 1
fi

exit 0
