# Test: SHALL-19-09-01-01-005
# Obligation: "Each variable assignment shall be expanded for tilde expansion,
#   parameter expansion, command substitution, arithmetic expansion, and
#   quote removal prior to assigning the value."
# Verifies: assignment values undergo proper expansion (no field splitting
#   or pathname expansion).

HOME_SAVE="$HOME"
result=$("$SHELL" -c '
X=$(printf "%s" expanded)
printf "%s\n" "$X"
')
if [ "$result" != "expanded" ]; then
    printf '%s\n' "FAIL: command substitution in assignment not expanded" >&2
    exit 1
fi

# No field splitting in assignments
result2=$("$SHELL" -c '
X="a   b   c"
Y=$X
printf "%s\n" "$Y"
')
if [ "$result2" != "a   b   c" ]; then
    printf '%s\n' "FAIL: field splitting occurred in assignment value" >&2
    exit 1
fi

# Arithmetic expansion in assignment
result3=$("$SHELL" -c '
X=$((2+3))
printf "%s\n" "$X"
')
if [ "$result3" != "5" ]; then
    printf '%s\n' "FAIL: arithmetic expansion in assignment failed" >&2
    exit 1
fi

exit 0
