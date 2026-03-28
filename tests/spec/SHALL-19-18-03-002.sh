# Test: SHALL-19-18-03-002
# Obligation: "The application shall ensure that the value of n is a positive
#   decimal integer. If n is greater than the number of enclosing loops, the
#   outermost enclosing loop shall be used."

# continue with n > nesting level continues outermost loop
result=
for i in a b; do
    for j in 1 2 3; do
        result="${result}${i}${j}"
        continue 99
    done
    result="${result}X"
done
if [ "$result" != "a1b1" ]; then
    printf '%s\n' "FAIL: continue 99, expected 'a1b1' got '$result'" >&2
    exit 1
fi

exit 0
