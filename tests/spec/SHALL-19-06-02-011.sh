# Test: SHALL-19-06-02-011
# Obligation: "String Length. The shortest decimal representation of the length
#   in characters of the value of parameter shall be substituted."
# Verifies: ${#parameter} returns correct string length.

x="hello"
result="${#x}"
if [ "$result" != "5" ]; then
    printf '%s\n' "FAIL: \${#x} for 'hello' gave '$result', expected '5'" >&2
    exit 1
fi

# Empty string
y=""
result2="${#y}"
if [ "$result2" != "0" ]; then
    printf '%s\n' "FAIL: \${#y} for '' gave '$result2', expected '0'" >&2
    exit 1
fi

# Positional parameter count
set -- a b c d
result3="${#}"
if [ "$result3" != "4" ]; then
    printf '%s\n' "FAIL: \${#} gave '$result3', expected '4'" >&2
    exit 1
fi

exit 0
