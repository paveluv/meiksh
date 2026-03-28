# SHALL-18-01-01-04-026
# "The named file shall be opened with the consequences defined for that
#  file type."
# (Duplicate of 04-021) Verify device file redirection works.

result=$("${MEIKSH:-meiksh}" -c 'printf "%s\n" "gone" > /dev/null; printf "%s\n" "ok"')
if [ "$result" != "ok" ]; then
  printf '%s\n' "FAIL: redirect to /dev/null failed" >&2
  exit 1
fi

exit 0
