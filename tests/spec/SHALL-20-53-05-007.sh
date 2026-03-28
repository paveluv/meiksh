# SHALL-20-53-05-007
# "The name of a shell variable that shall be set by the getopts utility to
#  the option character that was found."
# Verify getopts sets the named variable to the found option character.

got=$("${SHELL}" -c '
  OPTIND=1
  getopts "abc" myvar -a
  printf "%s\n" "$myvar"
')
if [ "$got" != "a" ]; then
  printf '%s\n' "FAIL: getopts did not set variable to 'a', got '$got'" >&2
  exit 1
fi

got2=$("${SHELL}" -c '
  OPTIND=1
  getopts "abc" result -c
  printf "%s\n" "$result"
')
if [ "$got2" != "c" ]; then
  printf '%s\n' "FAIL: getopts did not set variable to 'c', got '$got2'" >&2
  exit 1
fi

exit 0
