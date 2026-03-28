# SHALL-20-53-05-002
# "The following operands shall be supported:: optstring"
# Verify getopts accepts the optstring operand as its first argument.

OPTIND=1
getopts "abc" opt -b
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: getopts should accept optstring operand" >&2
  exit 1
fi
if [ "$opt" != "b" ]; then
  printf '%s\n' "FAIL: option should be 'b', got '$opt'" >&2
  exit 1
fi

exit 0
