# Test: SHALL-19-02-04-018
# Obligation: "These <backslash>-escape sequences shall be processed ...
#   immediately prior to word expansion of the word in which the
#   dollar-single-quotes sequence occurs."
# Verifies: $'...' result is treated as quoted (no splitting/globbing).

# Result of $'...' should not be field-split
r=$'a b c'
set -- "$r"
[ "$#" = "1" ] || { printf '%s\n' "FAIL: \$'a b c' should be 1 field, got $#" >&2; exit 1; }

# Result should not be pathname-expanded
r=$'*'
set -- "$r"
[ "$1" = "*" ] || { printf '%s\n' "FAIL: \$'*' should be literal asterisk" >&2; exit 1; }

exit 0
