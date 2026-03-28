# SHALL-20-53-14-003
# "An option, specified or unspecified by optstring, was found."
# Verify exit 0 for both recognized and unrecognized options.

# Recognized option
OPTIND=1
getopts "ab" opt -a
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(1): recognized option should return 0, got $_rc" >&2
  exit 1
fi

# Unrecognized option
OPTIND=1
getopts "a" opt -x 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(2): unrecognized option should return 0, got $_rc" >&2
  exit 1
fi

# Missing argument also returns 0
OPTIND=1
getopts "b:" opt -b 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(3): missing argument should return 0, got $_rc" >&2
  exit 1
fi

exit 0
