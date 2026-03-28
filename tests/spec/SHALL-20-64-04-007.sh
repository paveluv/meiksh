# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-002
# SHALL-20-64-04-007
# "[XSI] Equivalent to -s signal_name."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill accepts the XSI -signal_name form.

_got=""
_err=""
trap '_got=TERM' TERM
_err=$(kill -TERM $$ 2>&1 >/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: kill -TERM wrote to stderr: '$_err'" >&2
  exit 1
fi

if [ "$_got" != "TERM" ]; then
  printf '%s\n' "FAIL: kill -TERM did not deliver SIGTERM" >&2
  exit 1
fi

trap - TERM
exit 0
