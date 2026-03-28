# SHALL-20-147-14-001
# "If one or more operands were specified, all of them have terminated or were
#  not known in the invoking shell execution environment, and the status of the
#  last operand specified is known, then the exit status of wait shall be the
#  status of the last operand specified."

fail=0

# Last operand exit status is returned
(exit 0) &
a=$!
(exit 37) &
b=$!
wait "$a" "$b"
rc=$?
if [ "$rc" -ne 37 ]; then
  printf 'FAIL: expected 37 from last operand, got %d\n' "$rc" >&2
  fail=1
fi

# Single operand
(exit 99) &
c=$!
wait "$c"
rc=$?
if [ "$rc" -ne 99 ]; then
  printf 'FAIL: expected 99, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
