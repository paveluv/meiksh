# SHALL-20-62-04-007
# "By default, the jobs utility shall display the status of all background jobs,
#  both running and suspended, and all jobs whose status has changed and have
#  not been reported by the shell."
# Verify default jobs output shows running background jobs.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')

case "$_out" in
  *[Rr]unning*|*sleep*)
    ;;
  *)
    printf '%s\n' "FAIL: default jobs should show running job, got: $_out" >&2
    exit 1
    ;;
esac

exit 0
