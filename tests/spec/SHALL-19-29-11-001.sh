# SHALL-19-29-11-001
# "The standard error shall be used only for diagnostic messages and warning
#  messages about invalid signal names or numbers."

# Valid trap should produce no stderr
"$MEIKSH" -c 'trap "echo x" INT; trap -p' 2>"$TMPDIR/stderr_out" >/dev/null
stderr_content=$(cat "$TMPDIR/stderr_out")
if [ -n "$stderr_content" ]; then
  printf '%s\n' "FAIL: trap wrote non-diagnostic output to stderr: $stderr_content" >&2
  exit 1
fi

# Invalid signal should produce stderr
"$MEIKSH" -c 'trap "" FAKESIG' 2>"$TMPDIR/stderr_out2"
stderr_content=$(cat "$TMPDIR/stderr_out2")
if [ -z "$stderr_content" ]; then
  printf '%s\n' "FAIL: trap did not write warning for invalid signal to stderr" >&2
  exit 1
fi
exit 0
