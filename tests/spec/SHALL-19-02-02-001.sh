# Test: SHALL-19-02-02-001
# Obligation: "Enclosing characters in single-quotes ('') shall preserve the
#   literal value of each character within the single-quotes. A single-quote
#   cannot occur within single-quotes."
# Verifies: Single-quotes preserve all characters literally.

# Special characters are literal inside single-quotes
r=$(printf '%s' '$HOME')
[ "$r" = '$HOME' ] || { printf '%s\n' "FAIL: \$ in single-quotes" >&2; exit 1; }

r=$(printf '%s' '`echo hi`')
[ "$r" = '`echo hi`' ] || { printf '%s\n' "FAIL: backquote in single-quotes" >&2; exit 1; }

r=$(printf '%s' '\n')
[ "$r" = '\n' ] || { printf '%s\n' "FAIL: backslash-n in single-quotes" >&2; exit 1; }

# Glob characters are literal
r=$(printf '%s' '*?[')
[ "$r" = '*?[' ] || { printf '%s\n' "FAIL: glob in single-quotes" >&2; exit 1; }

# Spaces are literal (no word splitting)
r=$(printf '%s' 'a b c')
[ "$r" = 'a b c' ] || { printf '%s\n' "FAIL: spaces in single-quotes" >&2; exit 1; }

# Single-quote cannot occur within single-quotes; use end-escape-restart
r=$(printf '%s' 'can'\''t')
[ "$r" = "can't" ] || { printf '%s\n' "FAIL: embedded single-quote" >&2; exit 1; }

exit 0
