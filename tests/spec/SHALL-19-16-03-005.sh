# Test: SHALL-19-16-03-005
# Obligation: "A loop lexically encloses a break or continue command if the
#   command is: Not in the body of a function whose function definition command
#   is contained in a compound-list associated with the loop."

# break inside a function defined in a loop does NOT break the loop
result=
for i in 1 2 3; do
    f() { break 2>/dev/null; }
    f
    result="${result}${i}"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: break in function body should not break enclosing loop, got '$result'" >&2
    exit 1
fi

exit 0
