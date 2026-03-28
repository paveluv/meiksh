# Test: SHALL-19-11-021
# Obligation: "Each background job (whether suspended or not) shall have
#   associated with it a job number and a process ID that is known in the
#   current shell execution environment."
# Verifies: $! captures the PID of a background job and it is a valid number.

set -m
sleep 0 &
bg_pid=$!
if [ -z "$bg_pid" ]; then
    printf '%s\n' "FAIL: \$! is empty after background job" >&2
    exit 1
fi
case "$bg_pid" in
    *[!0-9]*) printf '%s\n' "FAIL: \$! is not numeric: $bg_pid" >&2; exit 1 ;;
esac
wait "$bg_pid" 2>/dev/null
exit 0
