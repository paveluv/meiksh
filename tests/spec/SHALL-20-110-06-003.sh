# Test: SHALL-20-110-06-003
# Obligation: "The standard input shall be used only if one of the following
#   is true: The -c option is not specified and no operands are specified."
# Verifies: sh with no -c and no operands reads commands from stdin.

result=$(printf '%s\n' 'printf "%s\n" "from-stdin"' | "$MEIKSH")
if [ "$result" != "from-stdin" ]; then
    printf '%s\n' "FAIL: sh without -c/operands did not read stdin, got '$result'" >&2
    exit 1
fi

exit 0
