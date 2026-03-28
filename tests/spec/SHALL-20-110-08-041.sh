# Test: SHALL-20-110-08-041
# Obligation: "This variable shall represent an absolute pathname of the
#   current working directory. Assignments to this variable may be ignored."
# Verifies: PWD is absolute after cd (duplicate of 08-033).

result=$("$MEIKSH" -c 'cd / && printf "%s\n" "$PWD"')
if [ "$result" != "/" ]; then
    printf '%s\n' "FAIL: PWD not '/' after cd /, got '$result'" >&2
    exit 1
fi

exit 0
