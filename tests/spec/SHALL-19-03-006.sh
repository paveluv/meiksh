# Test: SHALL-19-03-006
# Obligation: "If the previous character was used as part of an operator and
#   the current character cannot be used with the previous characters to form
#   an operator, the operator containing the previous character shall be
#   delimited."
# Verifies: Operator is delimited when next char cannot extend it.

# ">" followed by non-operator char: > is delimited, next char starts word
f="$TMPDIR/shall_19_03_006_$$"
printf '%s\n' "test" > "$f"
r=$(cat "$f")
[ "$r" = "test" ] || { printf '%s\n' "FAIL: > delimited before filename" >&2; rm -f "$f"; exit 1; }
rm -f "$f"

# "|" followed by non-| char: | is delimited as pipe operator
r=$(printf '%s\n' "hello" | cat)
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: | delimited" >&2; exit 1; }

exit 0
