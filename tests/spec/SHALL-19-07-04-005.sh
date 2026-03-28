# Test: SHALL-19-07-04-005
# Obligation: "All lines of the here-document shall be expanded, when the
#   redirection operator is evaluated but after the trailing delimiter for
#   the here-document has been located, for parameter expansion, command
#   substitution, and arithmetic expansion."
# Verifies: unquoted heredoc body undergoes parameter, command, and
#   arithmetic expansion.

VAR=hello
result=$(cat <<EOF
$VAR
$(printf '%s' world)
$((2+3))
EOF
)
expected='hello
world
5'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: heredoc expansion incorrect" >&2
    printf '%s\n' "  expected: $expected" >&2
    printf '%s\n' "  got:      $result" >&2
    exit 1
fi

# Quoted delimiter suppresses expansion
result2=$(cat <<'EOF'
$VAR
$(printf '%s' world)
$((2+3))
EOF
)
expected2='$VAR
$(printf '\''%s'\'' world)
$((2+3))'
if [ "$result2" != "$expected2" ]; then
    printf '%s\n' "FAIL: quoted heredoc should not expand" >&2
    printf '%s\n' "  got: $result2" >&2
    exit 1
fi

exit 0
