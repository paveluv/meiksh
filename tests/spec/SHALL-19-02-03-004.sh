# Test: SHALL-19-02-03-004
# Obligation: "` ... The backquote shall retain its special meaning
#   introducing the other form of command substitution."
# Verifies: Backquote introduces command substitution inside double-quotes.

r=$(printf '%s' "`printf '%s' hello`")
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: backquote cmd sub in dquotes" >&2; exit 1; }

exit 0
