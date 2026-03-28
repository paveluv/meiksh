# Test: SHALL-19-09-01-04-005
# Obligation: "If the command name contains at least one <slash>, the shell
#   shall execute a non-built-in utility as described in 2.9.1.6."
# Duplicate of SHALL-19-09-01-04-003 — same requirement.
# Verifies: slash-containing names executed directly.

f="$TMPDIR/slash_dup_$$"
printf '%s\n' '#!/bin/sh' 'printf "%s\n" "direct"' > "$f"
chmod +x "$f"
result=$("$SHELL" -c "'$f'")
rm -f "$f"
if [ "$result" != "direct" ]; then
    printf '%s\n' "FAIL: slash command not executed directly" >&2
    exit 1
fi

exit 0
