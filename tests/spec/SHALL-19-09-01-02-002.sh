# Test: SHALL-19-09-01-02-002
# Obligation: "If no command name results, variable assignments shall affect
#   the current execution environment."
# Verifies: assignment-only commands persist in current environment.

result=$("$SHELL" -c '
VAR_TEST=hello
printf "%s\n" "$VAR_TEST"
')
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: no-command assignment did not persist" >&2
    exit 1
fi

# Multiple assignments, no command
result2=$("$SHELL" -c '
A=1 B=2
printf "%s %s\n" "$A" "$B"
')
if [ "$result2" != "1 2" ]; then
    printf '%s\n' "FAIL: multiple no-command assignments did not persist" >&2
    exit 1
fi

exit 0
