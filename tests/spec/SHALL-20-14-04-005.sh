# SHALL-20-14-04-005
# "The following options shall be supported by the implementation:: -L"
# Verify cd -L is accepted.

_dir="$TMPDIR/cd_optL_$$"
mkdir -p "$_dir"
cd -L "$_dir"
_rc=$?
cd /
rmdir "$_dir"

if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd -L returned $_rc" >&2
  exit 1
fi

exit 0
