# SHALL-20-53-03-016
# "The shell variables OPTIND and OPTARG shall not be exported by default.
#  An error in setting any of these variables (such as if name has previously
#  been marked readonly) shall be considered an error of getopts processing,
#  and shall result in a return value greater than one."
# Verify OPTIND/OPTARG not exported; readonly name var causes exit >1.

# Test 1: OPTIND should not be exported by default in a fresh shell
_exported=$(sh -c 'export -p' 2>/dev/null | grep 'OPTIND')
if [ -n "$_exported" ]; then
  printf '%s\n' "FAIL: OPTIND should not be exported by default" >&2
  exit 1
fi

# Test 2: OPTARG should not be exported by default
_exported=$(sh -c 'export -p' 2>/dev/null | grep 'OPTARG')
if [ -n "$_exported" ]; then
  printf '%s\n' "FAIL: OPTARG should not be exported by default" >&2
  exit 1
fi

# Test 3: readonly name variable causes getopts to return >1
_rc=$(sh -c 'readonly opt="x"; getopts "a" opt -a 2>/dev/null; echo $?' 2>/dev/null)
if [ "$_rc" -le 1 ]; then
  printf '%s\n' "FAIL: readonly name var should cause exit >1, got $_rc" >&2
  exit 1
fi

exit 0
