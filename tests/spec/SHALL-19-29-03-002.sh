# SHALL-19-29-03-002
# "If action is '-', the shell shall reset each condition to the default value.
#  If action is null (''), the shell shall ignore each specified condition if it arises.
#  Otherwise, the argument action shall be read and executed by the shell when one of
#  the corresponding conditions arises. The action of trap shall override a previous
#  action (either default action or one explicitly set). The value of '$?' after the
#  trap action completes shall be the value it had before the trap action was executed."

# Test 1: action '-' resets to default
result=$("$MEIKSH" -c '
  trap "echo x" USR1
  trap - USR1
  out=$(trap -p USR1)
  [ -z "$out" ] && exit 0
  exit 1
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: trap - did not reset to default" >&2
  exit 1
fi

# Test 2: action '' (null) ignores signal
result=$("$MEIKSH" -c '
  trap "" USR1
  kill -USR1 $$
  exit 0
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: trap with empty action did not ignore signal" >&2
  exit 1
fi

# Test 3: $? preserved across trap action
result=$("$MEIKSH" -c '
  trap "true" USR1
  false
  saved=$?
  kill -USR1 $$
  if [ "$?" -eq "$saved" ]; then
    exit 0
  fi
  exit 1
')
if [ $? -ne 0 ]; then
  printf '%s\n' "FAIL: \$? not preserved across trap action" >&2
  exit 1
fi
exit 0
