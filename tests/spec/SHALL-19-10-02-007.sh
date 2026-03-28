# Test: SHALL-19-10-02-007
# Obligation: "[Assignment preceding command name] ... If all the characters in
#   the TOKEN preceding the first such <equals-sign> form a valid name ... the
#   token ASSIGNMENT_WORD shall be returned."
# Verifies: Assignment words recognized before command name.

# Simple assignment
FOO=bar
if [ "$FOO" != "bar" ]; then
    printf '%s\n' "FAIL: simple assignment not recognized" >&2
    exit 1
fi

# Multiple assignments before command
A=1 B=2 true
# A and B should NOT persist (prefix assignments with command)
# But the assignments should have been recognized

# Assignment with command
result=$(X=hello eval 'printf "%s" "$X"')
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: prefix assignment not passed to command" >&2
    exit 1
fi

# Token starting with = is WORD, not ASSIGNMENT_WORD
# =foo should be a command name attempt, not an assignment
=foo 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s\n' "FAIL: =foo should not be treated as assignment" >&2
    exit 1
fi

exit 0
