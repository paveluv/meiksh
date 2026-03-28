# Test: SHALL-19-09-03-002
# Obligation: "A ';' separator ... shall cause the preceding AND-OR list to be
#   executed sequentially; an '&' separator or terminator shall cause
#   asynchronous execution of the preceding AND-OR list."
# Verifies: ; is sequential, & is asynchronous.

tmpf="$TMPDIR/shall-19-09-03-002.$$"
trap 'rm -f "$tmpf"' EXIT

# Sequential: second command sees effect of first
printf '%s' "A" >"$tmpf"; printf '%s' "B" >>"$tmpf"
content=$(cat "$tmpf")
if [ "$content" != "AB" ]; then
    printf '%s\n' "FAIL: sequential ; did not execute in order" >&2
    exit 1
fi

# Async: & should not block
true &
wait
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: wait after & failed" >&2
    exit 1
fi

exit 0
