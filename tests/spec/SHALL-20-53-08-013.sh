# SHALL-20-53-08-013
# "This variable shall be used by the getopts utility as the index of the next
#  argument to be processed."
# Verify OPTIND tracks the index of the next argument.

OPTIND=1
getopts "a:b" opt -a val -b
if [ "$OPTIND" -ne 3 ]; then
  printf '%s\n' "FAIL(1): after -a val, OPTIND should be 3, got $OPTIND" >&2
  exit 1
fi

getopts "a:b" opt -a val -b
if [ "$opt" != "b" ]; then
  printf '%s\n' "FAIL(2): next option should be 'b', got '$opt'" >&2
  exit 1
fi

exit 0
