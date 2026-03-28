# SHALL-19-03-01-002
# "The TOKEN shall be subject to alias substitution if all of the following
#  conditions are true:: The TOKEN does not contain any quoting characters."
# Verify that quoting the alias name prevents alias substitution.

fail=0

alias myls='printf "%s\n" aliased'

# Unquoted should expand
result=$(eval 'myls')
[ "$result" = "aliased" ] || { printf '%s\n' "FAIL: unquoted alias not expanded: '$result'" >&2; fail=1; }

# Backslash-quoted should NOT expand — should run as command (expect failure)
eval '\myls' >/dev/null 2>&1
[ $? -ne 0 ] || { printf '%s\n' "FAIL: backslash-quoted alias was expanded" >&2; fail=1; }

# Double-quoted should NOT expand
eval '"myls"' >/dev/null 2>&1
[ $? -ne 0 ] || { printf '%s\n' "FAIL: double-quoted alias was expanded" >&2; fail=1; }

unalias myls 2>/dev/null

exit "$fail"
