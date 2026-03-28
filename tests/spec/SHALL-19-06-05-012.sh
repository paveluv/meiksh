# Test: SHALL-19-06-05-012
# Obligation: core field splitting algorithm: IFS whitespace coalesces,
#   non-whitespace IFS chars produce empty fields between them.
# Verifies: detailed splitting behavior with mixed IFS characters.

count_args() { printf '%s\n' "$#"; }
get_arg() { shift $(($1)); printf '%s\n' "$1"; }

# Non-whitespace IFS: adjacent delimiters produce empty fields
IFS=':'
val="a::b"
n=$(count_args $val)
if [ "$n" != "3" ]; then
    printf '%s\n' "FAIL: a::b with IFS=: gave $n fields, expected 3" >&2
    IFS=' '
    exit 1
fi

# Whitespace IFS coalesces: multiple spaces = one delimiter
IFS=' '
val2="a    b"
n2=$(count_args $val2)
if [ "$n2" != "2" ]; then
    printf '%s\n' "FAIL: 'a    b' with IFS=' ' gave $n2 fields, expected 2" >&2
    exit 1
fi

exit 0
