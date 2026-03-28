# SHALL-08-03-004
# "The system shall initialize this variable at the time of login to be the
#  user's login name."
# Verify LOGNAME is propagated to child processes.

if [ -z "$LOGNAME" ]; then
  printf '%s\n' "FAIL: LOGNAME is not set" >&2
  exit 1
fi

_val=$(sh -c 'printf "%s" "$LOGNAME"')
if [ "$_val" != "$LOGNAME" ]; then
  printf '%s\n' "FAIL: LOGNAME not propagated to child" >&2
  exit 1
fi

exit 0
