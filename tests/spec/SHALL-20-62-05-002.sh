# SHALL-20-62-05-002
# "The following operand shall be supported:: job_id"
# Verify jobs accepts the job_id operand in %n format.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  _result=$(jobs %1 2>/dev/null)
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
  printf "%s" "$_result"
')

case "$_out" in
  *sleep*|*[Rr]unning*)
    ;;
  *)
    printf '%s\n' "FAIL: jobs %%1 should show the specified job, got: $_out" >&2
    exit 1
    ;;
esac

exit 0
