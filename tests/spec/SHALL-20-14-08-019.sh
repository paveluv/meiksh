# SHALL-20-14-08-019
# "This variable shall be set as specified in the DESCRIPTION."
# Verify PWD is set correctly after cd (logical mode).

_dir="$TMPDIR/cd_pwdset_$$"
mkdir -p "$_dir"
cd -L "$_dir"
_got="$PWD"
cd /
rmdir "$_dir"

if [ "$_got" != "$_dir" ]; then
  printf '%s\n' "FAIL: PWD not set correctly in -L mode: '$_got' != '$_dir'" >&2
  exit 1
fi

exit 0
