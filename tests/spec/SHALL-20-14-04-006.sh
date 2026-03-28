# SHALL-20-14-04-006
# "Handle the operand dot-dot logically; symbolic link components shall not be
#  resolved before dot-dot components are processed"
# Verify cd -L handles .. logically through symlinks.

_base="$TMPDIR/cd_logical_$$"
mkdir -p "$_base/real" "$_base/parent"
ln -s "$_base/real" "$_base/parent/link"

cd -L "$_base/parent/link"
cd -L ..
_got="$PWD"
cd /
rm -rf "$_base"

if [ "$_got" != "$_base/parent" ]; then
  printf '%s\n' "FAIL: cd -L .. from symlink went to '$_got', expected '$_base/parent'" >&2
  exit 1
fi

exit 0
