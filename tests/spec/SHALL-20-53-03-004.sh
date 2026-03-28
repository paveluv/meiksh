# SHALL-20-53-03-004
# "When getopts reports end of options (that is, when exit status is 1), the value
#  of OPTIND shall be the integer index of the next element of the parameter list
#  (if any)."
# Verify OPTIND is set to the index of the first non-option arg at end of options.

fail=0

OPTIND=1
getopts "a" opt -a operand1 operand2 || true
# After -a is consumed, next getopts should report end-of-options
getopts "a" opt -a operand1 operand2
ret=$?
if [ "$ret" -ne 1 ]; then
  echo "FAIL: getopts did not return 1 at end of options (got $ret)" >&2
  fail=1
fi
# OPTIND should point to "operand1" which is index 2
if [ "$OPTIND" -ne 2 ]; then
  echo "FAIL: OPTIND at end-of-options is $OPTIND, expected 2" >&2
  fail=1
fi

exit "$fail"
