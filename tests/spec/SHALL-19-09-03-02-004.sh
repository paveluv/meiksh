# Test: SHALL-19-09-03-02-004
# Obligation: "If, and only if, job control is disabled, the standard input for
#   the subshell in which an asynchronous AND-OR list is executed shall initially
#   be assigned to an open file description that behaves as if /dev/null had
#   been opened for reading only."
# Verifies: Async command's stdin reads as empty (job control disabled).

tmpf="$TMPDIR/shall-19-09-03-02-004.$$"
trap 'rm -f "$tmpf"' EXIT

# With job control disabled (default in scripts), stdin of & should be /dev/null
cat >"$tmpf" &
wait
content=$(cat "$tmpf")
if [ -n "$content" ]; then
    printf '%s\n' "FAIL: async stdin was not /dev/null (got data)" >&2
    exit 1
fi

exit 0
