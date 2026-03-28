# Test: SHALL-19-02-03-009
# Obligation: (Duplicate of SHALL-19-02-03-005) Backquote retains command
#   substitution meaning inside double-quotes.
# Verifies: Backquote command substitution in double-quotes.

r="`printf '%s' test123`"
[ "$r" = "test123" ] || { printf '%s\n' "FAIL: backquote cmd sub" >&2; exit 1; }

exit 0
