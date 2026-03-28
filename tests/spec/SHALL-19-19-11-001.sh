# Test: SHALL-19-19-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid dot command produces no stderr
tmpfile="$TMPDIR/dot_stderr_$$.sh"
printf '%s\n' ':' > "$tmpfile"
err=$(. "$tmpfile" 2>&1 >/dev/null)
rm -f "$tmpfile"
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid dot produced stderr: $err" >&2
    exit 1
fi

exit 0
