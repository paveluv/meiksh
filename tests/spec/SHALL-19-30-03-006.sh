# SHALL-19-30-03-006
# "The unset special built-in shall support XBD 12.2 Utility Syntax Guidelines."
# Verify unset handles -- as end-of-options.

result=$("$MEIKSH" -c '
  MYVAR=hello
  unset -- MYVAR
  if [ -z "${MYVAR+set}" ]; then
    printf "unset\n"
  else
    printf "still_set\n"
  fi
')
if [ "$result" != "unset" ]; then
  printf '%s\n' "FAIL: unset -- did not work correctly, got: $result" >&2
  exit 1
fi
exit 0
