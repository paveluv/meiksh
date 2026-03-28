# Test: SHALL-19-16-03-002
# Obligation: "A loop shall enclose a break or continue command if the loop
#   lexically encloses the command."

# Verify that a loop lexically enclosing break is recognized
result=ok
for i in 1 2 3; do
    break
    result=bad
done
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: break not recognized as enclosed by lexical loop" >&2
    exit 1
fi

# Verify same for continue
result=
for i in 1 2 3; do
    continue
    result="${result}bad"
done
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: continue not recognized as enclosed by lexical loop" >&2
    exit 1
fi

exit 0
