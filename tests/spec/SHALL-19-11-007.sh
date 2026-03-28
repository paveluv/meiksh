# Test: SHALL-19-11-007
# Obligation: "When job control is enabled, the shell shall create one or more
#   jobs when it executes a list that has one of the following forms"
# Verifies: running a command with & in a job-control shell creates a job.

set -m
sleep 1 &
bg_pid=$!
if [ -z "$bg_pid" ]; then
    printf '%s\n' "FAIL: no background PID from async command under set -m" >&2
    exit 1
fi
wait "$bg_pid" 2>/dev/null
exit 0
