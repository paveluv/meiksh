# Test: SHALL-19-02-04-007
# Obligation: "\e yields an <ESC> character."
# Verifies: \e in $'...' produces ESC (0x1B).

r=$'\e'
expected=$(printf '\033')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\e should be ESC (0x1B)" >&2; exit 1; }

exit 0
