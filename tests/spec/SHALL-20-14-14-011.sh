# SHALL-20-14-14-011
# "The following exit values shall be returned:: Both the -e and the -P options
#  are in effect, and an error occurred."
# Verify cd -eP exits >1 when directory change fails.

"${MEIKSH:-meiksh}" -c 'cd -eP /nonexistent_dir_20_14_14_011' 2>/dev/null
rc=$?
if [ "$rc" -le 1 ]; then
  printf '%s\n' "FAIL: cd -eP to nonexistent dir should exit >1, got $rc" >&2
  exit 1
fi

exit 0
