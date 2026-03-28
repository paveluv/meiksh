# Test: SHALL-19-09-03-04-001
# Obligation: "AND-OR lists that are separated by a <semicolon> (';') shall be
#   executed sequentially."
# Verifies: Semicolon-separated lists run sequentially.

tmpf="$TMPDIR/shall-19-09-03-04-001.$$"
trap 'rm -f "$tmpf"' EXIT

printf '%s' "1" >"$tmpf"; printf '%s' "2" >>"$tmpf"; printf '%s' "3" >>"$tmpf"
content=$(cat "$tmpf")
if [ "$content" != "123" ]; then
    printf '%s\n' "FAIL: sequential execution order wrong: got '$content'" >&2
    exit 1
fi

exit 0
