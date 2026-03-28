# SHALL-20-14-08-020
# "A <colon>-separated list of pathnames that refer to directories. The cd
#  utility shall use this list in its attempt to change the directory ... An
#  empty string in place of a directory pathname represents the current
#  directory. If CDPATH is not set, it shall be treated as if it were an empty
#  string."
# Verify CDPATH search semantics including empty entries.

base="$TMPDIR/shall_20_14_08_020_$$"
mkdir -p "$base/dir1/sub" "$base/dir2/sub"

# CDPATH with multiple entries; first match wins
got=$("${SHELL}" -c '
  CDPATH="'"$base/dir1:$base/dir2"'"
  export CDPATH
  cd sub 2>/dev/null
  pwd -P
')
expected=$(cd "$base/dir1/sub" && pwd -P)
rm -rf "$base"

if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: CDPATH first-match not used, got '$got' expected '$expected'" >&2
  exit 1
fi

# Unset CDPATH = empty string = search cwd only
got2=$("${SHELL}" -c '
  unset CDPATH
  mkdir -p "'"$TMPDIR"'/shall_20_14_08_020b_$$/child"
  cd "'"$TMPDIR"'/shall_20_14_08_020b_$$" 2>/dev/null
  cd child 2>/dev/null
  pwd -P
')
expected2=$(mkdir -p "$TMPDIR/shall_20_14_08_020b_$$/child" && cd "$TMPDIR/shall_20_14_08_020b_$$/child" && pwd -P)
rm -rf "$TMPDIR/shall_20_14_08_020b_$$"

if [ "$got2" != "$expected2" ]; then
  printf '%s\n' "FAIL: unset CDPATH should search cwd, got '$got2'" >&2
  exit 1
fi

exit 0
