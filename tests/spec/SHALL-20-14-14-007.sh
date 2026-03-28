# SHALL-20-14-14-007
# "The following exit values shall be returned:: Either the -e option or the
#  -P option is not in effect, and an error occurred."
# Verify cd exits >0 when directory change fails (without -eP).

"${MEIKSH:-meiksh}" -c 'cd /nonexistent_dir_20_14_14_007' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd to nonexistent dir should exit >0" >&2
  exit 1
fi

# cd "" also errors
"${MEIKSH:-meiksh}" -c 'cd ""' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd '' should exit >0" >&2
  exit 1
fi

exit 0
