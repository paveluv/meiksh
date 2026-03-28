# SHALL-19-30-03-002
# "If -v is specified, name refers to a variable name and the shell shall unset it
#  and remove it from the environment. Read-only variables cannot be unset."

result=$("$MEIKSH" -c '
  FOO=bar
  export FOO
  unset -v FOO
  if [ -z "${FOO+set}" ]; then
    printf "unset\n"
  else
    printf "still_set\n"
  fi
')
if [ "$result" != "unset" ]; then
  printf '%s\n' "FAIL: unset -v did not remove variable, got: $result" >&2
  exit 1
fi
exit 0
