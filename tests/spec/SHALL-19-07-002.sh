# Test: SHALL-19-07-002
# Obligation: "The optional number, redirection operator, and word shall not
#   appear in the arguments provided to the command to be executed (if any)."
# Verifies: redirection tokens are stripped from command arguments.

f="$TMPDIR/shall_19_07_002_$$"
result=$(printf '%s\n' hello >$f)
content=""
if [ -f "$f" ]; then
    content=$(cat "$f")
fi
if [ "$content" != "hello" ]; then
    printf '%s\n' "FAIL: redirection word appeared in args or output wrong" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

# Verify no extra args: "echo hello >file" should only print "hello"
f2="$TMPDIR/shall_19_07_002b_$$"
printf '%s\n' hello >"$f2"
content2=$(cat "$f2")
if [ "$content2" != "hello" ]; then
    printf '%s\n' "FAIL: unexpected content in output: '$content2'" >&2
    rm -f "$f2"
    exit 1
fi
rm -f "$f2"
exit 0
