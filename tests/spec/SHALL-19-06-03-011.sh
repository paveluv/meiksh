# Test: SHALL-19-06-03-011
# Obligation: "Arithmetic expansion has precedence; that is, the shell shall
#   first determine whether it can parse the expansion as an arithmetic
#   expansion and shall only parse the expansion as a command substitution if
#   it determines that it cannot parse the expansion as an arithmetic
#   expansion."
# Verifies: $(( is parsed as arithmetic first when valid.

# $((expr)) should be arithmetic, not command sub
result=$((2 + 3))
if [ "$result" != "5" ]; then
    printf '%s\n' "FAIL: \$((2+3)) gave '$result', expected '5'" >&2
    exit 1
fi

# $( (subshell) ) with space should be command substitution
result2=$( (printf '%s\n' subshell) )
if [ "$result2" != "subshell" ]; then
    printf '%s\n' "FAIL: \$( (printf subshell) ) gave '$result2'" >&2
    exit 1
fi

exit 0
