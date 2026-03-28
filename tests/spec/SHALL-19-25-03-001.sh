# Test: SHALL-19-25-03-001
# Obligation: "The return utility shall cause the shell to stop executing the
#   current function or dot script."

# return exits a function
f() {
    return
    printf '%s\n' "FAIL: reached code after return" >&2
    exit 1
}
f
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: return in function did not work" >&2
    exit 1
fi

# return exits a dot script
tmpfile="$TMPDIR/return_dot_$$.sh"
printf '%s\n' 'return 0' 'RETURN_LEAKED=yes' > "$tmpfile"
. "$tmpfile"
rm -f "$tmpfile"
if [ "$RETURN_LEAKED" = "yes" ]; then
    printf '%s\n' "FAIL: return in dot script did not stop execution" >&2
    exit 1
fi

exit 0
