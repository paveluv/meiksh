# SHALL-20-64-05-003
# "A decimal integer specifying a process or process group to be signaled.
#  The process or processes selected by positive, negative, and zero values
#  of the pid operand shall be as described for the kill() function."
# Verify: positive pid targets specific process; signal 0 with pid 0
#  targets current process group.

# Test 1: positive pid targets a specific process
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: positive pid did not target process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 2: kill -s 0 with pid 0 targets current process group (existence test)
kill -s 0 0 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 0 returned $_rc, expected 0" >&2
  exit 1
fi

exit 0
