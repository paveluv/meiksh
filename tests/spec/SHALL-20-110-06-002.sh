# Test: SHALL-20-110-06-002
# Obligation: "The standard input shall be used only if one of the following
#   is true: The -s option is specified."
# Verifies: sh -s reads commands from stdin, with operands becoming
#   positional parameters.

cmd='printf "%s\n" "$1"'
result=$(printf '%s\n' "$cmd" | "$MEIKSH" -s hello)
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: sh -s did not read from stdin or set \$1, got '$result'" >&2
    exit 1
fi

result2=$(printf '%s\n' 'printf "%s %s\n" "$1" "$2"' | "$MEIKSH" -s aaa bbb)
if [ "$result2" != "aaa bbb" ]; then
    printf '%s\n' "FAIL: sh -s positional params wrong, got '$result2'" >&2
    exit 1
fi

exit 0
