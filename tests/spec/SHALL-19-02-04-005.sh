# Test: SHALL-19-02-04-005
# Obligation: "\a yields an <alert> character."
# Verifies: \a in $'...' produces BEL (0x07).

r=$'\a'
expected=$(printf '\007')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\a should be BEL (0x07)" >&2; exit 1; }

exit 0
