# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-002
# SHALL-20-64-04-009
# "[XSI] Specify a non-negative decimal integer, signal_number, representing
#  the signal to be used instead of SIGTERM ..."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill accepts the XSI -signal_number form.

_got=""
trap '_got=TERM' TERM
kill -15 $$ 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -15 returned $_rc, expected 0" >&2
  exit 1
fi

if [ "$_got" != "TERM" ]; then
  printf '%s\n' "FAIL: kill -15 did not deliver SIGTERM" >&2
  exit 1
fi

trap - TERM
exit 0
