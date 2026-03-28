# Test: SHALL-19-06-05-001
# Obligation: "the shell shall scan each field containing results of expansions
#   and substitutions that did not occur in double-quotes for field splitting"
# Verifies: unquoted expansions are subject to field splitting.

count_args() { printf '%s\n' "$#"; }

IFS=' '
val="one two three"
n=$(count_args $val)
if [ "$n" != "3" ]; then
    printf '%s\n' "FAIL: unquoted \$val split into $n fields, expected 3" >&2
    exit 1
fi

# Quoted expansion should NOT be split
n2=$(count_args "$val")
if [ "$n2" != "1" ]; then
    printf '%s\n' "FAIL: quoted \"\$val\" split into $n2 fields, expected 1" >&2
    exit 1
fi

exit 0
