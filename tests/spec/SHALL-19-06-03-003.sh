# Test: SHALL-19-06-03-003
# Obligation: "Within the backquoted style of command substitution, if the
#   command substitution is not within double-quotes, <backslash> shall retain
#   its literal meaning, except when followed by: '$', '`', or <backslash>."
# Verifies: backslash handling inside backquoted command substitution.

# \$ inside backticks should produce literal $
result=`printf '%s\n' \$HOME`
if [ "$result" != '$HOME' ]; then
    printf '%s\n' "FAIL: backslash-dollar in backticks: got '$result'" >&2
    exit 1
fi

# \\ inside backticks should produce single backslash
result2=`printf '%s\n' \\\\`
if [ "$result2" != '\' ]; then
    printf '%s\n' "FAIL: backslash-backslash in backticks: got '$result2'" >&2
    exit 1
fi

exit 0
