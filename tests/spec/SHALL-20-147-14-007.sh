# SHALL-20-147-14-007
# "The process ID specified by the last pid operand specified is not known in
#  the invoking shell execution environment."
# Unknown last PID → exit 127

fail=0

# An unknown PID that was never started
wait 99997
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: unknown PID expected 127, got %d\n' "$rc" >&2
  fail=1
fi

# A PID that was already waited for (now unknown)
(exit 0) &
pid=$!
wait "$pid"
wait "$pid"
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: already-waited PID expected 127, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
