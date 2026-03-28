# Test: SHALL-18-01-02-001
# Obligation: "Unless otherwise noted, the arithmetic and semantic concepts
#   (precision, type conversion, control flow, and so on) shall be equivalent
#   to those defined in the ISO C standard."
# Verifies: Shell arithmetic follows C semantics for basic operations.

# Addition
r=$(( 3 + 4 ))
if [ "$r" != "7" ]; then
    printf '%s\n' "FAIL: 3+4 expected 7 got $r" >&2; exit 1
fi

# Multiplication
r=$(( 6 * 7 ))
if [ "$r" != "42" ]; then
    printf '%s\n' "FAIL: 6*7 expected 42 got $r" >&2; exit 1
fi

# Integer division (truncation toward zero, C semantics)
r=$(( 7 / 2 ))
if [ "$r" != "3" ]; then
    printf '%s\n' "FAIL: 7/2 expected 3 got $r" >&2; exit 1
fi

# Modulo
r=$(( 7 % 3 ))
if [ "$r" != "1" ]; then
    printf '%s\n' "FAIL: 7%%3 expected 1 got $r" >&2; exit 1
fi

# Negative division (C99: truncation toward zero)
r=$(( -7 / 2 ))
if [ "$r" != "-3" ]; then
    printf '%s\n' "FAIL: -7/2 expected -3 got $r" >&2; exit 1
fi

exit 0
