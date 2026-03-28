# SHALL-19-03-01-001
# "After a token has been categorized as type TOKEN ... the TOKEN shall be
#  subject to alias substitution if all of the following conditions are true:"
# Verify basic alias substitution occurs for an unquoted command-name TOKEN
# that has a matching alias defined.

fail=0

alias myecho='printf "%s\n"'
result=$(eval 'myecho hello')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: alias substitution did not occur: '$result'" >&2; fail=1; }

unalias myecho 2>/dev/null

exit "$fail"
