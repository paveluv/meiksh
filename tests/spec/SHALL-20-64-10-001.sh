# SHALL-20-64-10-001
# "When the -l option is not specified, the standard output shall not be used."
# Verify: kill in signal mode produces no stdout.

sh -c 'sleep 60' &
_pid=$!
sleep 1

_out=$(kill "$_pid" 2>/dev/null)
if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: kill (signal mode) wrote to stdout: '$_out'" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

wait "$_pid" 2>/dev/null
exit 0
