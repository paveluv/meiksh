# SHALL-19-29-03-007
# "When a subshell is entered, traps that are not being ignored shall be set to
#  the default actions..."

result=$("$MEIKSH" -c '
  trap "echo parent" USR1
  out=$( trap -p USR1 )
  if [ -z "$out" ]; then
    printf "reset\n"
  else
    printf "kept\n"
  fi
')
if [ "$result" != "reset" ]; then
  printf '%s\n' "FAIL: traps not reset to default in subshell, got: $result" >&2
  exit 1
fi
exit 0
