# SHALL-20-132-14-004
# "The following exit values shall be returned: >0 - An error occurred."
# Verify umask returns nonzero on invalid input.

umask 'zzz_invalid' 2>/dev/null
_rc=$?
if [ "$_rc" = "0" ]; then
  printf '%s\n' "FAIL: umask accepted invalid operand 'zzz_invalid'" >&2
  exit 1
fi

exit 0
