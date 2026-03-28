# SHALL-20-14-08-021
# "This variable shall be set as specified in the DESCRIPTION. If an
#  application sets or unsets the value of PWD, the behavior of cd is
#  unspecified."
# Verify cd sets PWD after successful directory change.

got=$("${SHELL}" -c '
  cd /tmp 2>/dev/null
  printf "%s\n" "$PWD"
')
expected=$(cd /tmp && pwd -P)

if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: PWD not set after cd, got '$got' expected '$expected'" >&2
  exit 1
fi

# With -P, PWD should be physical
got2=$("${SHELL}" -c '
  cd -P /tmp 2>/dev/null
  printf "%s\n" "$PWD"
')
if [ "$got2" != "$expected" ]; then
  printf '%s\n' "FAIL: PWD not set after cd -P, got '$got2'" >&2
  exit 1
fi

exit 0
