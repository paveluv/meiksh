# Test: SHALL-19-09-01-04-003
# Obligation: "If the command name contains at least one <slash>, the shell
#   shall execute a non-built-in utility as described in 2.9.1.6."
# Verifies: slash-containing command name is executed directly, bypassing
#   builtins and functions.

f="$TMPDIR/slash_test_$$"
printf '%s\n' '#!/bin/sh' 'printf "%s\n" "direct_exec"' > "$f"
chmod +x "$f"
result=$("$SHELL" -c "'$f'")
rm -f "$f"
if [ "$result" != "direct_exec" ]; then
    printf '%s\n' "FAIL: slash-containing command not executed directly" >&2
    exit 1
fi

exit 0
