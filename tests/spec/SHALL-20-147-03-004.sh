# SHALL-20-147-03-004
# "Once a process ID that is known in the current shell execution environment
#  has been successfully waited for, it shall be removed from the list of
#  process IDs that are known in the current shell execution environment."

fail=0

# Start a background job and wait for it
(exit 7) &
pid=$!
wait "$pid"

# Waiting again should treat it as unknown → exit 127
wait "$pid"
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: second wait expected 127 (removed PID), got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
