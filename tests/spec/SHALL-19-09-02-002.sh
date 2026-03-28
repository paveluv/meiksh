# Test: SHALL-19-09-02-002
# Obligation: "If the pipeline begins with the reserved word ! and command1 is
#   a subshell command, the application shall ensure that the ( operator at the
#   beginning of command1 is separated from the ! by one or more <blank>
#   characters."
# Verifies: "! (cmd)" with blank is accepted and negates exit status.

! (false)
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: ! (false) should exit 0, got $rc" >&2
    exit 1
fi

! (true)
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: ! (true) should exit 1, got $rc" >&2
    exit 1
fi

exit 0
