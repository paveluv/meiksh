# SHALL-20-53-05-005
# "The name of a shell variable that shall be set by the getopts utility to the
#  option character that was found."
# Verify name variable is set to the found option character.

# Test 1: valid option sets name to the character
OPTIND=1
getopts "abc" myopt -c
if [ "$myopt" != "c" ]; then
  printf '%s\n' "FAIL(1): name should be 'c', got '$myopt'" >&2
  exit 1
fi

# Test 2: unknown option sets name to '?'
OPTIND=1
getopts "a" myopt -z 2>/dev/null
if [ "$myopt" != "?" ]; then
  printf '%s\n' "FAIL(2): unknown option should set name to '?', got '$myopt'" >&2
  exit 1
fi

# Test 3: end of options sets name to '?'
OPTIND=1
getopts "a" myopt foo
if [ "$myopt" != "?" ]; then
  printf '%s\n' "FAIL(3): end-of-options should set name to '?', got '$myopt'" >&2
  exit 1
fi

# Test 4: missing argument in silent mode sets name to ':'
OPTIND=1
getopts ":b:" myopt -b
if [ "$myopt" != ":" ]; then
  printf '%s\n' "FAIL(4): silent mode missing arg should set name to ':', got '$myopt'" >&2
  exit 1
fi

exit 0
