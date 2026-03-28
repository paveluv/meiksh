# SHALL-20-53-05-008
# "By default, the list of parameters parsed by the getopts utility shall be
#  the positional parameters currently set in the invoking shell environment.
#  If param operands are given, they shall be parsed instead of the positional
#  parameters."
# Verify default parses positional params; explicit params override.

# Test 1: explicit param operands are parsed instead of positional params
OPTIND=1
set -- -b
getopts "ab" opt -a
if [ "$opt" != "a" ]; then
  printf '%s\n' "FAIL(1): explicit params should override positionals, got '$opt'" >&2
  exit 1
fi

# Test 2: default parses positional parameters
_result=$(sh -c '
  set -- -x
  OPTIND=1
  getopts "x" opt
  printf "%s" "$opt"
')
if [ "$_result" != "x" ]; then
  printf '%s\n' "FAIL(2): default should parse positional params, got '$_result'" >&2
  exit 1
fi

# Test 3: OPTIND with explicit params reflects param count + 1 at end
OPTIND=1
getopts "a" opt -a
getopts "a" opt -a
# OPTIND should be 2 (1 param + 1)
if [ "$OPTIND" -ne 2 ]; then
  printf '%s\n' "FAIL(3): OPTIND should be 2 at end, got $OPTIND" >&2
  exit 1
fi

exit 0
