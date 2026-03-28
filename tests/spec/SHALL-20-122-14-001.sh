# SHALL-20-122-14-001
# "If the utility utility is invoked, the exit status of time shall be the exit
#  status of utility"
# Verify time forwards the invoked utility's exit status.

"${SHELL:-sh}" -c 'time -p sh -c "exit 0"' 2>/dev/null
if [ "$?" != "0" ]; then
  printf '%s\n' "FAIL: expected exit 0" >&2; exit 1
fi

"${SHELL:-sh}" -c 'time -p sh -c "exit 7"' 2>/dev/null
if [ "$?" != "7" ]; then
  printf '%s\n' "FAIL: expected exit 7, got $?" >&2; exit 1
fi

exit 0
