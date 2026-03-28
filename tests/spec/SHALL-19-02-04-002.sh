# Test: SHALL-19-02-04-002
# Obligation: '\" yields a <quotation-mark> (double-quote) character,
#   but note that <quotation-mark> can be included unescaped.'
# Verifies: \" in $'...' produces double-quote; unescaped " also works.

r=$'\"'
[ "$r" = '"' ] || { printf '%s\n' "FAIL: \\\" in \$'' should be double-quote" >&2; exit 1; }

r=$'"'
[ "$r" = '"' ] || { printf '%s\n' "FAIL: unescaped \" in \$'' should be double-quote" >&2; exit 1; }

exit 0
