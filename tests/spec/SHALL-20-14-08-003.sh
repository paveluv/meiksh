# SHALL-20-14-08-003
# "A <colon>-separated list of pathnames that refer to directories. The cd
#  utility shall use this list in its attempt to change the directory...
#  If CDPATH is not set, it shall be treated as if it were an empty string."
# Verify CDPATH is used for relative directory resolution.

_base="$TMPDIR/cd_cdpath_$$"
mkdir -p "$_base/searchdir/target"
CDPATH="$_base/searchdir"
export CDPATH

cd target 2>/dev/null
_got="$PWD"
cd /
rm -rf "$_base"
unset CDPATH

if [ "$_got" != "$_base/searchdir/target" ]; then
  printf '%s\n' "FAIL: CDPATH not used: got '$_got'" >&2
  exit 1
fi

exit 0
