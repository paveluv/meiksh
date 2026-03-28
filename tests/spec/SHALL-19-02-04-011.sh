# Test: SHALL-19-02-04-011
# Obligation: "\t yields a <tab> character."
# Verifies: \t in $'...' produces HT (0x09).

r=$'\t'
expected=$(printf '\011')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\t should be HT (0x09)" >&2; exit 1; }

exit 0
