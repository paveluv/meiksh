# SHALL-20-53-08-012
# "The following environment variables shall affect the execution of getopts:: OPTIND"
# Verify OPTIND affects getopts execution: tracks next argument index.

# Test 1: OPTIND=1 starts from the beginning
OPTIND=1
getopts "ab" opt -a -b
if [ "$opt" != "a" ]; then
  printf '%s\n' "FAIL(1): first call should get 'a', got '$opt'" >&2
  exit 1
fi

# Test 2: second call advances
getopts "ab" opt -a -b
if [ "$opt" != "b" ]; then
  printf '%s\n' "FAIL(2): second call should get 'b', got '$opt'" >&2
  exit 1
fi

# Test 3: resetting OPTIND=1 restarts parsing
OPTIND=1
getopts "ab" opt -a -b
if [ "$opt" != "a" ]; then
  printf '%s\n' "FAIL(3): reset OPTIND=1 should restart, got '$opt'" >&2
  exit 1
fi

exit 0
