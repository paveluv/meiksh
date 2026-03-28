# Test: SHALL-19-09-01-01-007
# Obligation: "Redirections shall be performed as described in 2.7 Redirection."
# Duplicate of SHALL-19-09-01-01-004 — same requirement.
# Verifies: redirections performed in simple commands.

f="$TMPDIR/redir_dup_$$"
printf '%s\n' "data" > "$f"
result=$(cat < "$f")
rm -f "$f"
if [ "$result" != "data" ]; then
    printf '%s\n' "FAIL: redirection not performed" >&2
    exit 1
fi

exit 0
