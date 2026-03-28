# Test: SHALL-19-06-05-013
# Obligation: "Once the input is empty, the candidate shall become an output
#   field if and only if it is not empty."
# Verifies: trailing IFS delimiters do not produce a trailing empty field.

count_args() { printf '%s\n' "$#"; }

IFS=':'
val="a:b:"
n=$(count_args $val)
if [ "$n" != "2" ]; then
    printf '%s\n' "FAIL: 'a:b:' with IFS=: gave $n fields, expected 2" >&2
    IFS=' '
    exit 1
fi

# Trailing whitespace IFS
IFS=' '
val2="x y "
n2=$(count_args $val2)
if [ "$n2" != "2" ]; then
    printf '%s\n' "FAIL: 'x y ' with IFS=' ' gave $n2 fields, expected 2" >&2
    exit 1
fi

exit 0
