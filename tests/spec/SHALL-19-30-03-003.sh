# SHALL-19-30-03-003
# "If -f is specified, name refers to a function and the shell shall unset the
#  function definition."

result=$("$MEIKSH" -c '
  myfunc() { printf "hello\n"; }
  unset -f myfunc
  if myfunc 2>/dev/null; then
    printf "still_defined\n"
  else
    printf "unset\n"
  fi
')
if [ "$result" != "unset" ]; then
  printf '%s\n' "FAIL: unset -f did not remove function, got: $result" >&2
  exit 1
fi
exit 0
