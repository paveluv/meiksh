# Test: SHALL-19-06-012
# Obligation: "If a '$' that is neither within single-quotes nor escaped by a
#   <backslash> is immediately followed by a <space>, <tab>, or a <newline>, or
#   is not followed by any character, the '$' shall be treated as a literal
#   character."
# Verifies: bare $ followed by space/tab/newline/nothing is literal.

# $ followed by space
result=$(printf '%s\n' $ foo)
if [ "$result" != '$ foo' ]; then
    printf '%s\n' "FAIL: '\$ foo' gave '$result', expected '\$ foo'" >&2
    exit 1
fi

# $ at end of word (no following character)
result2=$(printf '%s\n' "test$")
if [ "$result2" != 'test$' ]; then
    printf '%s\n' "FAIL: 'test\$' gave '$result2', expected 'test\$'" >&2
    exit 1
fi

# $ followed by tab (use literal tab)
result3=$(eval "printf '%s\n' \"\$	after\"")
case "$result3" in
    '$	after') ;;
    *)
        printf '%s\n' "FAIL: dollar-tab not treated as literal" >&2
        exit 1
        ;;
esac

exit 0
