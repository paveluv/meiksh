# Test: SHALL-19-06-03-001
# Obligation: "Command substitution allows the output of one or more commands
#   to be substituted in place of the commands themselves. Command substitution
#   shall occur when command(s) are enclosed as follows:"
# Verifies: both $(commands) and `commands` forms are recognized.

# $() form
result1=$(printf '%s\n' hello)
if [ "$result1" != "hello" ]; then
    printf '%s\n' "FAIL: \$(printf hello) gave '$result1'" >&2
    exit 1
fi

# backtick form
result2=`printf '%s\n' world`
if [ "$result2" != "world" ]; then
    printf '%s\n' "FAIL: backtick printf gave '$result2'" >&2
    exit 1
fi

exit 0
