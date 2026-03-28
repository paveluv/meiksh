# SHALL-19-29-03-003
# "The EXIT condition shall occur when the shell terminates normally (exits)..."

result=$("$MEIKSH" -c 'trap "printf exittrap" EXIT; exit 0')
if [ "$result" != "exittrap" ]; then
  printf '%s\n' "FAIL: EXIT trap did not fire on normal exit, got: $result" >&2
  exit 1
fi
exit 0
