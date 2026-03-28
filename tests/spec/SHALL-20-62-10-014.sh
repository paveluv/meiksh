# SHALL-20-62-10-014
# "If the -l option is specified:: For job-control background jobs, a field
#  containing the process group ID shall be inserted before the <state> field."
# Verify jobs -l includes the PID/PGID before the state.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  _result=$(jobs -l 2>/dev/null)
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
  printf "%s\n%s\n" "$_pid" "$_result"
')

_pid=$(printf '%s\n' "$_out" | head -1)
_result=$(printf '%s\n' "$_out" | tail -n +2)

# -l output should contain the PID
case "$_result" in
  *"$_pid"*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs -l should show PGID/PID $_pid, got: $_result" >&2
    exit 1
    ;;
esac

exit 0
