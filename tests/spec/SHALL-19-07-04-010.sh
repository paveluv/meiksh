# Test: SHALL-19-07-04-010
# Obligation: "Any <backslash> characters in the input shall behave as the
#   <backslash> inside double-quotes. However, the double-quote character
#   shall not be treated specially within a here-document."
# Duplicate of SHALL-19-07-04-006 — same requirement, same source lines.
# Verifies: heredoc backslash/quote handling.

result=$(cat <<EOF
\$dollar
\\slash
\literal
"quotes"
EOF
)
expected='$dollar
\slash
\literal
"quotes"'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: heredoc backslash/quote handling wrong" >&2
    printf '%s\n' "  got: $result" >&2
    exit 1
fi

exit 0
