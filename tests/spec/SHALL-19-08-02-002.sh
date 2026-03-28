# Test: SHALL-19-08-02-002
# Obligation: "If the command is not found, the exit status shall be 127."
# Verifies: exit status 127 for command not found.

"$SHELL" -c 'nonexistent_cmd_xyzzy_test_127' 2>/dev/null
status=$?
if [ "$status" -ne 127 ]; then
    printf '%s\n' "FAIL: command not found exit status is $status, expected 127" >&2
    exit 1
fi

exit 0
