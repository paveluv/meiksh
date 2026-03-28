# SHALL-20-62-10-001
# "If the -p option is specified, the output shall consist of one line for
#  each process ID:"
# Verify jobs -p outputs one PID per line in "%d\n" format.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  sleep 61 &
  _pid2=$!
  jobs -p 2>/dev/null
  kill "$_pid" "$_pid2" 2>/dev/null
  wait 2>/dev/null
')

# Each line should be a number
_bad=$(printf '%s\n' "$_out" | grep -v '^[0-9][0-9]*$' | grep -v '^$' || true)
if [ -n "$_bad" ]; then
  printf '%s\n' "FAIL: jobs -p should output only PIDs, got non-numeric: $_bad" >&2
  exit 1
fi

# Should have at least 2 lines (2 jobs)
_count=$(printf '%s\n' "$_out" | grep -c '^[0-9]' 2>/dev/null || true)
if [ "$_count" -lt 2 ]; then
  printf '%s\n' "FAIL: jobs -p should output 2 PIDs, got $_count" >&2
  exit 1
fi

exit 0
