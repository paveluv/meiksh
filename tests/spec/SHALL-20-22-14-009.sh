# SHALL-20-22-14-009
# "Otherwise, the following exit values shall be returned:: 127"
# command returns 127 when command_name is not found.

fail=0

command __absolutely_no_such_command__ 2>/dev/null
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: not-found expected 127, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
