# Test: SHALL-19-21-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid exec (redirections only) produces no stderr
tmpfile="$TMPDIR/exec_stderr_$$.txt"
err=$(exec 5>"$tmpfile" 2>&1; exec 5>&-)
rm -f "$tmpfile"
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid exec produced stderr: $err" >&2
    exit 1
fi

exit 0
