# Test: SHALL-19-09-01-02-005
# Obligation: "If the command name is a special built-in utility, variable
#   assignments shall affect the current execution environment before the
#   utility is executed and remain in effect when the command completes"
# Verifies: prefix assignments to special builtins persist.

result=$("$SHELL" -c '
X=hello eval "printf \"%s\n\" \"\$X\""
printf "%s\n" "$X"
')
expected='hello
hello'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: prefix assignment to special builtin should persist" >&2
    printf '%s\n' "  got: $result" >&2
    exit 1
fi

exit 0
