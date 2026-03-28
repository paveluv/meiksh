# Test: SHALL-18-01-02-01-003
# Obligation: "The evaluation of arithmetic expressions shall be equivalent
#   to that described in Section 6.5, Expressions, of the ISO C standard."
# Verifies: Operator precedence and short-circuit evaluation.

# Precedence: * before +
r=$(( 2 + 3 * 4 ))
[ "$r" = "14" ] || { printf '%s\n' "FAIL: 2+3*4 expected 14 got $r" >&2; exit 1; }

# Parentheses override precedence
r=$(( (2 + 3) * 4 ))
[ "$r" = "20" ] || { printf '%s\n' "FAIL: (2+3)*4 expected 20 got $r" >&2; exit 1; }

# Short-circuit: && does not evaluate RHS when LHS is 0
x=1
r=$(( 0 && (x=99) ))
[ "$x" = "1" ] || { printf '%s\n' "FAIL: && short-circuit failed, x=$x" >&2; exit 1; }

# Short-circuit: || does not evaluate RHS when LHS is non-zero
x=1
r=$(( 1 || (x=99) ))
[ "$x" = "1" ] || { printf '%s\n' "FAIL: || short-circuit failed, x=$x" >&2; exit 1; }

# Ternary: only selected branch evaluated
x=1
r=$(( 1 ? (x=10) : (x=20) ))
[ "$x" = "10" ] || { printf '%s\n' "FAIL: ternary true branch, x=$x" >&2; exit 1; }

x=1
r=$(( 0 ? (x=10) : (x=20) ))
[ "$x" = "20" ] || { printf '%s\n' "FAIL: ternary false branch, x=$x" >&2; exit 1; }

# Comma operator: evaluates both, returns last
r=$(( 1+2, 3+4 ))
[ "$r" = "7" ] || { printf '%s\n' "FAIL: comma operator expected 7 got $r" >&2; exit 1; }

exit 0
