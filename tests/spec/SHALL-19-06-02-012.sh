# Test: SHALL-19-06-02-012
# Obligation: "pattern matching notation (see 2.14 Pattern Matching Notation),
#   rather than regular expression notation, shall be used to evaluate the
#   patterns. [...] Enclosing the full parameter expansion string in
#   double-quotes shall not cause the [...] pattern characters to be quoted."
# Verifies: pattern expansion uses glob notation; double-quoting outer
#   expansion does not quote the pattern inside.

x="file.tar.gz"

# % with glob pattern
result="${x%.*}"
if [ "$result" != "file.tar" ]; then
    printf '%s\n' "FAIL: \${x%.*} gave '$result', expected 'file.tar'" >&2
    exit 1
fi

# Double-quoting outer expansion should NOT quote pattern chars inside
result2="${x%.*}"
if [ "$result2" != "file.tar" ]; then
    printf '%s\n' "FAIL: double-quoted \${x%.*} gave '$result2'" >&2
    exit 1
fi

# Empty pattern: no removal
result3="${x%}"
if [ "$result3" != "file.tar.gz" ]; then
    printf '%s\n' "FAIL: \${x%} (empty pattern) gave '$result3'" >&2
    exit 1
fi

exit 0
