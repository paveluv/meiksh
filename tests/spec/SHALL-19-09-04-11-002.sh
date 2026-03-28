# Test: SHALL-19-09-04-11-002
# Obligation: "The compound-list-1 shall be executed, and if it has a zero exit
#   status, the until command completes. Otherwise, the compound-list-2 shall
#   be executed, and the process repeats."
# Verifies: until stops when condition succeeds.

# Condition true initially: body never runs
ran=no
until true; do
    ran=yes
done
if [ "$ran" != "no" ]; then
    printf '%s\n' "FAIL: until true should not run body" >&2
    exit 1
fi

# Condition becomes true: loop terminates
n=0
until [ "$n" -ge 2 ]; do
    n=$((n + 1))
done
if [ "$n" -ne 2 ]; then
    printf '%s\n' "FAIL: until did not stop when condition succeeded" >&2
    exit 1
fi

exit 0
