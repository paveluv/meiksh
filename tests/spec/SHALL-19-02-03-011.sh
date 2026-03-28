# Test: SHALL-19-02-03-011
# Obligation: "When double-quotes are used to quote a parameter expansion,
#   command substitution, or arithmetic expansion, the literal value of all
#   characters within the result of the expansion shall be preserved."
# Verifies: Expansion results in double-quotes are not split or globbed.

X='* foo bar'
set -- "$X"
[ "$#" = "1" ] || { printf '%s\n' "FAIL: dquoted param should be 1 field, got $#" >&2; exit 1; }
[ "$1" = '* foo bar' ] || { printf '%s\n' "FAIL: dquoted param value wrong" >&2; exit 1; }

# Command substitution result preserved
r="$(printf '%s' '* hello')"
set -- "$r"
[ "$#" = "1" ] || { printf '%s\n' "FAIL: dquoted cmd sub should be 1 field" >&2; exit 1; }
[ "$1" = '* hello' ] || { printf '%s\n' "FAIL: dquoted cmd sub value" >&2; exit 1; }

# Arithmetic expansion result preserved (trivially)
r="$((1+2))"
[ "$r" = "3" ] || { printf '%s\n' "FAIL: arith in dquotes" >&2; exit 1; }

exit 0
