# Test: SHALL-19-09-01-06-005
# Obligation: "If the command name does not contain any <slash> characters, the
#   command name shall be searched for using the PATH environment variable ...
#   If the search is unsuccessful, the command shall fail with an exit status of
#   127 and the shell shall write an error message."
# Verifies: PATH search for no-slash commands; error message on 127.

# Nonexistent command should write to stderr
msg=$(__nonexistent_cmd_abc123__ 2>&1)
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: expected exit 127, got $rc" >&2
    exit 1
fi
if [ -z "$msg" ]; then
    printf '%s\n' "FAIL: no error message written for unknown command" >&2
    exit 1
fi

exit 0
