# SHALL-20-53-14-002
# "The following exit values shall be returned:: 0"
# Verify getopts returns 0 when an option is found.

# Valid option
OPTIND=1
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(1): valid option should return 0, got $_rc" >&2
  exit 1
fi

# Unknown option also returns 0 (application error, not getopts error)
OPTIND=1
getopts "a" opt -z 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(2): unknown option should still return 0, got $_rc" >&2
  exit 1
fi

exit 0
