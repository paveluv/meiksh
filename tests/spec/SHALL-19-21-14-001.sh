# Test: SHALL-19-21-14-001
# Obligation: "If a redirection error occurs, the exit status shall be a value
#   in the range 1-125. Otherwise, exec shall return a zero exit status."

# Successful exec with only redirections returns 0
tmpfile="$TMPDIR/exec_exit_$$.txt"
exec 6>"$tmpfile"
st=$?
exec 6>&-
rm -f "$tmpfile"
if [ "$st" -ne 0 ]; then
    printf '%s\n' "FAIL: exec with successful redirection did not return 0, got $st" >&2
    exit 1
fi

# exec with nonexistent command gives 127
(exec /nonexistent_command_$$ 2>/dev/null)
st=$?
if [ "$st" -ne 127 ]; then
    printf '%s\n' "FAIL: exec of nonexistent command did not return 127, got $st" >&2
    exit 1
fi

exit 0
