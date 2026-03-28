# SHALL-20-14-08-002
# "The following environment variables shall affect the execution of cd:: CDPATH"
# Verify CDPATH is used to resolve relative directory operands.

base="$TMPDIR/shall_20_14_08_002_$$"
mkdir -p "$base/target"

got=$("${SHELL}" -c '
  CDPATH="'"$base"'"
  export CDPATH
  cd target 2>/dev/null
  pwd -P
')
expected=$(cd "$base/target" && pwd -P)
rm -rf "$base"

if [ "$got" != "$expected" ]; then
  printf '%s\n' "FAIL: CDPATH not used, got '$got' expected '$expected'" >&2
  exit 1
fi

exit 0
