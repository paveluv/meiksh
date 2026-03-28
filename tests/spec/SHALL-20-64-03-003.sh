# reviewed: GPT-5.4
# Also covers: SHALL-20-64-03-002, SHALL-20-64-03-005
# SHALL-20-64-03-003
# "The value of the pid operand shall be used as the pid argument."
# Verifies docs/posix/utilities/kill.html#tag_20_64_03:
# the pid operand value is the pid argument to kill().

sh -c 'trap "" TERM; sleep 60' &
_pid1=$!
sh -c 'sleep 60' &
_pid2=$!
sleep 1

kill "$_pid2" 2>/dev/null
sleep 1

if kill -0 "$_pid2" 2>/dev/null; then
  printf '%s\n' "FAIL: pid2 ($_pid2) still alive after kill" >&2
  kill -9 "$_pid1" "$_pid2" 2>/dev/null
  exit 1
fi

if ! kill -0 "$_pid1" 2>/dev/null; then
  printf '%s\n' "FAIL: pid1 ($_pid1) was killed but should not have been" >&2
  exit 1
fi

kill -9 "$_pid1" 2>/dev/null
wait "$_pid1" "$_pid2" 2>/dev/null
exit 0
