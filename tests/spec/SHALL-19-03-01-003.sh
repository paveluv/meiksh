# SHALL-19-03-01-003
# "The TOKEN shall be subject to alias substitution if all of the following
#  conditions are true:: The TOKEN is a valid alias name (see XBD 3.10 Alias Name)."
# Verify only valid alias names are subject to substitution.

fail=0

# Valid alias name with alphanumeric + punctuation
alias ll='printf aliased\n'
result=$(eval 'll')
[ "$result" = "aliased" ] || { printf '%s\n' "FAIL: valid alias not expanded: '$result'" >&2; fail=1; }
unalias ll 2>/dev/null

# Token containing '=' is not a valid alias name — should not attempt alias lookup
# (assignment-like tokens are not valid alias names)
eval 'x=1' 2>/dev/null
[ $? -eq 0 ] || { printf '%s\n' "FAIL: token with = caused error" >&2; fail=1; }

exit "$fail"
