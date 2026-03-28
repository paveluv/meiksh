# SHALL-19-29-03-006
# "Traps shall remain in place for a given shell until explicitly changed with
#  another trap command."

result=$("$MEIKSH" -c '
  trap "printf trapped" USR1
  : noop
  : noop
  kill -USR1 $$
')
if [ "$result" != "trapped" ]; then
  printf '%s\n' "FAIL: trap did not remain in place, got: $result" >&2
  exit 1
fi
exit 0
