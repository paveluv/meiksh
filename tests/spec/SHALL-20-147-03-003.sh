# SHALL-20-147-03-003
# "If one or more pid operands are specified that represent known process IDs,
#  the wait utility shall wait until all of them have terminated. If one or more
#  pid operands are specified that represent unknown process IDs, wait shall treat
#  them as if they were known process IDs that exited with exit status 127. The
#  exit status returned by the wait utility shall be the exit status of the
#  process requested by the last pid operand."

fail=0

# Test 1: wait for a known PID returns its exit status
(exit 42) &
pid1=$!
wait "$pid1"
rc=$?
if [ "$rc" -ne 42 ]; then
  printf 'FAIL: wait for known PID expected 42, got %d\n' "$rc" >&2
  fail=1
fi

# Test 2: wait for unknown PID returns 127
wait 99999
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: wait for unknown PID expected 127, got %d\n' "$rc" >&2
  fail=1
fi

# Test 3: last operand determines exit status
(exit 5) &
pid_a=$!
(exit 19) &
pid_b=$!
wait "$pid_a" "$pid_b"
rc=$?
if [ "$rc" -ne 19 ]; then
  printf 'FAIL: wait last-pid exit expected 19, got %d\n' "$rc" >&2
  fail=1
fi

# Test 4: last operand unknown → 127 even if earlier ones are known
(exit 0) &
pid_c=$!
wait "$pid_c" 99998
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: wait known+unknown expected 127, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
