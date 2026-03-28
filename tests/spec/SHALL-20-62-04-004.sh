# SHALL-20-62-04-004
# "(The letter ell.) Provide more information about each job listed."
# Verify jobs -l includes process group ID / PID in output.

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

# The -l output should contain the PID somewhere
case "$_result" in
  *"$_pid"*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs -l should show PID $_pid, got: $_result" >&2
    exit 1
    ;;
esac

exit 0
