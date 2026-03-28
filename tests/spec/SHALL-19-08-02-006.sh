# Test: SHALL-19-08-02-006
# Obligation: "If the command is not found, the exit status shall be 127."
# Duplicate of SHALL-19-08-02-002 — same requirement.
# Verifies: exit status 127 for command not found.

"$SHELL" -c 'no_such_command_xyzzy' 2>/dev/null
status=$?
if [ "$status" -ne 127 ]; then
    printf '%s\n' "FAIL: command not found status is $status, expected 127" >&2
    exit 1
fi

exit 0
