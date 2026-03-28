# Test: SHALL-19-09-01-06-003
# Obligation: "If the command name does not contain any <slash> characters, the
#   command name shall be searched for using the PATH environment variable ...
#   If the search is unsuccessful, the command shall fail with an exit status of
#   127 and the shell shall write an error message."
# Verifies: PATH search for commands without slash; exit 127 on not found.

# A command found via PATH should execute successfully
result=$(PATH=/usr/bin:/bin printf '%s' works 2>/dev/null)
if [ "$result" != "works" ]; then
    printf '%s\n' "FAIL: PATH search did not find printf" >&2
    exit 1
fi

# A nonexistent command (no slash) should produce exit status 127
__nonexistent_cmd_xyzzy__ 2>/dev/null
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: nonexistent command exited $rc, expected 127" >&2
    exit 1
fi

exit 0
