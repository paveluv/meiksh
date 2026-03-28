# Test: SHALL-19-02-01-001
# Obligation: "A <backslash> that is not quoted shall preserve the literal
#   value of the following character, with the exception of a <newline>.
#   If a <newline> immediately follows the <backslash>, the shell shall
#   interpret this as line continuation."
# Verifies: Backslash escaping and line continuation.

# Backslash preserves literal value of $
r=$(printf '%s' \$HOME)
[ "$r" = '$HOME' ] || { printf '%s\n' "FAIL: \\$ not literal" >&2; exit 1; }

# Backslash preserves literal value of backslash
r=$(printf '%s' \\)
[ "$r" = '\' ] || { printf '%s\n' "FAIL: \\\\ not literal backslash" >&2; exit 1; }

# Line continuation: backslash-newline removed, tokens joined
r=$(eval 'printf "%s" hel\
lo')
[ "$r" = "hello" ] || { printf '%s\n' "FAIL: line continuation, got '$r'" >&2; exit 1; }

# Line continuation does not create whitespace (cannot serve as token separator)
r=$(eval 'printf "%s" "ab\
cd"')
[ "$r" = "abcd" ] || { printf '%s\n' "FAIL: continuation in dquote, got '$r'" >&2; exit 1; }

exit 0
