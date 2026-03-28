# Test: SHALL-19-11-010
# Obligation: "For the purposes of job control, a list that includes more than
#   one asynchronous AND-OR list shall be treated as if it were split into
#   multiple separate lists, each ending with an asynchronous AND-OR list."
# Verifies: multiple & in a list create separate background jobs with distinct PIDs.

set -m
sleep 0 & pid1=$!
sleep 0 & pid2=$!
if [ "$pid1" = "$pid2" ]; then
    printf '%s\n' "FAIL: two async lists got same PID ($pid1)" >&2
    exit 1
fi
wait "$pid1" 2>/dev/null
wait "$pid2" 2>/dev/null
exit 0
