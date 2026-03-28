# SHALL-20-14-05-001
# "The following operands shall be supported:"
# Verify cd accepts a directory operand.

_dir="$TMPDIR/cd_oper_$$"
mkdir -p "$_dir"
cd "$_dir"
_got="$PWD"
cd /
rmdir "$_dir"

if [ "$_got" != "$_dir" ]; then
  printf '%s\n' "FAIL: cd did not accept directory operand" >&2
  exit 1
fi

exit 0
