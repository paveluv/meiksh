# SHALL-20-62-04-005
# "The following options shall be supported:: -p"
# Verify jobs supports the -p option.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs -p 2>/dev/null
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
')

if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: jobs -p should produce output" >&2
  exit 1
fi

exit 0
