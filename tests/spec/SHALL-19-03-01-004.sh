# SHALL-19-03-01-004
# "The TOKEN shall be subject to alias substitution if all of the following
#  conditions are true:: An alias with that name is in effect."
# Verify that alias substitution only occurs when an alias is actually defined.

fail=0

# Ensure no alias 'notdefined' exists
unalias notdefined 2>/dev/null

# Should NOT be substituted (no alias) — expect command-not-found error
eval 'notdefined' >/dev/null 2>&1
[ $? -ne 0 ] || { printf '%s\n' "FAIL: non-existent alias somehow succeeded" >&2; fail=1; }

# Now define it and verify substitution occurs
alias notdefined='printf "%s\n" defined'
result=$(eval 'notdefined')
[ "$result" = "defined" ] || { printf '%s\n' "FAIL: alias not expanded after definition: '$result'" >&2; fail=1; }

# Unalias and verify it stops
unalias notdefined 2>/dev/null
eval 'notdefined' >/dev/null 2>&1
[ $? -ne 0 ] || { printf '%s\n' "FAIL: unaliased name still expanded" >&2; fail=1; }

exit "$fail"
