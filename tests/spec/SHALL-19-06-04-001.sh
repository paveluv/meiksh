# Test: SHALL-19-06-04-001
# Obligation: "The format for arithmetic expansion shall be as follows:
#   $(( expression ))"
# Verifies: arithmetic expansion syntax is recognized.

result=$((1 + 1))
if [ "$result" != "2" ]; then
    printf '%s\n' "FAIL: \$((1+1)) gave '$result', expected '2'" >&2
    exit 1
fi

result2=$((100 - 50))
if [ "$result2" != "50" ]; then
    printf '%s\n' "FAIL: \$((100-50)) gave '$result2', expected '50'" >&2
    exit 1
fi

result3=$((6 * 7))
if [ "$result3" != "42" ]; then
    printf '%s\n' "FAIL: \$((6*7)) gave '$result3', expected '42'" >&2
    exit 1
fi

exit 0
