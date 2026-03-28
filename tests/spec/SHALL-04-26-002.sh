# Test: SHALL-04-26-002
# Obligation: "The varname and value parts shall meet the requirements for a
#   name and a word, respectively, except that they are delimited by the
#   embedded unquoted <equals-sign>."
# Verifies: varname must be a valid Name, value is a word, unquoted = is
#   the delimiter.

# Valid name: starts with letter, contains letters/digits/underscores
_var123=ok
if [ "$_var123" != "ok" ]; then
    echo "FAIL: _var123=ok did not assign" >&2
    exit 1
fi

# Value can contain equals signs (first unquoted = splits name from value)
A=x=y=z
if [ "$A" != "x=y=z" ]; then
    echo "FAIL: A=x=y=z should set A to 'x=y=z', got '$A'" >&2
    exit 1
fi

# Value can be a complex word with expansions
B=hello
C=${B}_world
if [ "$C" != "hello_world" ]; then
    echo "FAIL: C=\${B}_world should be 'hello_world', got '$C'" >&2
    exit 1
fi

# Quoted = is not an assignment delimiter (the whole thing is a word)
result=$(printf '%s\n' 'FOO=bar')
if [ "$result" != "FOO=bar" ]; then
    echo "FAIL: quoted = should not create assignment" >&2
    exit 1
fi

exit 0
