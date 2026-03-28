# SHALL-20-14-03-004
# "The curpath value shall then be converted to canonical form as follows...
#  Dot components and any <slash> characters that separate them from the next
#  component shall be deleted."
# Verify cd canonicalizes dot components in logical mode.

_dir="$TMPDIR/cd_canon_$$"
mkdir -p "$_dir/sub"
cd -L "$_dir/./sub"
_got="$PWD"
cd /
rm -rf "$_dir"

if [ "$_got" != "$_dir/sub" ]; then
  printf '%s\n' "FAIL: cd -L did not canonicalize dot: got '$_got'" >&2
  exit 1
fi

exit 0
