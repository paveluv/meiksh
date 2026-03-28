# SHALL-20-53-03-007
# "When getopts reports end of options (that is, when exit status is 1), the value
#  of OPTIND shall be the integer index of the next element of the parameter list
#  (if any)."
# Verify OPTIND at end-of-options with -- terminator.

fail=0

OPTIND=1
# Parse past the -a, then hit --
getopts "a" opt -a -- operand || true
getopts "a" opt -a -- operand
ret=$?
if [ "$ret" -ne 1 ]; then
  echo "FAIL: getopts did not return 1 after -- (got $ret)" >&2
  fail=1
fi
# After -a and --, OPTIND should point to "operand" (index 3)
if [ "$OPTIND" -ne 3 ]; then
  echo "FAIL: OPTIND after -- is $OPTIND, expected 3" >&2
  fail=1
fi

exit "$fail"
