# Test: SHALL-19-09-01-03-002
# Obligation: "Additionally, if there is no command name but the command
#   contains a command substitution, the command shall complete with the exit
#   status of the command substitution whose exit status was the last to be
#   obtained. Otherwise, the command shall complete with a zero exit status."
# Verifies: no-command-name exit status from last command substitution.

# No command substitution: exit status 0
"$SHELL" -c 'X=hello'; s1=$?
if [ "$s1" -ne 0 ]; then
    printf '%s\n' "FAIL: assignment-only should exit 0, got $s1" >&2
    exit 1
fi

# With command substitution: exit status from last subst
"$SHELL" -c 'X=$(exit 42)'; s2=$?
if [ "$s2" -ne 42 ]; then
    printf '%s\n' "FAIL: exit status should be 42 from \$(exit 42), got $s2" >&2
    exit 1
fi

# Multiple substitutions: last one wins
"$SHELL" -c 'X=$(exit 1) Y=$(exit 7)'; s3=$?
if [ "$s3" -ne 7 ]; then
    printf '%s\n' "FAIL: exit status should be 7 from last subst, got $s3" >&2
    exit 1
fi

exit 0
