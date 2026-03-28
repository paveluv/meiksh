# Test: SHALL-18-04-002
# Obligation: "when given an option unrecognized by the implementation ...
#   standard utilities shall issue a diagnostic message to standard error
#   and exit with an exit status that indicates an error occurred."
# Also: "Standard utilities that do not accept options, but that do accept
#   operands, shall recognize '--' as a first argument to be discarded."
# Verifies: Unrecognized option causes error; -- is recognized.

# cd with unrecognized option should fail with diagnostic
err=$(cd --bogus-option 2>&1)
rc=$?
if [ "$rc" = "0" ]; then
    printf '%s\n' "FAIL: cd --bogus-option should have returned nonzero" >&2
    exit 1
fi

# true accepts no options; -- should be discarded silently
true -- ; rc=$?
if [ "$rc" != "0" ]; then
    printf '%s\n' "FAIL: true -- should succeed" >&2; exit 1
fi

exit 0
