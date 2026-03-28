# SHALL-20-14-05-003
# "If directory is an empty string, cd shall write a diagnostic message to
#  standard error and exit with non-zero status."
# Also tests: cd - uses OLDPWD and prints new dir.

# Test empty string operand
_err=$(cd '' 2>&1)
_rc=$?
if [ "$_rc" -eq 0 ]; then
  printf '%s\n' "FAIL: cd '' returned 0, expected non-zero" >&2
  exit 1
fi
if [ -z "$_err" ]; then
  printf '%s\n' "FAIL: cd '' produced no diagnostic on stderr" >&2
  exit 1
fi

# Test cd - uses OLDPWD and prints new dir to stdout
_dir1="$TMPDIR/cd_hyph1_$$"
_dir2="$TMPDIR/cd_hyph2_$$"
mkdir -p "$_dir1" "$_dir2"
cd "$_dir1"
cd "$_dir2"
_out=$(cd - 2>/dev/null)
_got="$PWD"
cd /
rm -rf "$_dir1" "$_dir2"

if [ "$_got" != "$_dir1" ]; then
  printf '%s\n' "FAIL: cd - went to '$_got', expected '$_dir1'" >&2
  exit 1
fi

# stdout should contain the new directory path
case "$_out" in
  *"$_dir1"*) ;;
  *) printf '%s\n' "FAIL: cd - did not print new dir to stdout (got '$_out')" >&2; exit 1 ;;
esac

exit 0
