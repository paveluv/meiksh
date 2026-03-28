# Test: SHALL-19-09-02-004
# Obligation: "If the pipeline is not in the background (see 2.9.3.1 ...),
#   the shell shall wait for the last command specified in the pipeline to
#   complete"
# Verifies: Shell waits for last command in foreground pipeline.

# The exit status of the pipeline should be from the last command
true | false
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: pipeline exit status should be 1 (from false), got $rc" >&2
    exit 1
fi

false | true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: pipeline exit status should be 0 (from true), got $rc" >&2
    exit 1
fi

exit 0
