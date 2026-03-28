# Test: SHALL-19-09-01-01-003
# Obligation: "The first word (if any) that is not a variable assignment or
#   redirection shall be expanded. If any fields remain following its
#   expansion, the first field shall be considered the command name."
# Verifies: first non-assignment/non-redirection word becomes command name
#   after expansion.

# Variable expansion produces command name
result=$("$SHELL" -c 'CMD=printf; $CMD "%s\n" "expanded"')
if [ "$result" != "expanded" ]; then
    printf '%s\n' "FAIL: expanded word not used as command name" >&2
    exit 1
fi

# Empty expansion skips to next word
result2=$("$SHELL" -c 'EMPTY=; $EMPTY printf "%s\n" "found"')
if [ "$result2" != "found" ]; then
    printf '%s\n' "FAIL: empty expansion not skipped for command name" >&2
    exit 1
fi

exit 0
