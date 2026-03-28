# SHALL-20-64-04-007
# "The following options shall be supported:: -signal_name"
# Verify: kill accepts -SIGNAL_NAME shorthand (XSI).

sh -c 'sleep 60' &
_pid=$!
sleep 1

kill -TERM "$_pid" 2>/dev/null
sleep 1

if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -TERM did not terminate process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

wait "$_pid" 2>/dev/null
exit 0
