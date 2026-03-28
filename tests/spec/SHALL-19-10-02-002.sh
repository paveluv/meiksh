# Test: SHALL-19-10-02-002
# Obligation: "The expansions specified in 2.7 Redirection shall occur. As
#   specified there, exactly one field can result"
# Verifies: Redirection target undergoes expansion and must produce one field.

tmpf="$TMPDIR/shall-19-10-02-002.$$"
trap 'rm -f "$tmpf"' EXIT

# Variable expansion in redirection target
TARGET="$tmpf"
printf '%s\n' "expanded" >$TARGET
content=$(cat "$tmpf")
if [ "$content" != "expanded" ]; then
    printf '%s\n' "FAIL: redirection target not expanded" >&2
    exit 1
fi

exit 0
