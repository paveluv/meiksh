# SHALL-20-14-14-010
# "The following exit values shall be returned:: >1"
# "Both the -e and the -P options are in effect, and an error occurred."
# Verify cd -eP returns >1 on error.

cd -eP /nonexistent_dir_$$ 2>/dev/null
_rc=$?
if [ "$_rc" -le 1 ]; then
  printf '%s\n' "FAIL: cd -eP to nonexistent dir returned $_rc, expected >1" >&2
  exit 1
fi

exit 0
