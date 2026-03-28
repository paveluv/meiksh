# Test: SHALL-19-07-04-008
# Obligation: "The removal of <backslash><newline> for line continuation
#   shall be performed during the search for the trailing delimiter."
# Duplicate of SHALL-19-07-04-004 — same requirement, same source line.
# Verifies: backslash-newline line continuation during heredoc delimiter scan.

result=$(cat <<EOF
hello\
EOF
world
EOF
)
expected='helloEOF
world'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: backslash-newline before delimiter was not joined" >&2
    exit 1
fi

exit 0
