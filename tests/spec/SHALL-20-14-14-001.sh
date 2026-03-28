# SHALL-20-14-14-001
# "The following exit values shall be returned:"
# Verify cd returns 0 on success and >0 on error.

_dir="$TMPDIR/cd_exit_$$"
mkdir -p "$_dir"
cd "$_dir"
_rc=$?
cd /
rmdir "$_dir"

if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd to valid dir returned $_rc" >&2
  exit 1
fi

cd /nonexistent_dir_$$ 2>/dev/null
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd to nonexistent dir returned 0" >&2
  exit 1
fi

exit 0
