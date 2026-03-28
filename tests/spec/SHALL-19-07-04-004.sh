# Test: SHALL-19-07-04-004
# Obligation: "The removal of <backslash><newline> for line continuation
#   (see 2.2.1 Escape Character (Backslash)) shall be performed during the
#   search for the trailing delimiter."
# Verifies: backslash-newline line continuation is processed during
#   here-document body scan for unquoted delimiter.

# A line ending with \ followed by the delimiter on the next line should
# join those lines, meaning the delimiter is NOT recognized.
result=$(cat <<EOF
hello\
EOF
world
EOF
)
expected='helloEOF
world'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: backslash-newline before delimiter line was not joined" >&2
    printf '%s\n' "  expected: $expected" >&2
    printf '%s\n' "  got:      $result" >&2
    exit 1
fi

exit 0
