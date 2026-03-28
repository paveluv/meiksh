# Test: SHALL-19-16-03-003
# Obligation: "A loop lexically encloses a break or continue command if the
#   command is: Executing in the same execution environment as the
#   compound-list of the loop's do-group"

# break in a subshell does NOT break the enclosing loop
result=
for i in 1 2 3; do
    (break) 2>/dev/null
    result="${result}${i}"
done
if [ "$result" != "123" ]; then
    printf '%s\n' "FAIL: break in subshell should not break parent loop, got '$result'" >&2
    exit 1
fi

exit 0
