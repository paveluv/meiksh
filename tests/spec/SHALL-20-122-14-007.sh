# SHALL-20-122-14-007
# "If the utility utility is invoked, the exit status of time shall be the
#  exit status of utility; otherwise, the time utility shall exit with one of
#  the following values:: The utility specified by utility could not be found."
# Verify time exits 127 when utility is not found.

"${MEIKSH:-meiksh}" -c 'time nonexistent_cmd_20_122_14_007' 2>/dev/null
rc=$?

if [ "$rc" -ne 127 ]; then
  printf '%s\n' "FAIL: expected exit 127, got $rc" >&2
  exit 1
fi

exit 0
