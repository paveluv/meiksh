# Test: SHALL-19-02-04-006
# Obligation: "\b yields a <backspace> character."
# Verifies: \b in $'...' produces BS (0x08).

r=$'\b'
expected=$(printf '\010')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\b should be BS (0x08)" >&2; exit 1; }

exit 0
