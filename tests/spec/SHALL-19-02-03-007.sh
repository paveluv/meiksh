# Test: SHALL-19-02-03-007
# Obligation: "Outside of '$(...)'  and '${...}' the <backslash> shall
#   retain its special meaning as an escape character only when immediately
#   followed by one of the following characters: $ ` \ <newline>"
# Verifies: Backslash before other chars is literal in double-quotes.

# \n (not a special char) — backslash is literal, both chars preserved
r=$(printf '%s' "\n")
[ "$r" = '\n' ] || { printf '%s\n' "FAIL: \\n in dquotes should be literal '\\n', got '$r'" >&2; exit 1; }

# \t — backslash is literal
r=$(printf '%s' "\t")
[ "$r" = '\t' ] || { printf '%s\n' "FAIL: \\t in dquotes should be literal '\\t', got '$r'" >&2; exit 1; }

# \a — backslash is literal
r=$(printf '%s' "\a")
[ "$r" = '\a' ] || { printf '%s\n' "FAIL: \\a in dquotes should be literal '\\a', got '$r'" >&2; exit 1; }

# Contrast: \$ — backslash IS an escape here
r=$(printf '%s' "\$")
[ "$r" = '$' ] || { printf '%s\n' "FAIL: \\\$ should yield literal $" >&2; exit 1; }

exit 0
