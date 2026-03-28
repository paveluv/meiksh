# SHALL-20-64-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify: kill produces no stderr output on successful signal delivery.

sh -c 'sleep 60' &
_pid=$!
sleep 1

_err=$(kill "$_pid" 2>&1 1>/dev/null)
sleep 1
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: kill wrote to stderr on success: '$_err'" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

wait "$_pid" 2>/dev/null
exit 0
