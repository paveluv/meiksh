# SHALL-20-14-14-006
# "The following exit values shall be returned:: >0"
# Verify cd returns >0 on error (nonexistent directory).

cd /nonexistent_dir_$$ 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd to nonexistent dir returned 0, expected >0" >&2
  exit 1
fi

exit 0
