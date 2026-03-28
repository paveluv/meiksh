# Test: SHALL-19-09-01-02-006
# Obligation: "If the command name is a function that is not a standard
#   utility implemented as a function, variable assignments shall affect the
#   current execution environment during the execution of the function."
# Verifies: prefix assignments are visible during function execution.

result=$("$SHELL" -c '
myfunc() { printf "%s\n" "$FVAR"; }
FVAR=visible myfunc
')
if [ "$result" != "visible" ]; then
    printf '%s\n' "FAIL: prefix assignment not visible during function" >&2
    exit 1
fi

exit 0
