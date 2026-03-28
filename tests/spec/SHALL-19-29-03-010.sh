# SHALL-19-29-03-010
# "If an invalid signal name or number is specified, the trap utility shall write
#  a warning message to standard error."

"$MEIKSH" -c 'trap "" BOGUSSIGNAL' 2>"$TMPDIR/stderr_out"
stderr_content=$(cat "$TMPDIR/stderr_out")
if [ -z "$stderr_content" ]; then
  printf '%s\n' "FAIL: trap did not write warning to stderr for invalid signal" >&2
  exit 1
fi
exit 0
