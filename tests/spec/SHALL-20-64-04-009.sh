# SHALL-20-64-04-009
# "The following options shall be supported:: -signal_number"
# Verify: kill accepts -N numeric signal shorthand (XSI).

sh -c 'sleep 60' &
_pid=$!
sleep 1

kill -15 "$_pid" 2>/dev/null
sleep 1

if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -15 did not terminate process (SIGTERM=15)" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

wait "$_pid" 2>/dev/null
exit 0
