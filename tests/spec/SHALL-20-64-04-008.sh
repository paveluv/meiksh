# reviewed: GPT-5.4
# SHALL-20-64-04-008
# "[XSI] Equivalent to -s signal_name."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# -signal_name is equivalent to -s signal_name.

_got1=""
_got2=""
_err1=""
_err2=""

trap '_got1=yes' HUP
_err1=$(kill -HUP $$ 2>&1 >/dev/null)

trap '_got2=yes' HUP
_err2=$(kill -s HUP $$ 2>&1 >/dev/null)

if [ -n "$_err1" ] || [ -n "$_err2" ]; then
  printf '%s\n' "FAIL: equivalent forms wrote stderr: -HUP='$_err1' -s='$_err2'" >&2
  exit 1
fi

if [ "$_got1" != "yes" ]; then
  printf '%s\n' "FAIL: kill -HUP did not deliver SIGHUP" >&2
  exit 1
fi

if [ "$_got2" != "yes" ]; then
  printf '%s\n' "FAIL: kill -s HUP did not deliver SIGHUP" >&2
  exit 1
fi

trap - HUP
exit 0
