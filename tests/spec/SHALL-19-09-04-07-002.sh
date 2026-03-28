# Test: SHALL-19-09-04-07-002
# Obligation: "The if compound-list shall be executed; if its exit status is
#   zero, the then compound-list shall be executed and the command shall
#   complete. Otherwise, each elif compound-list shall be executed, in turn ...
#   Otherwise, the else compound-list shall be executed."
# Verifies: if/elif/else execution order.

# if true path
result=""
if true; then
    result="if"
elif true; then
    result="elif"
else
    result="else"
fi
if [ "$result" != "if" ]; then
    printf '%s\n' "FAIL: should take if branch, got '$result'" >&2
    exit 1
fi

# elif path
result=""
if false; then
    result="if"
elif true; then
    result="elif"
else
    result="else"
fi
if [ "$result" != "elif" ]; then
    printf '%s\n' "FAIL: should take elif branch, got '$result'" >&2
    exit 1
fi

# else path
result=""
if false; then
    result="if"
elif false; then
    result="elif"
else
    result="else"
fi
if [ "$result" != "else" ]; then
    printf '%s\n' "FAIL: should take else branch, got '$result'" >&2
    exit 1
fi

exit 0
