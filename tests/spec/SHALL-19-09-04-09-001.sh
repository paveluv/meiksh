# Test: SHALL-19-09-04-09-001
# Obligation: "The while loop shall continuously execute one compound-list as
#   long as another compound-list has a zero exit status."
# Verifies: while loop basic behavior.

count=0
while [ "$count" -lt 3 ]; do
    count=$((count + 1))
done
if [ "$count" -ne 3 ]; then
    printf '%s\n' "FAIL: while loop did not iterate correctly: count=$count" >&2
    exit 1
fi

exit 0
