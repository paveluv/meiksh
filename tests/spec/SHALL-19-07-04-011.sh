# Test: SHALL-19-07-04-011
# Obligation: "If the redirection operator is \"<<-\", all leading <tab>
#   characters shall be stripped from input lines ... and from the line
#   containing the trailing delimiter."
# Verifies: <<- strips leading tabs from body and delimiter lines.

result=$(cat <<-EOF
	hello
	world
	EOF
)
expected='hello
world'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: <<- did not strip leading tabs" >&2
    printf '%s\n' "  expected: $expected" >&2
    printf '%s\n' "  got:      $result" >&2
    exit 1
fi

# Spaces should NOT be stripped, only tabs
result2=$(cat <<-EOF
    not_stripped
	EOF
)
if [ "$result2" != "    not_stripped" ]; then
    printf '%s\n' "FAIL: <<- stripped spaces instead of only tabs" >&2
    exit 1
fi

exit 0
