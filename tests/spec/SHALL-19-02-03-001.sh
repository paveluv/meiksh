# Test: SHALL-19-02-03-001
# Obligation: "Enclosing characters in double-quotes shall preserve the
#   literal value of all characters within the double-quotes, with the
#   exception of the characters backquote, <dollar-sign>, and <backslash>."
# Verifies: Characters other than $, `, \ are literal in double-quotes.

# Glob characters are literal in double-quotes
r=$(printf '%s' "*?[")
[ "$r" = '*?[' ] || { printf '%s\n' "FAIL: glob chars in dquotes" >&2; exit 1; }

# Semicolon is literal
r=$(printf '%s' "a;b")
[ "$r" = 'a;b' ] || { printf '%s\n' "FAIL: semicolon in dquotes" >&2; exit 1; }

# Pipe is literal
r=$(printf '%s' "a|b")
[ "$r" = 'a|b' ] || { printf '%s\n' "FAIL: pipe in dquotes" >&2; exit 1; }

# Single-quote is literal
r=$(printf '%s' "it's")
[ "$r" = "it's" ] || { printf '%s\n' "FAIL: single-quote in dquotes" >&2; exit 1; }

# Spaces: no word splitting
r=$(printf '%s' "a b c")
[ "$r" = 'a b c' ] || { printf '%s\n' "FAIL: spaces in dquotes" >&2; exit 1; }

exit 0
