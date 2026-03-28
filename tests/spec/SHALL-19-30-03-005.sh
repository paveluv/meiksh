# SHALL-19-30-03-005
# "Unsetting a variable or function that was not previously set shall not be
#  considered an error and does not cause the shell to abort."

"$MEIKSH" -c '
  unset NEVER_SET_VAR_12345
  unset -f never_set_func_12345
  printf "ok\n"
' >"$TMPDIR/stdout_out" 2>"$TMPDIR/stderr_out"
result=$(cat "$TMPDIR/stdout_out")
if [ "$result" != "ok" ]; then
  printf '%s\n' "FAIL: unsetting non-existent variable/function caused shell to abort" >&2
  exit 1
fi
exit 0
