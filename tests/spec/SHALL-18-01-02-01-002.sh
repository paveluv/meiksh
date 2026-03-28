# Test: SHALL-18-01-02-01-002
# Obligation: "Arithmetic operators and control flow keywords shall be
#   implemented as equivalent to those in the cited ISO C standard section."
# Verifies: C-equivalent arithmetic operators in shell $(( )).

# Unary negation
r=$(( -5 ))
[ "$r" = "-5" ] || { printf '%s\n' "FAIL: unary - got $r" >&2; exit 1; }

# Bitwise NOT
r=$(( ~0 ))
[ "$r" = "-1" ] || { printf '%s\n' "FAIL: ~0 expected -1 got $r" >&2; exit 1; }

# Logical NOT
r=$(( !0 ))
[ "$r" = "1" ] || { printf '%s\n' "FAIL: !0 expected 1 got $r" >&2; exit 1; }
r=$(( !5 ))
[ "$r" = "0" ] || { printf '%s\n' "FAIL: !5 expected 0 got $r" >&2; exit 1; }

# Left shift
r=$(( 1 << 4 ))
[ "$r" = "16" ] || { printf '%s\n' "FAIL: 1<<4 expected 16 got $r" >&2; exit 1; }

# Right shift
r=$(( 16 >> 2 ))
[ "$r" = "4" ] || { printf '%s\n' "FAIL: 16>>2 expected 4 got $r" >&2; exit 1; }

# Bitwise AND
r=$(( 12 & 10 ))
[ "$r" = "8" ] || { printf '%s\n' "FAIL: 12&10 expected 8 got $r" >&2; exit 1; }

# Bitwise OR
r=$(( 12 | 3 ))
[ "$r" = "15" ] || { printf '%s\n' "FAIL: 12|3 expected 15 got $r" >&2; exit 1; }

# Bitwise XOR
r=$(( 12 ^ 10 ))
[ "$r" = "6" ] || { printf '%s\n' "FAIL: 12^10 expected 6 got $r" >&2; exit 1; }

# Comparison operators
[ $(( 3 < 5 )) = "1" ] || { printf '%s\n' "FAIL: 3<5" >&2; exit 1; }
[ $(( 5 < 3 )) = "0" ] || { printf '%s\n' "FAIL: 5<3" >&2; exit 1; }
[ $(( 3 == 3 )) = "1" ] || { printf '%s\n' "FAIL: 3==3" >&2; exit 1; }
[ $(( 3 != 4 )) = "1" ] || { printf '%s\n' "FAIL: 3!=4" >&2; exit 1; }

# Ternary
r=$(( 1 ? 10 : 20 ))
[ "$r" = "10" ] || { printf '%s\n' "FAIL: 1?10:20 expected 10 got $r" >&2; exit 1; }
r=$(( 0 ? 10 : 20 ))
[ "$r" = "20" ] || { printf '%s\n' "FAIL: 0?10:20 expected 20 got $r" >&2; exit 1; }

# Assignment operators
x=5; r=$(( x += 3 ))
[ "$r" = "8" ] || { printf '%s\n' "FAIL: x+=3 expected 8 got $r" >&2; exit 1; }

# Logical AND / OR (short-circuit)
[ $(( 1 && 1 )) = "1" ] || { printf '%s\n' "FAIL: 1&&1" >&2; exit 1; }
[ $(( 1 && 0 )) = "0" ] || { printf '%s\n' "FAIL: 1&&0" >&2; exit 1; }
[ $(( 0 || 1 )) = "1" ] || { printf '%s\n' "FAIL: 0||1" >&2; exit 1; }
[ $(( 0 || 0 )) = "0" ] || { printf '%s\n' "FAIL: 0||0" >&2; exit 1; }

exit 0
