# SHALL-20-02-03-002
# "An alias definition shall affect the current shell execution environment and the
#  execution environments of the subshells of the current shell."

result=$("$MEIKSH" -c '
  alias myalias="echo hello"
  out=$(alias myalias)
  case "$out" in
    *myalias*hello*) printf "ok\n" ;;
    *) printf "fail\n" ;;
  esac
')
if [ "$result" != "ok" ]; then
  printf '%s\n' "FAIL: alias not visible in subshell, got: $result" >&2
  exit 1
fi
exit 0
