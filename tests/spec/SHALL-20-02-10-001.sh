# SHALL-20-02-10-001
# "The format for displaying aliases (when no operands or only name operands are
#  specified) shall be: '%s=%s\n', alias-name, value"

result=$("$MEIKSH" -c '
  alias mytest="echo hello"
  alias mytest
')
case "$result" in
  mytest=*hello*) ;;
  *)
    printf '%s\n' "FAIL: alias output format wrong, got: $result" >&2
    exit 1
    ;;
esac
exit 0
