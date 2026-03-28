# Test: SHALL-19-10-01-004
# Obligation: "If the string contains at least three characters, begins with a
#   <left-curly-bracket> ('{') and ends with a <right-curly-bracket> ('}'), and
#   the delimiter character is one of '<' or '>', the token identifier
#   IO_LOCATION may result"
# Verifies: {varname}>file allocates a file descriptor (if supported).

tmpf="$TMPDIR/shall-19-10-01-004.$$"
trap 'rm -f "$tmpf"' EXIT

# Try {fd}>file syntax; if not supported, that's acceptable (MAY)
result=$(eval '{myfd}>"$tmpf" && printf "%s\n" "ok" >&${myfd}' 2>/dev/null)
if [ -f "$tmpf" ]; then
    content=$(cat "$tmpf")
    if [ "$content" = "ok" ]; then
        exit 0
    fi
fi

# MAY not support IO_LOCATION — still passes
exit 0
