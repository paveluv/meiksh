# reviewed: GPT-5.4
# SHALL-20-64-11-001
# "The standard error shall be used only for diagnostic messages."
# Verifies docs/posix/utilities/kill.html#tag_20_64_11:
# successful signal delivery and successful -l output do not write stderr.

trap ':' TERM
_err=$(kill $$ 2>&1 1>/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: kill wrote to stderr on success: '$_err'" >&2
  exit 1
fi

_err=$(kill -l 2>&1 >/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: kill -l wrote to stderr on success: '$_err'" >&2
  exit 1
fi

trap - TERM
exit 0
