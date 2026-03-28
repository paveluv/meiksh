# Test: SHALL-19-02-04-009
# Obligation: "\n yields a <newline> character."
# Verifies: \n in $'...' produces LF (0x0A).

r=$'\n'
expected=$(printf '\012.')
expected="${expected%.}"
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\n should be LF (0x0A)" >&2; exit 1; }

exit 0
