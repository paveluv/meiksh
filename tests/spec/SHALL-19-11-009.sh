# Test: SHALL-19-11-009
# Obligation: "When job control is enabled, the shell shall create one or more
#   jobs when it executes a list that has one of the following forms:
#   One or more sequentially executed AND-OR lists followed by at most one
#   asynchronous AND-OR list"
# Verifies: sequential AND-OR lists followed by an async one create jobs.

set -m
true; true; sleep 0 &
bg_pid=$!
if [ -z "$bg_pid" ]; then
    printf '%s\n' "FAIL: no PID from seq+async list" >&2
    exit 1
fi
wait "$bg_pid" 2>/dev/null
exit 0
