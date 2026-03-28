# SHALL-20-53-14-006
# "The following exit values shall be returned:: >1"
# Verify getopts returns >1 on processing error (readonly variable).

_rc=$(sh -c 'readonly opt="x"; getopts "a" opt -a 2>/dev/null; echo $?')
if [ "$_rc" -le 1 ]; then
  printf '%s\n' "FAIL: readonly var should cause exit >1, got $_rc" >&2
  exit 1
fi

exit 0
