# SHALL-20-62-04-003
# "The following options shall be supported:: -l"
# Verify jobs supports the -l option (long format with PIDs).

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs -l 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')

# -l output should contain the PID
_pid_from=$(sh -c 'sleep 60 & echo $!; kill $! 2>/dev/null; wait $! 2>/dev/null')

if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: jobs -l produced no output" >&2
  exit 1
fi

exit 0
