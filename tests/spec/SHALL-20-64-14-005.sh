# SHALL-20-64-14-005
# "The following exit values shall be returned:: An error occurred."
# Verify: kill returns >0 when targeting a nonexistent process.

kill -s TERM 2147483647 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: kill returned 0 for nonexistent pid, expected >0" >&2
  exit 1
fi

exit 0
