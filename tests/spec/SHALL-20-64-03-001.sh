# reviewed: GPT-5.4
# SHALL-20-64-03-001
# "The kill utility shall send a signal to the process or processes specified
#  by each pid operand."
# Verifies docs/posix/utilities/kill.html#tag_20_64_03:
# kill sends a signal to each specified pid operand.

sh -c 'sleep 60' &
_pid1=$!
sh -c 'sleep 60' &
_pid2=$!
sleep 1

kill "$_pid1" "$_pid2" 2>/dev/null
sleep 1

if kill -0 "$_pid1" 2>/dev/null; then
  printf '%s\n' "FAIL: process $_pid1 still alive after kill" >&2
  kill -9 "$_pid1" "$_pid2" 2>/dev/null
  exit 1
fi

if kill -0 "$_pid2" 2>/dev/null; then
  printf '%s\n' "FAIL: process $_pid2 still alive after kill" >&2
  kill -9 "$_pid1" "$_pid2" 2>/dev/null
  exit 1
fi

wait "$_pid1" "$_pid2" 2>/dev/null
exit 0
