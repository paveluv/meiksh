# SHALL-20-53-14-005
# "The end of options was encountered."
# Verify exit status 1 means end-of-options: name='?' and OPTIND set correctly.

OPTIND=1
getopts "a" opt -a
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL: expected exit 1 at end, got $_rc" >&2
  exit 1
fi
if [ "$opt" != "?" ]; then
  printf '%s\n' "FAIL: name should be '?' at end, got '$opt'" >&2
  exit 1
fi

exit 0
