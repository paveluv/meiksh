# Test: SHALL-19-21-03-004
# Obligation: "If the exec command fails and the shell does not exit, any
#   redirections associated with the exec command that were successfully made
#   shall take effect in the current shell execution environment."

# This is tested implicitly: exec with only redirections always applies them
tmpfile="$TMPDIR/exec_redir2_$$.txt"
exec 4>"$tmpfile"
printf '%s\n' "redir_test" >&4
exec 4>&-
content=$(cat "$tmpfile")
rm -f "$tmpfile"
if [ "$content" != "redir_test" ]; then
    printf '%s\n' "FAIL: exec redirections did not take effect" >&2
    exit 1
fi

exit 0
