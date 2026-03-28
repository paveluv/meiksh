# SHALL-20-22-03-004
# "The command utility shall be treated as a declaration utility if the first
#  argument passed to the utility is recognized as a declaration utility."
# Tilde expansion must occur in assignment context: command export VAR=~/x

fail=0

HOME=/tmp/testhome
export HOME

command export TESTVAR=~/testpath 2>/dev/null
case "$TESTVAR" in
  /tmp/testhome/testpath) ;;
  '~/testpath')
    printf 'FAIL: tilde not expanded in command export assignment\n' >&2
    fail=1
    ;;
  *)
    printf 'FAIL: unexpected TESTVAR value: %s\n' "$TESTVAR" >&2
    fail=1
    ;;
esac

unset TESTVAR

exit "$fail"
