# SHALL-20-14-04-008
# "Handle the operand dot-dot physically; symbolic link components shall be
#  resolved before dot-dot components are processed"
# Verify cd -P resolves symlinks before processing ..

_base="$TMPDIR/cd_physical_$$"
mkdir -p "$_base/real" "$_base/parent"
ln -s "$_base/real" "$_base/parent/link"

cd -P "$_base/parent/link"
cd -P ..
_got="$PWD"
cd /
rm -rf "$_base"

# Physical parent of real/ is _base, not _base/parent
if [ "$_got" != "$_base" ]; then
  printf '%s\n' "FAIL: cd -P .. from symlink went to '$_got', expected '$_base'" >&2
  exit 1
fi

exit 0
