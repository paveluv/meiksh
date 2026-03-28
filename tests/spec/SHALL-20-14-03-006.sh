# SHALL-20-14-03-006
# "The cd utility shall then perform actions equivalent to the chdir() function
#  called with curpath as the path argument. If these actions fail for any
#  reason, the cd utility shall display an appropriate error message"
# Verify cd to nonexistent directory fails with error.

_err=$(cd /nonexistent_dir_$$ 2>&1)
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd to nonexistent dir returned 0" >&2
  exit 1
fi
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: cd to nonexistent dir produced no error message" >&2
  exit 1
fi

exit 0
