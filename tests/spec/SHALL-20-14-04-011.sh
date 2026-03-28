# SHALL-20-14-04-011
# "If both -L and -P options are specified, the last of these options shall be
#  used and all others ignored. If neither -L nor -P is specified, the operand
#  shall be handled dot-dot logically"
# Verify last option wins and default is logical.

_base="$TMPDIR/cd_lastopt_$$"
mkdir -p "$_base/real" "$_base/parent"
ln -s "$_base/real" "$_base/parent/link"

# -P -L: last is -L, so logical
cd -P -L "$_base/parent/link"
cd ..
_got_logical="$PWD"

# -L -P: last is -P, so physical
cd -L -P "$_base/parent/link"
cd ..
_got_physical="$PWD"

cd /
rm -rf "$_base"

if [ "$_got_logical" != "$_base/parent" ]; then
  printf '%s\n' "FAIL: cd -P -L (last -L) did not use logical: got '$_got_logical'" >&2
  exit 1
fi

if [ "$_got_physical" != "$_base" ]; then
  printf '%s\n' "FAIL: cd -L -P (last -P) did not use physical: got '$_got_physical'" >&2
  exit 1
fi

exit 0
