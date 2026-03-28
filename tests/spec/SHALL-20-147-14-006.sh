# SHALL-20-147-14-006
# "Otherwise, the wait utility shall exit with one of the following values:: 127"
# wait returns 127 when last pid operand is unknown.

fail=0

wait 99999
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: wait for unknown PID expected 127, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
