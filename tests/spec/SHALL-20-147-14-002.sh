# SHALL-20-147-14-002
# "Otherwise, the wait utility shall exit with one of the following values:: 0"
# wait with no operands returns 0 when all known PIDs have terminated.

fail=0

# Launch a background job, let it finish, then wait with no operands
(exit 0) &
wait
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: wait (no operands) expected 0, got %d\n' "$rc" >&2
  fail=1
fi

# No background jobs at all: wait should return 0
wait
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: wait (no jobs) expected 0, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
