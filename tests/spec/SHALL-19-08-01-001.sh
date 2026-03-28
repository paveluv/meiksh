# Test: SHALL-19-08-01-001
# Obligation: "Certain errors shall cause the shell to write a diagnostic
#   message to standard error and exit as shown in the following table"
# Verifies: shell writes diagnostic to stderr on errors (syntax error as
#   representative case).

err=$("$SHELL" -c 'if then fi' 2>&1 >/dev/null)
if [ -z "$err" ]; then
    printf '%s\n' "FAIL: no diagnostic on stderr for syntax error" >&2
    exit 1
fi

exit 0
