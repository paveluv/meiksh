# SHALL-20-62-04-002
# "The following options shall be supported:"
# Verify jobs supports -l and -p options.

_out=$(sh -c '
  sleep 60 &
  _pid=$!
  jobs -l >/dev/null 2>&1
  _r1=$?
  jobs -p >/dev/null 2>&1
  _r2=$?
  kill "$_pid" 2>/dev/null
  wait "$_pid" 2>/dev/null
  if [ "$_r1" -ne 0 ] || [ "$_r2" -ne 0 ]; then
    exit 1
  fi
  exit 0
')

if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: jobs must support -l and -p" >&2
  exit 1
fi

exit 0
