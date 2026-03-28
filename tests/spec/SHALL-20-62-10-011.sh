# SHALL-20-62-10-011
# "The associated command that was given to the shell."
# Verify jobs output includes the command text.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')

case "$_out" in
  *sleep*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs should show command text 'sleep', got: $_out" >&2
    exit 1
    ;;
esac

exit 0
