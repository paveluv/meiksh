# Test: SHALL-19-07-003
# Obligation: "all implementations shall support at least 0 to 9, inclusive,
#   for use by the application."
# Verifies: fd numbers 0-9 work in redirections.

f="$TMPDIR/shall_19_07_003_$$"

# fd 3 write
eval "exec 3>'$f'"
printf '%s\n' "fd3" >&3
exec 3>&-
content=$(cat "$f")
if [ "$content" != "fd3" ]; then
    printf '%s\n' "FAIL: fd 3 redirection: got '$content'" >&2
    rm -f "$f"
    exit 1
fi

# fd 9 write
eval "exec 9>'$f'"
printf '%s\n' "fd9" >&9
exec 9>&-
content2=$(cat "$f")
if [ "$content2" != "fd9" ]; then
    printf '%s\n' "FAIL: fd 9 redirection: got '$content2'" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
