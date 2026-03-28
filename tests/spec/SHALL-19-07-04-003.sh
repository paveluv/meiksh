# Test: SHALL-19-07-04-003
# Obligation: "The delimiter shall be the word itself." (when unquoted)
# Verifies: unquoted here-doc delimiter uses word as-is and body is expanded.

MYVAR=expanded_value
result=$(cat <<EOF
$MYVAR
EOF
)
if [ "$result" != "expanded_value" ]; then
    printf '%s\n' "FAIL: unquoted here-doc did not expand \$MYVAR: got '$result'" >&2
    exit 1
fi

exit 0
