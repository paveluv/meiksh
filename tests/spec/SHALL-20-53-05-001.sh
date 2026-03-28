# SHALL-20-53-05-001
# "The following operands shall be supported:"
# Verify getopts accepts the required operands: optstring, name, and optional param.

# Test 1: getopts with optstring and name (minimum required operands)
OPTIND=1
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: getopts with optstring and name should succeed" >&2
  exit 1
fi

# Test 2: getopts with optstring, name, and param operands
OPTIND=1
getopts "a" opt -a extra
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: getopts with optstring, name, and params should succeed" >&2
  exit 1
fi

exit 0
