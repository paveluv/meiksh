# Test: SHALL-19-09-04-09-002
# Obligation: "The compound-list-1 shall be executed, and if it has a non-zero
#   exit status, the while command shall complete. Otherwise, the compound-list-2
#   shall be executed, and the process shall repeat."
# Verifies: while condition evaluated before each iteration.

# Condition false initially: body never runs
ran=no
while false; do
    ran=yes
done
if [ "$ran" != "no" ]; then
    printf '%s\n' "FAIL: while false should not run body" >&2
    exit 1
fi

# Condition becomes false: loop terminates
n=0
while [ "$n" -lt 2 ]; do
    n=$((n + 1))
done
if [ "$n" -ne 2 ]; then
    printf '%s\n' "FAIL: while did not stop when condition became false" >&2
    exit 1
fi

exit 0
