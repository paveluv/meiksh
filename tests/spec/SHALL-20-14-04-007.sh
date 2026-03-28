# SHALL-20-14-04-007
# "The following options shall be supported by the implementation:: -P"
# Verify cd -P is accepted.

_dir="$TMPDIR/cd_optP_$$"
mkdir -p "$_dir"
cd -P "$_dir"
_rc=$?
cd /
rmdir "$_dir"

if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: cd -P returned $_rc" >&2
  exit 1
fi

exit 0
