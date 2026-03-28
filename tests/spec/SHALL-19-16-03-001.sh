# Test: SHALL-19-16-03-001
# Obligation: "If n is specified, the break utility shall exit from the nth
#   enclosing for, while, or until loop. If n is not specified, break shall
#   behave as if n was specified as 1. Execution shall continue with the
#   command immediately following the exited loop."

# break with no argument exits innermost loop
result=
for i in 1 2 3; do
    break
    result=bad
done
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: break without n did not exit innermost loop" >&2
    exit 1
fi

# break 1 exits innermost loop
result=
for i in 1 2 3; do
    break 1
    result=bad
done
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: break 1 did not exit innermost loop" >&2
    exit 1
fi

# break 2 exits second enclosing loop
outer_ran=no
for i in 1 2 3; do
    for j in a b c; do
        break 2
    done
    outer_ran=yes
done
if [ "$outer_ran" = "yes" ]; then
    printf '%s\n' "FAIL: break 2 did not exit outer loop" >&2
    exit 1
fi

# break n > nesting exits outermost loop
result=
for i in 1 2; do
    for j in a b; do
        break 99
    done
    result=bad
done
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: break 99 did not exit outermost loop" >&2
    exit 1
fi

# execution continues after the exited loop
after=no
for i in 1 2 3; do
    break
done
after=yes
if [ "$after" != "yes" ]; then
    printf '%s\n' "FAIL: execution did not continue after exited loop" >&2
    exit 1
fi

# break works with while loop
count=0
while true; do
    count=$((count + 1))
    if [ "$count" -ge 3 ]; then
        break
    fi
done
if [ "$count" -ne 3 ]; then
    printf '%s\n' "FAIL: break in while loop did not work" >&2
    exit 1
fi

# break works with until loop
count=0
until false; do
    count=$((count + 1))
    break
done
if [ "$count" -ne 1 ]; then
    printf '%s\n' "FAIL: break in until loop did not work" >&2
    exit 1
fi

exit 0
