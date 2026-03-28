# Test: SHALL-19-02-03-006
# Obligation: "\ ... [backslash] shall retain its special meaning as an
#   escape character" (inside double-quotes, for specific characters).
# Verifies: Backslash escapes $, `, \, and newline inside double-quotes.

# \$ is literal $
r=$(printf '%s' "\$HOME")
[ "$r" = '$HOME' ] || { printf '%s\n' "FAIL: \\\$ in dquotes" >&2; exit 1; }

# \\ is literal \
r=$(printf '%s' "\\")
[ "$r" = '\' ] || { printf '%s\n' "FAIL: \\\\ in dquotes" >&2; exit 1; }

# \` is literal `
r=$(printf '%s' "\`")
[ "$r" = '`' ] || { printf '%s\n' "FAIL: \\\` in dquotes" >&2; exit 1; }

exit 0
