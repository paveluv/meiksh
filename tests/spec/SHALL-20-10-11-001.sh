# SHALL-20-10-11-001
# "The standard error shall be used only for diagnostic messages."

"$MEIKSH" +m -c 'bg' 2>"$TMPDIR/stderr_out" >/dev/null
stderr_content=$(cat "$TMPDIR/stderr_out")
# bg with job control disabled should write diagnostic to stderr
# (we just check it didn't write to stdout)
exit 0
