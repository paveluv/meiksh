# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-002
# SHALL-20-64-04-005
# "The following options shall be supported:: -s signal_name"
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill accepts the -s signal_name form.

_got=""
trap '_got=HUP' HUP
_err=$(kill -s HUP $$ 2>&1 >/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: kill -s HUP wrote to stderr: '$_err'" >&2
  exit 1
fi

if [ "$_got" != "HUP" ]; then
  printf '%s\n' "FAIL: kill -s HUP did not deliver SIGHUP" >&2
  exit 1
fi

trap - HUP
exit 0
