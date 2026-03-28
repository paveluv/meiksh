# SHALL-19-28-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify times builtin writes nothing to stderr on success.

result=$("$MEIKSH" -c 'times' 2>"$TMPDIR/stderr_out")
stderr_content=$(cat "$TMPDIR/stderr_out")
if [ -n "$stderr_content" ]; then
  printf '%s\n' "FAIL: times wrote to stderr: $stderr_content" >&2
  exit 1
fi
exit 0
