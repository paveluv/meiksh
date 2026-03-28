# SHALL-20-64-04-005
# "The following options shall be supported:: -s signal_name"
# Verify: kill accepts -s signal_name to specify the signal.

sh -c 'sleep 60' &
_pid=$!
sleep 1

kill -s TERM "$_pid" 2>/dev/null
sleep 1

if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -s TERM did not terminate process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

wait "$_pid" 2>/dev/null
exit 0
