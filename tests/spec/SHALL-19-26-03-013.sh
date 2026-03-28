# Test: SHALL-19-26-03-013
# Obligation: "Set various options, many of which shall be equivalent to the
#   single option letters."

# -o allexport equivalent to -a
set -o allexport
OTEST_VAR=otest_val
set +o allexport
result=$(printf '%s' "$OTEST_VAR")
if [ "$result" != "otest_val" ]; then
    printf '%s\n' "FAIL: set -o allexport did not work like -a" >&2
    exit 1
fi

# -o noglob equivalent to -f
set -o noglob
result=$(printf '%s' *)
set +o noglob
if [ "$result" != "*" ]; then
    printf '%s\n' "FAIL: set -o noglob did not disable globbing" >&2
    exit 1
fi

# -o errexit equivalent to -e
result=$(set -o errexit; false; printf '%s' "not_reached")
if [ "$result" = "not_reached" ]; then
    printf '%s\n' "FAIL: set -o errexit did not work" >&2
    exit 1
fi

# -o noclobber equivalent to -C
tmpfile="$TMPDIR/setopt_test_$$.txt"
printf '%s\n' "orig" > "$tmpfile"
set -o noclobber
(printf '%s\n' "over" > "$tmpfile") 2>/dev/null
set +o noclobber
rm -f "$tmpfile"

exit 0
