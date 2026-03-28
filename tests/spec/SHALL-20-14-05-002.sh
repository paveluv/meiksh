# SHALL-20-14-05-002
# "The following operands shall be supported:: directory"
# Verify cd accepts a directory operand (both absolute and relative).

subdir="$TMPDIR/shall_20_14_05_002_$$"
mkdir -p "$subdir"

# Absolute path
got=$("${SHELL}" -c 'cd "'"$subdir"'" && pwd -P')
real=$(cd "$subdir" && pwd -P)
rm -rf "$subdir"

if [ "$got" != "$real" ]; then
  printf '%s\n' "FAIL: cd with absolute path gave '$got', expected '$real'" >&2
  exit 1
fi

# Relative path
mkdir -p "$TMPDIR/shall_20_14_05_002r_$$/child"
got=$("${SHELL}" -c '
  cd "'"$TMPDIR/shall_20_14_05_002r_$$"'" || exit 1
  cd child || exit 1
  pwd -P
')
real=$(cd "$TMPDIR/shall_20_14_05_002r_$$/child" && pwd -P)
rm -rf "$TMPDIR/shall_20_14_05_002r_$$"

if [ "$got" != "$real" ]; then
  printf '%s\n' "FAIL: cd with relative path gave '$got', expected '$real'" >&2
  exit 1
fi

exit 0
