# SHALL-20-53-05-004
# "The following operands shall be supported:: name"
# Verify getopts accepts a name operand as its second argument.

OPTIND=1
getopts "a" myvar -a
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: getopts should accept name operand" >&2
  exit 1
fi
if [ "$myvar" != "a" ]; then
  printf '%s\n' "FAIL: name var should be 'a', got '$myvar'" >&2
  exit 1
fi

exit 0
