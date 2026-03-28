# Test: SHALL-19-21-03-001
# Obligation: "If exec is specified with no operands, any redirections
#   associated with the exec command shall be made in the current shell
#   execution environment."

# exec with no operands makes redirections permanent
tmpfile="$TMPDIR/exec_redir_$$.txt"
exec 3>"$tmpfile"
printf '%s\n' "hello" >&3
exec 3>&-
content=$(cat "$tmpfile")
rm -f "$tmpfile"
if [ "$content" != "hello" ]; then
    printf '%s\n' "FAIL: exec redirection did not persist in current env" >&2
    exit 1
fi

exit 0
