# SHALL-20-22-14-004
# "When the -v or -V options are specified, the following exit values shall be
#  returned:: >0"
# Exit >0 when command_name not found.

fail=0

command -v __nosuchcmd_876__ >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -v nonexistent should exit >0\n' >&2
  fail=1
fi

command -V __nosuchcmd_876__ >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -V nonexistent should exit >0\n' >&2
  fail=1
fi

exit "$fail"
