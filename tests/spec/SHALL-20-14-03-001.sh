# SHALL-20-14-03-001
# "The cd utility shall change the working directory of the current shell
#  execution environment"
# Verify cd changes the working directory.

_orig="$PWD"
_dir="$TMPDIR/cd_test_$$"
mkdir -p "$_dir"
cd "$_dir"
_new="$PWD"
cd "$_orig"
rmdir "$_dir"

if [ "$_new" != "$_dir" ]; then
  printf '%s\n' "FAIL: cd did not change working directory (got '$_new', expected '$_dir')" >&2
  exit 1
fi

exit 0
