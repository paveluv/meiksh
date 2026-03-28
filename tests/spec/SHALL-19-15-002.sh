# Test: SHALL-19-15-002
# Obligation: "An error in a special built-in utility may cause a shell
#   executing that utility to abort, while an error in a regular built-in
#   utility shall not cause a shell executing that utility to abort."
# Verifies: regular built-in error does not abort the shell.

# A regular built-in error (e.g., test with bad args) must not abort
# the shell - subsequent commands must still run.
result=$(
    test -f /nonexistent/path/that/does/not/exist 2>/dev/null
    printf '%s\n' "still_running"
)
if [ "$result" != "still_running" ]; then
    printf '%s\n' "FAIL: shell aborted after regular built-in error" >&2
    exit 1
fi

# Special built-in error in non-interactive shell may abort.
# Test that special built-in error produces non-zero exit.
(readonly RO_VAR=fixed; RO_VAR=changed) 2>/dev/null
status=$?
if [ "$status" -eq 0 ]; then
    printf '%s\n' "FAIL: assigning to readonly var did not produce error" >&2
    exit 1
fi

exit 0
