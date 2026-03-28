# Test: SHALL-19-02-04-010
# Obligation: "\r yields a <carriage-return> character."
# Verifies: \r in $'...' produces CR (0x0D).

r=$'\r'
expected=$(printf '\015')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\r should be CR (0x0D)" >&2; exit 1; }

exit 0
