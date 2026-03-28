# SHALL-20-14-10-001
# "If a non-empty directory name from CDPATH is used, or if the operand '-' is
#  used... that pathname shall be written to the standard output"
# Verify cd prints new dir to stdout when using CDPATH or '-'.

_base="$TMPDIR/cd_stdout_$$"
mkdir -p "$_base/search/tgt"
_fail=0

# Test CDPATH output
CDPATH="$_base/search"
export CDPATH
_out=$(cd tgt 2>/dev/null)
case "$_out" in
  *"$_base/search/tgt"*) ;;
  *) printf '%s\n' "FAIL: cd via CDPATH did not print dir to stdout" >&2; _fail=1 ;;
esac
unset CDPATH

# Test cd - output
cd "$_base"
cd "$_base/search"
_out=$(cd - 2>/dev/null)
case "$_out" in
  *"$_base"*) ;;
  *) printf '%s\n' "FAIL: cd - did not print dir to stdout" >&2; _fail=1 ;;
esac

cd /
rm -rf "$_base"
exit "$_fail"
