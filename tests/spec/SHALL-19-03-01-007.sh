# SHALL-19-03-01-007
# "When a TOKEN is subject to alias substitution, the value of the alias shall
#  be processed as if it had been read from the input instead of the TOKEN, with
#  token recognition resuming at the start of the alias value."
# Verify alias value is re-tokenized from its start.

fail=0

alias greet='printf "%s\n" hello'
result=$(eval 'greet')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: alias value not re-tokenized: '$result'" >&2; fail=1; }

# Alias chaining: alias a -> alias b
alias a='b'
alias b='printf "%s\n" chained'
result=$(eval 'a')
[ "$result" = "chained" ] || { printf '%s\n' "FAIL: alias chaining failed: '$result'" >&2; fail=1; }

unalias greet a b 2>/dev/null

exit "$fail"
