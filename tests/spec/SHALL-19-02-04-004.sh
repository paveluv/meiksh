# Test: SHALL-19-02-04-004
# Obligation: "\\ yields a <backslash> character."
# Verifies: \\ in $'...' produces a literal backslash.

r=$'\\'
[ "$r" = '\' ] || { printf '%s\n' "FAIL: \\\\ in \$'' should be backslash" >&2; exit 1; }

exit 0
