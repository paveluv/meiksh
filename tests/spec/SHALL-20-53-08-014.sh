# SHALL-20-53-08-014
# "This variable shall be used by the getopts utility as the index of the next
#  argument to be processed."
# Verify OPTIND is used and updated by getopts.

got=$("${SHELL}" -c '
  OPTIND=1
  getopts "ab" opt -a -b
  printf "%s\n" "$OPTIND"
')
if [ "$got" != "2" ]; then
  printf '%s\n' "FAIL: OPTIND after first getopts should be 2, got '$got'" >&2
  exit 1
fi

got2=$("${SHELL}" -c '
  OPTIND=1
  getopts "ab" opt -a -b
  getopts "ab" opt -a -b
  printf "%s\n" "$OPTIND"
')
if [ "$got2" != "3" ]; then
  printf '%s\n' "FAIL: OPTIND after second getopts should be 3, got '$got2'" >&2
  exit 1
fi

exit 0
