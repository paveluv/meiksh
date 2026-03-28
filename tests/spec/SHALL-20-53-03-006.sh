# SHALL-20-53-03-006
# "When getopts successfully parses an option that takes an option-argument [...]
#  the value of OPTIND shall be the integer index of the next element of the
#  parameter list (if any; see OPERANDS below) to be searched for an option
#  character. Index 1 identifies the first element of the parameter list."
# Verify OPTIND after option-argument is correct index (duplicate of 003).

fail=0

OPTIND=1
getopts "b:" opt -b argval next
if [ "$OPTIND" -ne 3 ]; then
  echo "FAIL: OPTIND after -b argval is $OPTIND, expected 3" >&2
  fail=1
fi

exit "$fail"
