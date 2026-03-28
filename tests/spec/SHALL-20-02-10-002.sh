# SHALL-20-02-10-002
# "The value string shall be written with appropriate quoting so that it is suitable
#  for reinput to the shell."

result=$("$MEIKSH" -c '
  alias mytest="echo hello world"
  saved=$(alias mytest)
  unalias mytest
  eval "$saved"
  alias mytest
')
case "$result" in
  *mytest*hello*world*) ;;
  *)
    printf '%s\n' "FAIL: alias output not suitable for reinput, got: $result" >&2
    exit 1
    ;;
esac
exit 0
