# Test: SHALL-19-06-04-005
# Obligation: "Only the decimal-constant, octal-constant, and
#   hexadecimal-constant constants specified in the ISO C standard, Section
#   6.4.4.1 are required to be recognized as constants."
# Verifies: decimal, octal, and hexadecimal constants in arithmetic.

# Decimal
result=$((42))
if [ "$result" != "42" ]; then
    printf '%s\n' "FAIL: decimal 42: got '$result'" >&2
    exit 1
fi

# Octal (077 = 63 decimal)
result2=$((077))
if [ "$result2" != "63" ]; then
    printf '%s\n' "FAIL: octal 077: got '$result2', expected '63'" >&2
    exit 1
fi

# Hexadecimal (0xFF = 255 decimal)
result3=$((0xFF))
if [ "$result3" != "255" ]; then
    printf '%s\n' "FAIL: hex 0xFF: got '$result3', expected '255'" >&2
    exit 1
fi

# Hex with uppercase X
result4=$((0XA))
if [ "$result4" != "10" ]; then
    printf '%s\n' "FAIL: hex 0XA: got '$result4', expected '10'" >&2
    exit 1
fi

exit 0
