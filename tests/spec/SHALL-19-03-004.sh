# Test: SHALL-19-03-004
# Obligation: "If the end of input is recognized, the current token
#   (if any) shall be delimited."
# Verifies: Token at end of input is properly delimited (no trailing
#   newline required).

r=$(printf '%s' 'hello' | eval 'read x; printf "%s" "$x"')
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: EOF delimits token, got '$r'" >&2; exit 1; }

exit 0
