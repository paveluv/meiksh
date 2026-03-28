# Test: SHALL-19-09-01-06-004
# Obligation: "If the command name contains at least one <slash>: If the named
#   utility exists, the shell shall execute the utility ... If the named utility
#   does not exist, the command shall fail with an exit status of 127 and the
#   shell shall write an error message."
# Verifies: Slash-containing commands execute or yield 127 if not found.

# A command with a slash that exists should run
result=$(/bin/echo hello 2>/dev/null)
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: /bin/echo did not produce expected output" >&2
    exit 1
fi

# A nonexistent path should produce exit status 127
/no/such/command/ever 2>/dev/null
rc=$?
if [ "$rc" -ne 127 ]; then
    printf '%s\n' "FAIL: nonexistent /path command exited $rc, expected 127" >&2
    exit 1
fi

exit 0
