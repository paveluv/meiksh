# Test: SHALL-19-02-03-002
# Obligation: "$ ... The <dollar-sign> shall retain its special meaning
#   introducing parameter expansion, a form of command substitution, and
#   arithmetic expansion."
# Verifies: $ triggers expansion inside double-quotes.

X=hello
r=$(printf '%s' "$X")
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: param expansion in dquotes" >&2; exit 1; }

r=$(printf '%s' "$(printf '%s' world)")
[ "$r" = "world" ] || { printf '%s\n' "FAIL: cmd sub in dquotes" >&2; exit 1; }

r=$(printf '%s' "$((2+3))")
[ "$r" = "5" ] || { printf '%s\n' "FAIL: arith expansion in dquotes" >&2; exit 1; }

exit 0
