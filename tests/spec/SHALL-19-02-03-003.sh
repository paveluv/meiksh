# Test: SHALL-19-02-03-003
# Obligation: "The <dollar-sign> shall retain its special meaning introducing
#   parameter expansion ... but shall not retain its special meaning
#   introducing the dollar-single-quotes form of quoting."
# Also: "The input characters within the quoted string that are also enclosed
#   between '$(' and the matching ')' shall not be affected by the
#   double-quotes."
# Verifies: $'...' is NOT recognized inside double-quotes; $(...) is a
#   nested unquoted context.

# $' inside double-quotes should NOT trigger dollar-single-quote
r=$(printf '%s' "$'hello'")
[ "$r" = "\$'hello'" ] || [ "$r" = "$'hello'" ] || {
    printf '%s\n' "FAIL: \$'...' inside dquotes should be literal, got '$r'" >&2
    exit 1
}

# $(...) inside double-quotes creates a new context (unquoted inside)
r=$(printf '%s' "$(printf '%s' 'a b')")
[ "$r" = "a b" ] || { printf '%s\n' "FAIL: \$() nested context" >&2; exit 1; }

exit 0
