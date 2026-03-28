# reviewed: GPT-5.4
# SHALL-20-64-10-001
# "When the -l option is not specified, the standard output shall not be used."
# Verifies docs/posix/utilities/kill.html#tag_20_64_10:
# successful non--l kill forms do not use stdout.

_out=$(kill -s 0 $$ 2>/dev/null)
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill returned $_rc, expected 0" >&2
  exit 1
fi

if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: kill (signal mode) wrote to stdout: '$_out'" >&2
  exit 1
fi

exit 0
