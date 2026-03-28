# Test: SHALL-19-02-03-010
# Obligation: (Duplicate of SHALL-19-02-03-007) Backslash in double-quotes
#   escapes only $ ` \ <newline> and special ".
# Verifies: Same as SHALL-19-02-03-007.

# \x where x is not special: both chars preserved
r=$(printf '%s' "\z")
[ "$r" = '\z' ] || { printf '%s\n' "FAIL: \\z in dquotes" >&2; exit 1; }

# \$ is an escape
r=$(printf '%s' "\$")
[ "$r" = '$' ] || { printf '%s\n' "FAIL: \\\$" >&2; exit 1; }

# \\ is an escape
r=$(printf '%s' "\\")
[ "$r" = '\' ] || { printf '%s\n' "FAIL: \\\\" >&2; exit 1; }

exit 0
