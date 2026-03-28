# Test: SHALL-19-07-04-001
# Obligation: "The here-document shall be treated as a single word that begins
#   after the next NEWLINE token and continues until there is a line containing
#   only the delimiter and a <newline>."
# Verifies: basic here-document structure and termination.

result=$(cat <<EOF
hello world
EOF
)
if [ "$result" != "hello world" ]; then
    printf '%s\n' "FAIL: here-doc gave '$result', expected 'hello world'" >&2
    exit 1
fi

# Multi-line here-doc
result2=$(cat <<DELIM
line1
line2
DELIM
)
expected=$(printf 'line1\nline2')
if [ "$result2" != "$expected" ]; then
    printf '%s\n' "FAIL: multi-line here-doc incorrect" >&2
    exit 1
fi

exit 0
