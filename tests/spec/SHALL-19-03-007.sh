# SHALL-19-03-007
# "If the current character is an unquoted <backslash>, single-quote, or
#  double-quote or is the first character of an unquoted <dollar-sign>
#  single-quote sequence, it shall affect quoting for subsequent characters up
#  to the end of the quoted text. ... During token recognition no substitutions
#  shall be actually performed, and the result token shall contain exactly the
#  characters that appear in the input unmodified ... The token shall not be
#  delimited by the end of the quoted field."

# Verify quoting characters do not delimit the token — text before, inside,
# and after quotes forms a single word.

fail=0

# Double-quote in middle of word: he"ll"o → hello (single arg)
result=$(eval 'printf "%s\n" he"ll"o')
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: he\"ll\"o produced '$result'" >&2; fail=1; }

# Single-quote in middle of word: he'll'o → hello
result=$(eval "printf '%s\n' he'll'o")
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: he'll'o produced '$result'" >&2; fail=1; }

# Backslash in middle of word: hel\lo → hello (\ quotes l)
result=$(printf '%s\n' hel\lo)
[ "$result" = "hello" ] || { printf '%s\n' "FAIL: hel\\lo produced '$result'" >&2; fail=1; }

# Mixed quoting forms single token: a"b"c'd'e → abcde
result=$(eval 'printf "%s\n" a"b"c'"'"'d'"'"'e')
[ "$result" = "abcde" ] || { printf '%s\n' "FAIL: mixed quoting produced '$result'" >&2; fail=1; }

exit "$fail"
