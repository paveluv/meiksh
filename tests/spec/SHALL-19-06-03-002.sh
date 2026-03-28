# Test: SHALL-19-06-03-002
# Obligation: "The shell shall expand the command substitution by executing
#   commands in a subshell environment [...] replacing the command substitution
#   [...] with the standard output of the command(s); if the output ends with
#   one or more bytes that have the encoded value of a <newline> character,
#   they shall not be included in the replacement."
# Verifies: runs in subshell; trailing newlines stripped; interior preserved.

# Trailing newlines must be stripped
result=$(printf 'hello\n\n\n')
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: trailing newlines not stripped: got '$result'" >&2
    exit 1
fi

# Interior newlines must be preserved
result2=$(printf 'a\nb')
expected=$(printf 'a\nb')
if [ "$result2" != "$expected" ]; then
    printf '%s\n' "FAIL: interior newline not preserved" >&2
    exit 1
fi

# Subshell: variable changes don't affect parent
myvar=before
dummy=$(myvar=after)
if [ "$myvar" != "before" ]; then
    printf '%s\n' "FAIL: subshell variable change leaked to parent" >&2
    exit 1
fi

exit 0
