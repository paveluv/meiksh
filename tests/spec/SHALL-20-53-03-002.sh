# SHALL-20-53-03-002
# "When the shell is first invoked, the shell variable OPTIND shall be initialized
#  to 1. Each time getopts is invoked, it shall place the value of the next option
#  found in the parameter list in the shell variable specified by the name operand
#  and the shell variable OPTIND shall be set..."
# Verify OPTIND starts at 1 and is updated by getopts.

fail=0

# OPTIND should be 1 at start
if [ "$OPTIND" -ne 1 ]; then
  echo "FAIL: initial OPTIND is $OPTIND, expected 1" >&2
  fail=1
fi

# After parsing one option, OPTIND should change
OPTIND=1
getopts "a" opt -a
saved=$OPTIND
if [ "$saved" -lt 1 ]; then
  echo "FAIL: OPTIND after getopts is $saved, expected >= 1" >&2
  fail=1
fi

exit "$fail"
