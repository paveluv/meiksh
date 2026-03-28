# SHALL-20-62-04-006
# "Display only the process IDs for the process group leaders of job-control
#  background jobs"
# Verify jobs -p outputs only PIDs, one per line.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  _result=$(jobs -p 2>/dev/null)
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
  printf "%s\n%s\n" "$_pid" "$_result"
')

_pid=$(printf '%s\n' "$_out" | head -1)
_result=$(printf '%s\n' "$_out" | tail -n +2)

# -p output should be just the PID (numeric), matching the background job PID
case "$_result" in
  *"$_pid"*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs -p should output PID $_pid, got: $_result" >&2
    exit 1
    ;;
esac

# Output should be purely numeric (PID per line)
_non_numeric=$(printf '%s\n' "$_result" | grep -v '^[0-9]*$' || true)
if [ -n "$_non_numeric" ]; then
  printf '%s\n' "FAIL: jobs -p should output only PIDs, got non-numeric: $_non_numeric" >&2
  exit 1
fi

exit 0
