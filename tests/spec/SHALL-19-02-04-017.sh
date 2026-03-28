# Test: SHALL-19-02-04-017
# Obligation: "the escape sequence shall be terminated by the first character
#   that is not of the expected type or, for \ddd sequences, when the maximum
#   number of characters specified has been found, whichever occurs first."
# Verifies: \x and \ddd termination rules.

# \x41B: consumes \x41 (='A'), then 'B' is literal -> "AB"
r=$'\x41B'
[ "$r" = "AB" ] || { printf '%s\n' "FAIL: \\x41B should be AB, got '$r'" >&2; exit 1; }

# \101C: consumes \101 (='A'), then 'C' is literal -> "AC"
r=$'\101C'
[ "$r" = "AC" ] || { printf '%s\n' "FAIL: \\101C should be AC, got '$r'" >&2; exit 1; }

# \1z: consumes \1 (0x01), then 'z' is literal
r=$'\1z'
expected=$(printf '\001z')
[ "$r" = "$expected" ] || { printf '%s\n' "FAIL: \\1z termination" >&2; exit 1; }

exit 0
