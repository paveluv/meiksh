# SHALL-20-02-05-006
# "If no operands are given, all alias definitions shall be written to standard output."

result=$("$MEIKSH" -c '
  alias aa="echo a"
  alias bb="echo b"
  alias
')
case "$result" in
  *aa*) ;;
  *)
    printf '%s\n' "FAIL: alias with no operands missing aa, got: $result" >&2
    exit 1
    ;;
esac
case "$result" in
  *bb*) ;;
  *)
    printf '%s\n' "FAIL: alias with no operands missing bb, got: $result" >&2
    exit 1
    ;;
esac
exit 0
