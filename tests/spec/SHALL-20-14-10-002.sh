# SHALL-20-14-10-002
# "If a non-empty directory name from CDPATH is not used, and the directory
#  argument is not '-', there shall be no output."
# Verify cd produces no stdout for normal directory change.

_dir="$TMPDIR/cd_noout_$$"
mkdir -p "$_dir"
unset CDPATH
_out=$(cd "$_dir" 2>/dev/null)
cd /
rmdir "$_dir"

if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: cd produced stdout output: '$_out'" >&2
  exit 1
fi

exit 0
