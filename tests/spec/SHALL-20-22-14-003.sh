# SHALL-20-22-14-003
# "When the -v or -V options are specified, the following exit values shall be
#  returned:: Successful completion."
# Same as 14-002: exit 0 on successful lookup.

fail=0

command -v true >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -v true expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command -V true >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -V true expected 0, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
