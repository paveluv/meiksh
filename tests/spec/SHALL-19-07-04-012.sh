# Test: SHALL-19-07-04-012
# Obligation: "If more than one \"<<\" or \"<<-\" operator is specified on a
#   line, the here-document associated with the first operator shall be
#   supplied first by the application and shall be read first by the shell."
# Verifies: multiple heredocs on one line are read in left-to-right order.

result=$(cat <<EOF1; cat <<EOF2
first
EOF1
second
EOF2
)
expected='first
second'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: multiple heredocs not read in order" >&2
    printf '%s\n' "  expected: $expected" >&2
    printf '%s\n' "  got:      $result" >&2
    exit 1
fi

exit 0
