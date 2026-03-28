# Test: SHALL-19-09-01-02-011
# Obligation: "If the command name is a function that is not a standard
#   utility implemented as a function, variable assignments shall affect the
#   current execution environment during the execution of the function."
# Duplicate of SHALL-19-09-01-02-006 — same requirement.
# Verifies: prefix assignments visible during function execution.

result=$("$SHELL" -c '
f() { printf "%s\n" "$FV"; }
FV=seen f
')
if [ "$result" != "seen" ]; then
    printf '%s\n' "FAIL: prefix assignment not visible in function" >&2
    exit 1
fi

exit 0
