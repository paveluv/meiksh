# Test: SHALL-19-07-001
# Obligation: "The number n is an optional one or more digit decimal number
#   designating the file descriptor number [...] If n is quoted, the number
#   shall not be recognized as part of the redirection expression."
# Verifies: unquoted fd number before redir operator; quoted fd is literal.

# Redirect fd 2 to a file
f="$TMPDIR/shall_19_07_001_$$"
printf '%s\n' "stderr" 2>"$f" >&2
if [ ! -f "$f" ]; then
    printf '%s\n' "FAIL: 2>file did not create file" >&2
    exit 1
fi

# Quoted digit should be literal arg, not fd number
# \2>file means: arg "2", redirect stdout to file
f2="$TMPDIR/shall_19_07_001_b_$$"
result=$(eval 'printf "%s\n" \2' 2>/dev/null)
if [ "$result" != "2" ]; then
    printf '%s\n' "FAIL: quoted \\2 not treated as literal: got '$result'" >&2
    rm -f "$f" "$f2"
    exit 1
fi

rm -f "$f" "$f2"
exit 0
