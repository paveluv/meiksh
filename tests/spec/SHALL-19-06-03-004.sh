# Test: SHALL-19-06-03-004
# Obligation: "the commands string shall be tokenized (see 2.3 Token
#   Recognition) and parsed (see 2.10 Shell Grammar)."
# Verifies: command substitution body is properly tokenized and parsed.

# Multi-command pipeline inside command substitution
result=$(printf '%s\n' 'hello world' | tr ' ' '_')
if [ "$result" != "hello_world" ]; then
    printf '%s\n' "FAIL: pipeline in cmd sub: got '$result'" >&2
    exit 1
fi

# Compound commands inside command substitution
result2=$(if true; then printf '%s\n' yes; else printf '%s\n' no; fi)
if [ "$result2" != "yes" ]; then
    printf '%s\n' "FAIL: if/then in cmd sub: got '$result2'" >&2
    exit 1
fi

# Semicolons separating commands
result3=$(printf '%s' a; printf '%s\n' b)
if [ "$result3" != "ab" ]; then
    printf '%s\n' "FAIL: semicolons in cmd sub: got '$result3'" >&2
    exit 1
fi

exit 0
