# Test: SHALL-19-07-04-009
# Obligation: "All lines of the here-document shall be expanded ... for
#   parameter expansion, command substitution, and arithmetic expansion."
# Duplicate of SHALL-19-07-04-005 — same requirement, same source line.
# Verifies: unquoted heredoc body undergoes expansion.

VAR=test_val
result=$(cat <<EOF
$VAR
$(printf '%s' sub)
$((1+1))
EOF
)
expected='test_val
sub
2'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: heredoc expansion incorrect" >&2
    printf '%s\n' "  got: $result" >&2
    exit 1
fi

exit 0
