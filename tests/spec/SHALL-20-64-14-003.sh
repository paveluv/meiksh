# SHALL-20-64-14-003
# "The -l option was specified and the output specified in STDOUT was
#  successfully written to standard output; or, the -l option was not
#  specified, at least one matching process was found for each pid operand,
#  and the specified signal was successfully processed for at least one
#  matching process."
# Verify: kill returns 0 on success in both -l mode and signal mode.

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

# Test 3: signal mode returns 0 when process exists
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill "$_pid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill pid returned $_rc, expected 0" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
wait "$_pid" 2>/dev/null

exit 0
