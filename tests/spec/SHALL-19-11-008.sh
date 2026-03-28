# Test: SHALL-19-11-008
# Obligation: "When job control is enabled, the shell shall create one or more
#   jobs when it executes a list that has one of the following forms:
#   A single asynchronous AND-OR list"
# Verifies: a single async AND-OR list creates a background job.

set -m
true && sleep 0 &
bg_pid=$!
if [ -z "$bg_pid" ]; then
    printf '%s\n' "FAIL: no PID from async AND-OR list" >&2
    exit 1
fi
wait "$bg_pid" 2>/dev/null
exit 0
