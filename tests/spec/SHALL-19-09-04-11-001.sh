# Test: SHALL-19-09-04-11-001
# Obligation: "The until loop shall continuously execute one compound-list as
#   long as another compound-list has a non-zero exit status."
# Verifies: until loop basic behavior.

count=0
until [ "$count" -ge 3 ]; do
    count=$((count + 1))
done
if [ "$count" -ne 3 ]; then
    printf '%s\n' "FAIL: until loop did not iterate correctly: count=$count" >&2
    exit 1
fi

exit 0
