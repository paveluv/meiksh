# SHALL-20-22-14-006
# "Otherwise, the following exit values shall be returned:"
# In execution mode: 126 for found-but-not-invokable, 127 for not found,
# or the exit status of the command.

fail=0

# Not found → 127
command __nosuchcmd_xyz__ 2>/dev/null
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: command not-found expected 127, got %d\n' "$rc" >&2
  fail=1
fi

# Found and invoked → pass through exit status
command true
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command true expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command false
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command false expected nonzero, got 0\n' >&2
  fail=1
fi

exit "$fail"
