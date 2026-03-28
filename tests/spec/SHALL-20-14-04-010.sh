# SHALL-20-14-04-010
# "Handle the operand dot-dot physically; symbolic link components shall be
#  resolved before dot-dot components are processed"
# Verify cd -P resolves symlinks before handling .. .

real="$TMPDIR/shall_20_14_04_010_real_$$"
link="$TMPDIR/shall_20_14_04_010_link_$$"
mkdir -p "$real/sub"
ln -s "$real/sub" "$link"

got=$("${MEIKSH:-meiksh}" -c '
  cd -P "'"$link"'" 2>/dev/null || exit 1
  cd -P .. 2>/dev/null || exit 1
  pwd -P
')
rm -rf "$real" "$link"

# Physical: after resolving the symlink, .. goes to parent of real/sub
expected=$(cd "$real" && pwd -P)
if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: cd -P .. gave '$got', expected '$expected'" >&2
  exit 1
fi

exit 0
