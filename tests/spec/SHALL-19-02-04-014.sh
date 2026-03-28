# Test: SHALL-19-02-04-014
# Obligation: "\xXX yields the byte whose value is the hexadecimal value XX
#   (one or more hexadecimal digits)."
# Verifies: \x41 produces 'A' (0x41), \x61 produces 'a' (0x61).

r=$'\x41'
[ "$r" = "A" ] || { printf '%s\n' "FAIL: \\x41 should be A" >&2; exit 1; }

r=$'\x61'
[ "$r" = "a" ] || { printf '%s\n' "FAIL: \\x61 should be a" >&2; exit 1; }

# Single hex digit
r=$'\x9'
expected=$(printf '\011')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\x9 should be HT (0x09)" >&2; exit 1; }

exit 0
