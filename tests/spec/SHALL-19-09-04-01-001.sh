# Test: SHALL-19-09-04-01-001
# Obligation: "Execute compound-list in a subshell environment ... Variable
#   assignments and built-in commands that affect the environment shall not
#   remain in effect after the list finishes."
# Verifies: Subshell ( ) isolates variable assignments.

X=before
(X=after)
if [ "$X" != "before" ]; then
    printf '%s\n' "FAIL: variable set in subshell leaked to parent" >&2
    exit 1
fi

# cd in subshell should not affect parent
origdir=$(pwd)
(cd /tmp)
if [ "$(pwd)" != "$origdir" ]; then
    printf '%s\n' "FAIL: cd in subshell affected parent working directory" >&2
    exit 1
fi

exit 0
