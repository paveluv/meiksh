# Test: SHALL-19-18-03-001
# Obligation: "If n is specified, the continue utility shall return to the top
#   of the nth enclosing for, while, or until loop. If n is not specified,
#   continue shall behave as if n was specified as 1."

# continue without argument skips rest of innermost loop body
result=
for i in 1 2 3; do
    result="${result}${i}"
    continue
    result="${result}x"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: continue without n, expected '123' got '$result'" >&2
    exit 1
fi

# continue 1 same as continue
result=
for i in 1 2 3; do
    result="${result}${i}"
    continue 1
    result="${result}x"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: continue 1, expected '123' got '$result'" >&2
    exit 1
fi

# continue 2 returns to top of second enclosing loop
result=
for i in a b; do
    for j in 1 2 3; do
        result="${result}${i}${j}"
        continue 2
    done
    result="${result}X"
done
if [ "$result" != "a1b1" ]; then
    printf '%s\n' "FAIL: continue 2, expected 'a1b1' got '$result'" >&2
    exit 1
fi

# continue in while loop re-evaluates condition
count=0
total=0
while [ "$count" -lt 5 ]; do
    count=$((count + 1))
    if [ "$count" -eq 3 ]; then
        continue
    fi
    total=$((total + count))
done
# total = 1+2+4+5 = 12
if [ "$total" -ne 12 ]; then
    printf '%s\n' "FAIL: continue in while, expected total=12 got $total" >&2
    exit 1
fi

exit 0
