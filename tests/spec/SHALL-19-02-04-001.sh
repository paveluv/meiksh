# Test: SHALL-19-02-04-001
# Obligation: "A sequence of characters starting with a <dollar-sign>
#   immediately followed by a single-quote ($') shall preserve the literal
#   value of all characters up to an unescaped terminating single-quote,
#   with the exception of certain <backslash>-escape sequences."
# Verifies: $'...' quoting preserves literals; no expansion occurs inside.

# Literal characters are preserved
r=$'hello world'
[ "$r" = "hello world" ] || { printf '%s\n' "FAIL: literal text in \$''" >&2; exit 1; }

# No parameter expansion inside $'...'
X=nope
r=$'$X'
[ "$r" = '$X' ] || { printf '%s\n' "FAIL: \$X should be literal in \$''" >&2; exit 1; }

# No command substitution
r=$'$(echo nope)'
[ "$r" = '$(echo nope)' ] || { printf '%s\n' "FAIL: \$() should be literal in \$''" >&2; exit 1; }

exit 0
