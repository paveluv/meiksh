# Test: SHALL-19-09-01-04-001
# Obligation: "If a simple command has a command name and an optional list of
#   arguments after word expansion, the following actions shall be performed"
# Verifies: command search and execution occurs for simple commands with a
#   command name (intro obligation; test representative case).

result=$("$SHELL" -c 'printf "%s\n" "executed"')
if [ "$result" != "executed" ]; then
    printf '%s\n' "FAIL: simple command with name not executed" >&2
    exit 1
fi

exit 0
