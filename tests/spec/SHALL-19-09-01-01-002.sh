# Test: SHALL-19-09-01-01-002
# Obligation: "The words that are recognized as variable assignments or
#   redirections according to 2.10.2 Shell Grammar Rules are saved for
#   processing in steps 3 and 4."
# Verifies: leading assignments and redirections are recognized and
#   separated from command name/args.

# Assignment before command is recognized
result=$("$SHELL" -c 'MY_VAR=hello sh -c "printf \"%s\n\" \"\$MY_VAR\""')
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: prefix assignment not passed to command" >&2
    exit 1
fi

# Redirection is recognized
f="$TMPDIR/step1_test_$$"
"$SHELL" -c "printf '%s\n' redir_ok > '$f'"
content=$(cat "$f")
rm -f "$f"
if [ "$content" != "redir_ok" ]; then
    printf '%s\n' "FAIL: redirection in simple command not performed" >&2
    exit 1
fi

exit 0
