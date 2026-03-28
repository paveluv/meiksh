# Test: SHALL-19-09-04-03-003
# Obligation: "[for without in] shall be equivalent to: for name in "$@""
# Verifies: for without 'in' iterates over positional parameters.

# Use a function to set positional parameters
test_for_noargs() {
    result=""
    for i do
        result="${result}${i}"
    done
    printf '%s' "$result"
}

out=$(test_for_noargs p q r)
if [ "$out" != "pqr" ]; then
    printf '%s\n' "FAIL: for without in should iterate \$@: got '$out'" >&2
    exit 1
fi

exit 0
