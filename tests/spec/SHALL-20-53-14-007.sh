# SHALL-20-53-14-007
# "An error occurred."
# Verify exit >1 is a getopts processing error (distinct from application errors).

# Application error (unknown option) should NOT return >1
OPTIND=1
getopts "a" opt -z 2>/dev/null
_rc=$?
if [ "$_rc" -gt 1 ]; then
  printf '%s\n' "FAIL(1): unknown option should return 0, not >1, got $_rc" >&2
  exit 1
fi

# Getopts processing error (readonly) should return >1
_rc=$(sh -c 'readonly opt="x"; getopts "a" opt -a 2>/dev/null; echo $?')
if [ "$_rc" -le 1 ]; then
  printf '%s\n' "FAIL(2): processing error should return >1, got $_rc" >&2
  exit 1
fi

exit 0
