# SHALL-19-29-03-005
# "If action is neither '-' nor the empty string, then each time a matching
#  condition arises, the action shall be executed in a manner equivalent to:
#  eval action"

result=$("$MEIKSH" -c '
  x=0
  trap "x=\$((x+1))" USR1
  kill -USR1 $$
  kill -USR1 $$
  printf "%s\n" "$x"
')
if [ "$result" != "2" ]; then
  printf '%s\n' "FAIL: trap action not executed via eval each time, got: $result" >&2
  exit 1
fi
exit 0
