# Test: SHALL-19-02-04-003
# Obligation: "\' yields an <apostrophe> (single-quote) character."
# Verifies: \' in $'...' produces a single-quote without terminating.

r=$'it\'s'
[ "$r" = "it's" ] || { printf '%s\n' "FAIL: \\' in \$'' should produce single-quote" >&2; exit 1; }

exit 0
