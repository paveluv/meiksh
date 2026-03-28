# SHALL-20-14-04-009
# "Handle the operand dot-dot logically; symbolic link components shall not be
#  resolved before dot-dot components are processed"
# Verify cd -L handles .. logically (without resolving symlinks).

real="$TMPDIR/shall_20_14_04_009_real_$$"
link="$TMPDIR/shall_20_14_04_009_link_$$"
mkdir -p "$real/sub"
ln -s "$real/sub" "$link"

got=$("${SHELL}" -c '
  cd -L "'"$link"'" 2>/dev/null || exit 1
  cd -L .. 2>/dev/null || exit 1
  printf "%s\n" "$PWD"
')
rm -rf "$real" "$link"

# Logical: PWD after cd -L .. should be parent of the symlink, not parent of real/sub
expected=$(dirname "$link")
if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: cd -L .. gave '$got', expected '$expected'" >&2
  exit 1
fi

exit 0
