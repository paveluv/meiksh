# SHALL-20-14-08-018
# "The following environment variables shall affect the execution of cd:: PWD"
# Verify PWD is updated after cd.

_dir="$TMPDIR/cd_pwd_$$"
mkdir -p "$_dir"
cd "$_dir"
_got="$PWD"
cd /
rmdir "$_dir"

if [ "$_got" != "$_dir" ]; then
  printf '%s\n' "FAIL: PWD not updated after cd: got '$_got', expected '$_dir'" >&2
  exit 1
fi

exit 0
