# Test: SHALL-19-16-03-004
# Obligation: "A loop lexically encloses a break or continue command if the
#   command is: Contained in a compound-list associated with the loop (either
#   in the compound-list of the loop's do-group or, if the loop is a while or
#   until loop, in the compound-list following the while or until reserved word)"

# break in the condition of a while loop
count=0
while
    count=$((count + 1))
    if [ "$count" -ge 3 ]; then break; fi
    true
do
    :
done
if [ "$count" -ne 3 ]; then
    printf '%s\n' "FAIL: break in while condition did not work, count=$count" >&2
    exit 1
fi

# break in the body of a while loop
count=0
while true; do
    count=$((count + 1))
    break
done
if [ "$count" -ne 1 ]; then
    printf '%s\n' "FAIL: break in while body did not work" >&2
    exit 1
fi

exit 0
