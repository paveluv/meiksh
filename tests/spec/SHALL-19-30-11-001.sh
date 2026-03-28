# SHALL-19-30-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify unset produces no stderr on normal operation.

"$MEIKSH" -c 'FOO=bar; unset FOO' 2>"$TMPDIR/stderr_out"
stderr_content=$(cat "$TMPDIR/stderr_out")
if [ -n "$stderr_content" ]; then
  printf '%s\n' "FAIL: unset wrote to stderr on normal operation: $stderr_content" >&2
  exit 1
fi
exit 0
