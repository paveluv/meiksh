# SHALL-20-14-04-001
# "The cd utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify cd accepts -- to end option processing.

_dir="$TMPDIR/cd_syntax_$$"
mkdir -p "$_dir"
cd -- "$_dir"
_got="$PWD"
cd /
rmdir "$_dir"

if [ "$_got" != "$_dir" ]; then
  printf '%s\n' "FAIL: cd -- did not work correctly" >&2
  exit 1
fi

exit 0
