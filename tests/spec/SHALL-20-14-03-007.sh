# SHALL-20-14-03-007
# "If, during the execution of the above steps, the PWD environment variable
#  is set, the OLDPWD shell variable shall also be set to the value of the old
#  working directory"
# Verify OLDPWD is set to previous directory after cd.

_dir1="$TMPDIR/cd_oldpwd1_$$"
_dir2="$TMPDIR/cd_oldpwd2_$$"
mkdir -p "$_dir1" "$_dir2"
cd "$_dir1"
cd "$_dir2"
_got="$OLDPWD"
cd /
rm -rf "$_dir1" "$_dir2"

if [ "$_got" != "$_dir1" ]; then
  printf '%s\n' "FAIL: OLDPWD='$_got', expected '$_dir1'" >&2
  exit 1
fi

exit 0
