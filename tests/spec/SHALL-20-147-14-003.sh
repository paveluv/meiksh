# SHALL-20-147-14-003
# "The wait utility was invoked with no operands and all process IDs known by
#  the invoking shell have terminated."
# Verifies exit 0 when wait is called with no operands after all bg jobs finish.

fail=0

(exit 3) &
(exit 7) &
wait
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: wait (no operands, all done) expected 0, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
