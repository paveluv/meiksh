# SHALL-20-64-04-008
# "[XSI] Equivalent to -s signal_name."
# Verify: kill -HUP is equivalent to kill -s HUP.

_got1=""
_got2=""

trap '_got1=yes' HUP
kill -HUP $$ 2>/dev/null
sleep 1

trap '_got2=yes' HUP
kill -s HUP $$ 2>/dev/null
sleep 1

if [ "$_got1" != "yes" ]; then
  printf '%s\n' "FAIL: kill -HUP did not deliver SIGHUP" >&2
  exit 1
fi

if [ "$_got2" != "yes" ]; then
  printf '%s\n' "FAIL: kill -s HUP did not deliver SIGHUP" >&2
  exit 1
fi

exit 0
