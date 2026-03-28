# SHALL-20-14-08-016
# "The following environment variables shall affect the execution of cd:: OLDPWD"
# Verify OLDPWD is used by cd - .

_dir1="$TMPDIR/cd_oldpwd_a_$$"
_dir2="$TMPDIR/cd_oldpwd_b_$$"
mkdir -p "$_dir1" "$_dir2"
cd "$_dir1"
cd "$_dir2"
cd - >/dev/null 2>&1
_got="$PWD"
cd /
rm -rf "$_dir1" "$_dir2"

if [ "$_got" != "$_dir1" ]; then
  printf '%s\n' "FAIL: cd - did not use OLDPWD: got '$_got', expected '$_dir1'" >&2
  exit 1
fi

exit 0
