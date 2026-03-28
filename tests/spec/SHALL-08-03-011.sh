# SHALL-08-03-011
# "This variable shall represent timezone information."
# Verify TZ is propagated to child processes.

TZ=UTC
export TZ
_val=$(sh -c 'printf "%s" "$TZ"')
if [ "$_val" != "UTC" ]; then
  printf '%s\n' "FAIL: TZ not propagated to child" >&2
  exit 1
fi

exit 0
