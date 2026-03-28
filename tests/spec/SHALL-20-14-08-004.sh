# SHALL-20-14-08-004
# "The following environment variables shall affect the execution of cd:: HOME"
# Verify HOME is used as default directory for cd with no args.

_dir="$TMPDIR/cd_home_$$"
mkdir -p "$_dir"
HOME="$_dir"
export HOME
cd
_got="$PWD"
cd /
rmdir "$_dir"

if [ "$_got" != "$_dir" ]; then
  printf '%s\n' "FAIL: cd without args did not go to HOME='$_dir', got '$_got'" >&2
  exit 1
fi

exit 0
