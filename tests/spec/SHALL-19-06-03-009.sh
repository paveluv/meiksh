# Test: SHALL-19-06-03-009
# Obligation: "The results of command substitution shall not be processed for
#   further tilde expansion, parameter expansion, command substitution, or
#   arithmetic expansion."
# Verifies: command substitution output is not re-expanded.

# Output containing $ should not be expanded
HOME=/safe
result=$(printf '%s\n' '$HOME')
if [ "$result" != '$HOME' ]; then
    printf '%s\n' "FAIL: \$HOME in cmd sub output was expanded: got '$result'" >&2
    exit 1
fi

# Output containing ~ should not be tilde-expanded
result2=$(printf '%s\n' '~')
if [ "$result2" != '~' ]; then
    printf '%s\n' "FAIL: ~ in cmd sub output was expanded: got '$result2'" >&2
    exit 1
fi

# Output containing $((..)) should not be arith-expanded
result3=$(printf '%s\n' '$((1+1))')
if [ "$result3" != '$((1+1))' ]; then
    printf '%s\n' "FAIL: arith in cmd sub output was expanded: got '$result3'" >&2
    exit 1
fi

exit 0
