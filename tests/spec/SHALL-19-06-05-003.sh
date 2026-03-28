# Test: SHALL-19-06-05-003
# Obligation: "If the IFS variable is set and has an empty string as its value,
#   no field splitting shall occur. However, if an input field which contained
#   the results of an expansion is entirely empty, it shall be removed."
# Verifies: IFS="" disables splitting; empty expansion fields are removed.

count_args() { printf '%s\n' "$#"; }

IFS=""
val="one two three"
n=$(count_args $val)
if [ "$n" != "1" ]; then
    printf '%s\n' "FAIL: IFS='' split into $n fields, expected 1" >&2
    IFS=' '
    exit 1
fi

# Empty expansion field should be removed
empty=""
n2=$(count_args $empty extra)
if [ "$n2" != "1" ]; then
    printf '%s\n' "FAIL: empty field not removed with IFS='': got $n2 args" >&2
    IFS=' '
    exit 1
fi

IFS=' '
exit 0
