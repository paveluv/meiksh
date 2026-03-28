# SHALL-18-01-01-04-021
# "The named file shall be opened with the consequences defined for that
#  file type."
# Verify redirection to /dev/null works (device file opened normally).

result=$("${MEIKSH:-meiksh}" -c 'printf "%s\n" "discarded" > /dev/null; printf "%s\n" "ok"')
if [ "$result" != "ok" ]; then
  printf '%s\n' "FAIL: redirect to /dev/null failed, got '$result'" >&2
  exit 1
fi

exit 0
