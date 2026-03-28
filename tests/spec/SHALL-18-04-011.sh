# Test: SHALL-18-04-011
# Obligation: "If specific numeric values are listed in this section, the
#   system shall use those values for the errors described."
# Also: "When the description of exit status 0 is 'Successful completion',
#   it means that exit status 0 shall indicate that all of the actions the
#   utility is required to perform were completed successfully."
# Verifies: true returns 0, false returns non-zero.

true; rc=$?
if [ "$rc" != "0" ]; then
    printf '%s\n' "FAIL: true exit status should be 0, got $rc" >&2; exit 1
fi

false; rc=$?
if [ "$rc" = "0" ]; then
    printf '%s\n' "FAIL: false exit status should be nonzero, got 0" >&2; exit 1
fi

exit 0
