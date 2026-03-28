# Test: SHALL-19-02-001
# Obligation: "The application shall quote the following characters if they
#   are to represent themselves:"
#   |  &  ;  <  >  (  )  $  `  \  "  '  <space>  <tab>  <newline>
# Verifies: These characters are special when unquoted, literal when quoted.

# $ is special unquoted (triggers expansion)
X=hello
r=$(printf '%s' $X)
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: unquoted \$ expansion" >&2; exit 1; }

# $ quoted is literal
r=$(printf '%s' '$X')
[ "$r" = '$X' ] || { printf '%s\n' "FAIL: single-quoted \$ not literal" >&2; exit 1; }

# Backslash-escaped special characters
r=$(printf '%s' \|)
[ "$r" = "|" ] || { printf '%s\n' "FAIL: escaped pipe" >&2; exit 1; }

r=$(printf '%s' \;)
[ "$r" = ";" ] || { printf '%s\n' "FAIL: escaped semicolon" >&2; exit 1; }

r=$(printf '%s' \&)
[ "$r" = "&" ] || { printf '%s\n' "FAIL: escaped ampersand" >&2; exit 1; }

# Space inside quotes is literal (no word splitting)
r=$(printf '%s' "a b")
[ "$r" = "a b" ] || { printf '%s\n' "FAIL: quoted space" >&2; exit 1; }

exit 0
