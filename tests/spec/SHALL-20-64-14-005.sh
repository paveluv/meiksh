# reviewed: GPT-5.4
# Also covers: SHALL-20-64-14-001, SHALL-20-64-14-004
# SHALL-20-64-14-005
# "An error occurred."
# Verifies docs/posix/utilities/kill.html#tag_20_64_14:
# a concrete kill error path returns a non-zero exit status.

kill -s TERM 2147483647 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: kill returned 0 for nonexistent pid, expected >0" >&2
  exit 1
fi

exit 0
