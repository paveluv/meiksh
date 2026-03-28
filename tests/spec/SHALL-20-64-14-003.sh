# reviewed: GPT-5.4
# Also covers: SHALL-20-64-14-001, SHALL-20-64-14-002
# SHALL-20-64-14-003
# "The -l option was specified and the output specified in STDOUT was
#  successfully written to standard output; or, the -l option was not
#  specified, at least one matching process was found for each pid operand,
#  and the specified signal was successfully processed for at least one
#  matching process."
# Verifies docs/posix/utilities/kill.html#tag_20_64_14:
# kill returns 0 for successful -l output and successful multi-pid signaling.

# Test 1: kill -l returns 0
kill -l >/dev/null 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -l returned $_rc, expected 0" >&2
  exit 1
fi

# Test 2: kill -l 9 returns 0
kill -l 9 >/dev/null 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -l 9 returned $_rc, expected 0" >&2
  exit 1
fi

# Test 3: signal mode returns 0 when each pid operand has a match
sh -c 'sleep 60' &
_pid1=$!
sh -c 'sleep 60' &
_pid2=$!
sleep 1

kill "$_pid1" "$_pid2" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill pid1 pid2 returned $_rc, expected 0" >&2
  kill -9 "$_pid1" "$_pid2" 2>/dev/null
  exit 1
fi

wait "$_pid1" "$_pid2" 2>/dev/null

exit 0
