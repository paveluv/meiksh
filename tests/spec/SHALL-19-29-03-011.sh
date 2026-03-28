# SHALL-19-29-03-011
# "The trap special built-in shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify trap handles -- as end-of-options.

result=$("$MEIKSH" -c '
  trap -- "echo caught" USR1
  kill -USR1 $$
')
if [ "$result" != "caught" ]; then
  printf '%s\n' "FAIL: trap did not handle -- correctly, got: $result" >&2
  exit 1
fi
exit 0
