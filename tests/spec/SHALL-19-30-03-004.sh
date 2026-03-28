# SHALL-19-30-03-004
# "If neither -f nor -v is specified, name refers to a variable; if a variable by
#  that name does not exist, it is unspecified whether a function by that name, if
#  any, shall be unset."

result=$("$MEIKSH" -c '
  MYVAR=hello
  unset MYVAR
  if [ -z "${MYVAR+set}" ]; then
    printf "unset\n"
  else
    printf "still_set\n"
  fi
')
if [ "$result" != "unset" ]; then
  printf '%s\n' "FAIL: bare unset did not remove variable, got: $result" >&2
  exit 1
fi
exit 0
