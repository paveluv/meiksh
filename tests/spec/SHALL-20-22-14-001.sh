# SHALL-20-22-14-001
# "When the -v or -V options are specified, the following exit values shall be
#  returned:"
# Tests both success (0) and failure (>0) for -v/-V.

fail=0

command -v ls >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -v ls expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command -v __nonexistent_99__ >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -v nonexistent expected >0\n' >&2
  fail=1
fi

command -V ls >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -V ls expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command -V __nonexistent_99__ >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -V nonexistent expected >0\n' >&2
  fail=1
fi

exit "$fail"
