# SHALL-20-62-05-001
# "The following operand shall be supported:"
# Verify jobs accepts job_id operands.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs %1 2>/dev/null
  _rc=$?
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
  exit $_rc
')

if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: jobs should accept job_id operand" >&2
  exit 1
fi

exit 0
