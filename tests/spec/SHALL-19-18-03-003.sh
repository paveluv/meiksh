# Test: SHALL-19-18-03-003
# Obligation: "The meaning of \"enclosing\" shall be as specified in the
#   description of the break utility."

# continue in subshell does NOT affect enclosing loop (same exec env rule)
result=
for i in 1 2 3; do
    (continue) 2>/dev/null
    result="${result}${i}"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: continue in subshell should not affect parent loop, got '$result'" >&2
    exit 1
fi

# continue in function defined in loop does NOT affect enclosing loop
result=
for i in 1 2 3; do
    g() { continue 2>/dev/null; }
    g
    result="${result}${i}"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: continue in function should not affect enclosing loop, got '$result'" >&2
    exit 1
fi

exit 0
