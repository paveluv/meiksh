# SHALL-20-14-04-002
# "The following options shall be supported by the implementation:"
# Verify cd accepts -e, -L, and -P without error.

_dir="$TMPDIR/cd_opts_$$"
mkdir -p "$_dir"
_fail=0

cd -L "$_dir" 2>/dev/null || { printf '%s\n' "FAIL: cd -L rejected" >&2; _fail=1; }
cd -P "$_dir" 2>/dev/null || { printf '%s\n' "FAIL: cd -P rejected" >&2; _fail=1; }
cd -eP "$_dir" 2>/dev/null
_rc=$?
# -eP may return 1 if PWD can't be determined, but should not return >1 for valid dir
if [ "$_rc" -gt 1 ]; then
  printf '%s\n' "FAIL: cd -eP rejected valid dir with exit $_rc" >&2
  _fail=1
fi

cd /
rmdir "$_dir"
exit "$_fail"
