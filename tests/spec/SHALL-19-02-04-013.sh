# Test: SHALL-19-02-04-013
# Obligation: "\cX yields the control character listed in the Value column
#   of Values for cpio c_mode Field in the OPERANDS section of the stty
#   utility when X is one of the characters listed in the ^c column."
# Verifies: \cA produces SOH (0x01), \cZ produces SUB (0x1A).

r=$'\cA'
expected=$(printf '\001')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\cA should be SOH (0x01)" >&2; exit 1; }

r=$'\cZ'
expected=$(printf '\032')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\cZ should be SUB (0x1A)" >&2; exit 1; }

r=$'\c?'
expected=$(printf '\177')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\c? should be DEL (0x7F)" >&2; exit 1; }

exit 0
