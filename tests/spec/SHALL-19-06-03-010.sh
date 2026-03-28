# Test: SHALL-19-06-03-010
# Obligation: "Command substitution can be nested. To specify nesting within
#   the backquoted version, the application shall precede the inner backquotes
#   with <backslash> characters."
# Verifies: nested command substitution works (both forms).

# Nested $() form
result=$(printf '%s\n' "$(printf '%s' inner)")
if [ "$result" != "inner" ]; then
    printf '%s\n' "FAIL: nested \$() gave '$result'" >&2
    exit 1
fi

# Nested backtick form with escaped inner backquotes
result2=`printf '%s\n' \`printf '%s' nested\``
if [ "$result2" != "nested" ]; then
    printf '%s\n' "FAIL: nested backticks gave '$result2'" >&2
    exit 1
fi

exit 0
