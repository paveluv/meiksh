# Test: SHALL-19-13-010
# Obligation: "Changes made to the subshell environment shall not affect the
#   shell environment. Command substitution, commands that are grouped with
#   parentheses, and asynchronous AND-OR lists shall be executed in a subshell
#   environment."
# Verifies: subshell changes do not propagate; parentheses/cmdsub/$() are subshells.

# Parenthesized group does not affect parent
MY_VAR=parent
(MY_VAR=child)
if [ "$MY_VAR" != "parent" ]; then
    printf '%s\n' "FAIL: parenthesized subshell changed parent var" >&2
    exit 1
fi

# Command substitution does not affect parent
MY_VAR=parent
result=$(MY_VAR=subst; printf '%s\n' "$MY_VAR")
if [ "$MY_VAR" != "parent" ]; then
    printf '%s\n' "FAIL: command substitution changed parent var" >&2
    exit 1
fi
if [ "$result" != "subst" ]; then
    printf '%s\n' "FAIL: command substitution did not see its own var" >&2
    exit 1
fi

# Async list does not affect parent
MY_VAR=parent
MY_VAR=async &
wait $!
if [ "$MY_VAR" != "parent" ]; then
    printf '%s\n' "FAIL: async list changed parent var" >&2
    exit 1
fi

exit 0
