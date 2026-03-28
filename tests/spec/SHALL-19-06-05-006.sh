# Test: SHALL-19-06-05-006
# Obligation: "If the IFS variable is unset, then [...] its value shall be
#   considered to contain the three single-byte characters <space>, <tab>, and
#   <newline>."
# Verifies: unset IFS defaults to space/tab/newline for splitting.

count_args() { printf '%s\n' "$#"; }

unset IFS
val="one two	three"
n=$(count_args $val)
if [ "$n" != "3" ]; then
    printf '%s\n' "FAIL: unset IFS did not split on space/tab: got $n" >&2
    IFS=' '
    exit 1
fi

IFS=' '
exit 0
