# Test: SHALL-19-09-05-01-001
# Obligation: "The exit status of a function definition shall be zero if the
#   function was declared successfully ... The exit status of a function
#   invocation shall be the exit status of the last command executed by the
#   function."
# Verifies: Function definition and invocation exit status.

# Successful definition: exit status 0
myfn() { true; }
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: successful function def should exit 0, got $rc" >&2
    exit 1
fi

# Invocation exit status = last command in body
retfalse() { false; }
retfalse
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: function ending with false should exit 1, got $rc" >&2
    exit 1
fi

# return overrides
retval() { return 42; }
retval
rc=$?
if [ "$rc" -ne 42 ]; then
    printf '%s\n' "FAIL: return 42 should exit 42, got $rc" >&2
    exit 1
fi

exit 0
