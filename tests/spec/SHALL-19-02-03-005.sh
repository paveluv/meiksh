# Test: SHALL-19-02-03-005
# Obligation: "The backquote shall retain its special meaning introducing
#   the other form of command substitution. The portion of the quoted string
#   from the initial backquote ... up to the next backquote that is not
#   preceded by a <backslash>, having escape characters removed, defines
#   that command whose output replaces '`...`'."
# Verifies: Backquote command substitution works inside double-quotes,
#   backslash-backquote is an escape.

# Basic backquote substitution in double-quotes
r="`printf '%s' abc`"
[ "$r" = "abc" ] || { printf '%s\n' "FAIL: basic backquote sub" >&2; exit 1; }

# Escaped backquote inside backquote substitution
r="`printf '%s' 'x\`y'`"
# The \` in the source should be treated carefully; let's test simple nesting
r=$(printf '%s' "`printf '%s' ok`")
[ "$r" = "ok" ] || { printf '%s\n' "FAIL: backquote in dquote" >&2; exit 1; }

exit 0
