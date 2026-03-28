# SHALL-20-02-11-001
# "The standard error shall be used only for diagnostic messages."

"$MEIKSH" -c 'alias mytest="echo hi"; alias mytest' 2>"$TMPDIR/stderr_out" >/dev/null
stderr_content=$(cat "$TMPDIR/stderr_out")
if [ -n "$stderr_content" ]; then
  printf '%s\n' "FAIL: alias wrote non-diagnostic to stderr: $stderr_content" >&2
  exit 1
fi
exit 0
