# SHALL-20-53-03-015
# "When the end of options is encountered, the getopts utility shall exit with
#  a return value of one; the shell variable OPTIND shall be set to the index
#  of the argument containing the first operand in the parameter list, or the
#  value 1 plus the number of elements in the parameter list if there are no
#  operands in the parameter list; the name variable shall be set to the
#  <question-mark> character."
# Verify end-of-options behavior: exit 1, OPTIND correct, name='?'.

# Test 1: -- terminates options, operand follows
OPTIND=1
getopts "a" opt -a -- foo
getopts "a" opt -a -- foo
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(1): end-of-options should return 1, got $_rc" >&2
  exit 1
fi
if [ "$opt" != "?" ]; then
  printf '%s\n' "FAIL(1): name should be '?' but got '$opt'" >&2
  exit 1
fi
# OPTIND should point to 'foo' (index 3)
if [ "$OPTIND" -ne 3 ]; then
  printf '%s\n' "FAIL(1): OPTIND should be 3, got $OPTIND" >&2
  exit 1
fi

# Test 2: no operands after options -> OPTIND = param count + 1
OPTIND=1
getopts "a" opt -a
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(2): end-of-options should return 1, got $_rc" >&2
  exit 1
fi
# 1 param -> OPTIND should be 2
if [ "$OPTIND" -ne 2 ]; then
  printf '%s\n' "FAIL(2): OPTIND should be 2, got $OPTIND" >&2
  exit 1
fi

# Test 3: non-option argument ends options
OPTIND=1
getopts "a" opt foo
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(3): non-option arg should end options (rc=1), got $_rc" >&2
  exit 1
fi
if [ "$OPTIND" -ne 1 ]; then
  printf '%s\n' "FAIL(3): OPTIND should be 1, got $OPTIND" >&2
  exit 1
fi

exit 0
