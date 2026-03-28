# SHALL-20-53-14-004
# "The following exit values shall be returned:: 1"
# Verify getopts returns 1 at end of options.

# All options consumed
OPTIND=1
getopts "a" opt -a
getopts "a" opt -a
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(1): end-of-options should return 1, got $_rc" >&2
  exit 1
fi

# -- ends options
OPTIND=1
getopts "a" opt -- foo
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(2): -- should end options (rc=1), got $_rc" >&2
  exit 1
fi

# Non-option argument ends options
OPTIND=1
getopts "a" opt foo
_rc=$?
if [ "$_rc" -ne 1 ]; then
  printf '%s\n' "FAIL(3): non-option arg should end options (rc=1), got $_rc" >&2
  exit 1
fi

exit 0
