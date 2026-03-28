# SHALL-20-14-03-003
# "If no directory operand is given and the HOME environment variable is set to
#  a non-empty value, the cd utility shall behave as if the directory named in
#  the HOME environment variable was specified as the directory operand."
# Verify cd with no args goes to $HOME.

_dir="$TMPDIR/cd_home_test_$$"
mkdir -p "$_dir"
HOME="$_dir"
export HOME
cd
_new="$PWD"
rmdir "$_dir" 2>/dev/null

if [ "$_new" != "$_dir" ]; then
  printf '%s\n' "FAIL: cd with no args went to '$_new', expected '$_dir' (HOME)" >&2
  exit 1
fi

exit 0
