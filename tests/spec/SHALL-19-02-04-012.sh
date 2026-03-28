# Test: SHALL-19-02-04-012
# Obligation: "\v yields a <vertical-tab> character."
# Verifies: \v in $'...' produces VT (0x0B).

r=$'\v'
expected=$(printf '\013')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\v should be VT (0x0B)" >&2; exit 1; }

exit 0
