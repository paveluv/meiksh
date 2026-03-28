# SHALL-20-14-14-002
# "The following exit values shall be returned:: 0"
# Verify cd returns 0 on successful directory change.

_dir="$TMPDIR/cd_exit0_$$"
mkdir -p "$_dir"
cd "$_dir"
_rc=$?
cd /
rmdir "$_dir"

if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd returned $_rc, expected 0" >&2
  exit 1
fi

exit 0
