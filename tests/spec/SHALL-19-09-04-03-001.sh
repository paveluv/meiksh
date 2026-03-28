# Test: SHALL-19-09-04-03-001
# Obligation: "The for loop shall execute a sequence of commands for each member
#   in a list of items. The for loop requires that the reserved words do and
#   done be used to delimit the sequence of commands."
# Verifies: Basic for loop with do/done.

result=""
for i in a b c; do
    result="${result}${i}"
done
if [ "$result" != "abc" ]; then
    printf '%s\n' "FAIL: for loop did not iterate correctly: got '$result'" >&2
    exit 1
fi

exit 0
