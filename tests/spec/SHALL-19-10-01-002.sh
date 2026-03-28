# Test: SHALL-19-10-01-002
# Obligation: "If the token is an operator, the token identifier for that
#   operator shall result."
# Verifies: Shell recognizes multi-character operators as single tokens.

# && recognized as AND operator (not two &)
true && true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: && not recognized as operator" >&2
    exit 1
fi

# || recognized as OR operator (not two |)
false || true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: || not recognized as operator" >&2
    exit 1
fi

# ;; recognized in case
result=""
case x in x) result="ok" ;; esac
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: ;; not recognized in case" >&2
    exit 1
fi

exit 0
