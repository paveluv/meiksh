# Test: SHALL-19-02-04-020
# Obligation: "If a <backslash>-escape sequence represents a single-quote
#   character (for example \'), that sequence shall not terminate the
#   dollar-single-quote sequence."
# Verifies: \' does not terminate $'...'.

r=$'ab\'cd'
[ "$r" = "ab'cd" ] || { printf '%s\n' "FAIL: \\' should not terminate \$'', got '$r'" >&2; exit 1; }

r=$'\''
[ "$r" = "'" ] || { printf '%s\n' "FAIL: \$'\\'' should be single-quote" >&2; exit 1; }

exit 0
