# Test: SHALL-19-09-01-01-004
# Obligation: "Redirections shall be performed as described in 2.7 Redirection."
# Verifies: redirections in simple commands are performed.

f="$TMPDIR/redir_step3_$$"
printf '%s\n' "content" > "$f"
result=$(cat < "$f")
rm -f "$f"
if [ "$result" != "content" ]; then
    printf '%s\n' "FAIL: input redirection not performed" >&2
    exit 1
fi

exit 0
