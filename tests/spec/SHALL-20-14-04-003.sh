# SHALL-20-14-04-003
# "The following options shall be supported by the implementation:: -e"
# Verify cd accepts the -e option without error.

"${MEIKSH:-meiksh}" -c 'cd -e / && exit 0' 2>/dev/null
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd -e / exited $rc, expected 0" >&2
  exit 1
fi

# -e with -P should also be accepted
"${MEIKSH:-meiksh}" -c 'cd -eP / && exit 0' 2>/dev/null
rc=$?
if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd -eP / exited $rc, expected 0" >&2
  exit 1
fi

exit 0
