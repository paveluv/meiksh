# Test: SHALL-19-06-05-008
# Obligation: "if the input field is wholly empty or consists entirely of IFS
#   white space, the result shall be zero fields (rather than an empty field)."
# Verifies: wholly empty/IFS-whitespace expansion produces zero fields.

count_args() { printf '%s\n' "$#"; }

IFS=' '
empty=""
n=$(count_args $empty)
if [ "$n" != "0" ]; then
    printf '%s\n' "FAIL: empty expansion gave $n fields, expected 0" >&2
    exit 1
fi

spaces="   "
n2=$(count_args $spaces)
if [ "$n2" != "0" ]; then
    printf '%s\n' "FAIL: all-spaces expansion gave $n2 fields, expected 0" >&2
    exit 1
fi

exit 0
