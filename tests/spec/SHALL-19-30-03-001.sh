# SHALL-19-30-03-001
# "The unset utility shall unset each variable or function definition specified by
#  name that does not have the readonly attribute and remove any attributes other
#  than readonly that have been given to name."

result=$("$MEIKSH" -c '
  FOO=bar
  export FOO
  unset FOO
  if [ -z "${FOO+set}" ]; then
    printf "unset\n"
  else
    printf "still_set\n"
  fi
')
if [ "$result" != "unset" ]; then
  printf '%s\n' "FAIL: unset did not remove variable, got: $result" >&2
  exit 1
fi
exit 0
