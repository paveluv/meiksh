# Test: SHALL-19-09-04-001
# Obligation: "Each redirection shall apply to all the commands within the
#   compound command that do not explicitly override that redirection."
# Verifies: Compound command redirections apply to inner commands.

tmpf="$TMPDIR/shall-19-09-04-001.$$"
trap 'rm -f "$tmpf"' EXIT

# Redirection on for loop applies to all iterations
for i in a b c; do printf '%s\n' "$i"; done >"$tmpf"
content=$(cat "$tmpf")
expected="a
b
c"
if [ "$content" != "$expected" ]; then
    printf '%s\n' "FAIL: compound redirection not applied to all commands" >&2
    exit 1
fi

# Inner explicit redirect overrides compound redirect
tmpf2="$TMPDIR/shall-19-09-04-001b.$$"
trap 'rm -f "$tmpf" "$tmpf2"' EXIT
{
    printf '%s\n' "outer"
    printf '%s\n' "inner" >"$tmpf2"
} >"$tmpf"
outer=$(cat "$tmpf")
inner=$(cat "$tmpf2")
if [ "$outer" != "outer" ]; then
    printf '%s\n' "FAIL: outer redirect lost" >&2
    exit 1
fi
if [ "$inner" != "inner" ]; then
    printf '%s\n' "FAIL: inner redirect did not override" >&2
    exit 1
fi

exit 0
