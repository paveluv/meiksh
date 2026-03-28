# Test: SHALL-19-09-01-06-006
# Obligation: "If the command name contains at least one <slash>: ... If the
#   named utility does not exist, the command shall fail with an exit status of
#   127 and the shell shall write an error message."
# Verifies: Slash command not found yields 127 with error message.

msg=$(/nonexistent/path/to/cmd 2>&1)
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: expected exit 127, got $rc" >&2
    exit 1
fi
if [ -z "$msg" ]; then
    printf '%s\n' "FAIL: no error message for nonexistent /path command" >&2
    exit 1
fi

exit 0
