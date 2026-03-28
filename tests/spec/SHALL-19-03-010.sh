# SHALL-19-03-010
# "If the current character is an unquoted <blank>, any token containing the
#  previous character is delimited and the current character shall be discarded."
# Verify blanks delimit tokens and are discarded.

fail=0

# Multiple spaces between args — should still produce exactly 2 args
set -- $(printf 'a   b')
[ $# -eq 2 ] || { printf '%s\n' "FAIL: expected 2 args, got $#" >&2; fail=1; }
[ "$1" = "a" ] || { printf '%s\n' "FAIL: arg1 = '$1', expected 'a'" >&2; fail=1; }
[ "$2" = "b" ] || { printf '%s\n' "FAIL: arg2 = '$2', expected 'b'" >&2; fail=1; }

# Tab also delimits
set -- $(printf 'x\ty')
[ $# -eq 2 ] || { printf '%s\n' "FAIL: tab delimit: expected 2 args, got $#" >&2; fail=1; }

exit "$fail"
