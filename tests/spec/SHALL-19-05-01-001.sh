# SHALL-19-05-01-001
# "A positional parameter is a parameter denoted by a decimal representation of
#  a positive integer. The digits denoting the positional parameters shall always
#  be interpreted as a decimal value, even if there is a leading zero. When a
#  positional parameter with more than one digit is specified, the application
#  shall enclose the digits in braces."
# Verify positional parameter access including multi-digit with braces.

fail=0

set -- a b c d e f g h i j k

# Single digit access
[ "$1" = "a" ] || { printf '%s\n' "FAIL: \$1 = '$1'" >&2; fail=1; }
[ "$9" = "i" ] || { printf '%s\n' "FAIL: \$9 = '$9'" >&2; fail=1; }

# Multi-digit requires braces
[ "${10}" = "j" ] || { printf '%s\n' "FAIL: \${10} = '${10}'" >&2; fail=1; }
[ "${11}" = "k" ] || { printf '%s\n' "FAIL: \${11} = '${11}'" >&2; fail=1; }

# $10 without braces is $1 followed by '0'
result=$(eval 'printf "%s" "$1"0')
[ "$result" = "a0" ] || { printf '%s\n' "FAIL: \$10 without braces = '$result'" >&2; fail=1; }

exit "$fail"
