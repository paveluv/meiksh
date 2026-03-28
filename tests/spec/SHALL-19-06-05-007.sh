# Test: SHALL-19-06-05-007
# Obligation: "The shell shall use the byte sequences that form the characters
#   in the value of the IFS variable as delimiters. [...] these delimiters
#   terminate a field; they do not, of themselves, cause a new field to start."
# Verifies: IFS chars are terminators; leading/trailing whitespace is ignored.

count_args() { printf '%s\n' "$#"; }

IFS=' '
val="  one  two  "
n=$(count_args $val)
if [ "$n" != "2" ]; then
    printf '%s\n' "FAIL: leading/trailing IFS whitespace created extra fields: $n" >&2
    exit 1
fi

# Non-whitespace IFS delimiter
IFS=':'
val2="a:b:c"
n2=$(count_args $val2)
if [ "$n2" != "3" ]; then
    printf '%s\n' "FAIL: IFS=: split a:b:c into $n2 fields, expected 3" >&2
    IFS=' '
    exit 1
fi

IFS=' '
exit 0
