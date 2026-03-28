# Test: SHALL-19-02-04-015
# Obligation: "\ddd yields the byte whose value is the octal value ddd
#   (one to three octal digits)."
# Verifies: \101 produces 'A', \012 produces newline.

r=$'\101'
[ "$r" = "A" ] || { printf '%s\n' "FAIL: \\101 should be A" >&2; exit 1; }

r=$'\012'
expected=$(printf '\012.')
expected="${expected%.}"
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\012 should be newline" >&2; exit 1; }

# Single octal digit
r=$'\7'
expected=$(printf '\007')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\7 should be BEL" >&2; exit 1; }

exit 0
