# Test: SHALL-19-09-04-03-002
# Obligation: "First, the list of words following in shall be expanded to
#   generate a list of items. ... If no items result from the expansion, the
#   compound-list shall not be executed."
# Verifies: for loop expands word list; empty list skips body.

# Expansion of word list
V="x y z"
result=""
for i in $V; do
    result="${result}${i}"
done
if [ "$result" != "xyz" ]; then
    printf '%s\n' "FAIL: word expansion in for not working: got '$result'" >&2
    exit 1
fi

# Empty expansion: body should not run
ran=no
for i in $(true); do
    ran=yes
done
if [ "$ran" != "no" ]; then
    printf '%s\n' "FAIL: for body ran on empty expansion" >&2
    exit 1
fi

exit 0
