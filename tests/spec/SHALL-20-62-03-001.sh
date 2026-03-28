# SHALL-20-62-03-001
# "If the current shell execution environment is not a subshell environment,
#  the jobs utility shall display the status of background jobs that were
#  created in the current shell execution environment"
# Verify jobs displays background job status in the main shell environment.

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
    printf '%s\n' "FAIL: jobs should show background job, got: $_out" >&2
    exit 1
    ;;
esac

exit 0
