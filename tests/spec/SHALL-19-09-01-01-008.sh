# Test: SHALL-19-09-01-01-008
# Obligation: "Each variable assignment shall be expanded for tilde expansion,
#   parameter expansion, command substitution, arithmetic expansion, and
#   quote removal prior to assigning the value."
# Duplicate of SHALL-19-09-01-01-005 — same requirement.
# Verifies: assignment values are expanded.

result=$("$SHELL" -c 'X=$((3*4)); printf "%s\n" "$X"')
if [ "$result" != "12" ]; then
    printf '%s\n' "FAIL: arithmetic in assignment not expanded" >&2
    exit 1
fi

exit 0
