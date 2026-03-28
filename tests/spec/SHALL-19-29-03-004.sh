# SHALL-19-29-03-004
# "The environment in which the shell executes a trap action on EXIT shall be
#  identical to the environment immediately after the last command executed before
#  the trap action on EXIT was executed."

result=$("$MEIKSH" -c '
  trap "printf \"%s\" \"\$MYVAR\"" EXIT
  MYVAR=hello
  cd "$TMPDIR"
  true
')
if [ "$result" != "hello" ]; then
  printf '%s\n' "FAIL: EXIT trap did not see last environment, got: $result" >&2
  exit 1
fi
exit 0
