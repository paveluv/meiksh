# Test: SHALL-19-06-007
# Obligation: "Parameter expansion of the special parameters '@' and '*', as
#   described in 2.5.2 Special Parameters, can create multiple fields or no
#   fields from a single word."
# Verifies: $@ and $* can produce multiple fields or zero fields.

count_args() {
    printf '%s\n' "$#"
}

# With arguments, "$@" should expand to multiple fields
set -- one two three
n=$(count_args "$@")
if [ "$n" != "3" ]; then
    printf '%s\n' "FAIL: \"\$@\" with 3 args gave $n fields, expected 3" >&2
    exit 1
fi

# With no arguments, "$@" should produce zero fields
set --
n=$(count_args "$@")
if [ "$n" != "0" ]; then
    printf '%s\n' "FAIL: \"\$@\" with 0 args gave $n fields, expected 0" >&2
    exit 1
fi

# $* unquoted with multiple args should also produce multiple fields
set -- a b c
n=$(count_args $*)
if [ "$n" != "3" ]; then
    printf '%s\n' "FAIL: \$* with 3 args gave $n fields, expected 3" >&2
    exit 1
fi

exit 0
