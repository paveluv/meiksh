# SHALL-20-22-14-002
# "When the -v or -V options are specified, the following exit values shall be
#  returned:: 0"
# Exit 0 when command_name is found.

fail=0

command -v ls >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -v ls expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command -V ls >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -V ls expected 0, got %d\n' "$rc" >&2
  fail=1
fi

# Also for builtins and reserved words
command -v export >/dev/null 2>&1
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -v export expected 0, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
