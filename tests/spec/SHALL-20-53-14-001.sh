# SHALL-20-53-14-001
# "The following exit values shall be returned:"
# Verify getopts produces the three defined exit status ranges: 0, 1, >1.

# Test 1: valid option returns 0
OPTIND=1
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL(1): valid option should return 0, got $_rc" >&2
  exit 1
fi

# Test 2: end of options returns 1
OPTIND=1
getopts "a" opt foo
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(2): end-of-options should return 1, got $_rc" >&2
  exit 1
fi

# Test 3: getopts processing error returns >1
_rc=$(sh -c 'readonly opt="x"; getopts "a" opt -a 2>/dev/null; echo $?')
if [ "$_rc" -le 1 ]; then
  printf '%s\n' "FAIL(3): processing error should return >1, got $_rc" >&2
  exit 1
fi

exit 0
