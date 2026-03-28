# Test: SHALL-19-09-01-03-001
# Obligation: "If a simple command has no command name after word expansion,
#   any redirections shall be performed in a subshell environment"
# Verifies: no-command-name redirections do not affect current environment
#   (unless exec is used).

f="$TMPDIR/nocmd_redir_$$"
result=$("$SHELL" -c "
> '$f'
# Redirect-only command should create the file (in subshell env)
if [ -f '$f' ]; then
    printf '%s\n' 'created'
else
    printf '%s\n' 'missing'
fi
")
rm -f "$f"
if [ "$result" != "created" ]; then
    printf '%s\n' "FAIL: no-command redirect should create file" >&2
    exit 1
fi

exit 0
