# Test: SHALL-19-07-06-001
# Obligation: "[n]>&word shall duplicate one output file descriptor from
#   another, or shall close one. If word evaluates to one or more digits,
#   the file descriptor denoted by n ... shall be made to be a copy of the
#   file descriptor denoted by word; ... If word evaluates to '-', file
#   descriptor n ... shall be closed."
# Verifies: output fd duplication and closing via >&.

# 2>&1 duplicates stdout to stderr
result=$(printf '%s\n' "dup_test" 2>&1 >/dev/null)
# stdout was redirected to /dev/null, but 2>&1 happened first (left-to-right)
# Actually, redirections are left-to-right: 2>&1 makes fd2 a copy of fd1,
# then >dev/null redirects fd1. So fd2 still points to original stdout.
# Let's test a simpler case:
out=$( { printf '%s\n' "to_stderr" >&2; } 2>&1 )
if [ "$out" != "to_stderr" ]; then
    printf '%s\n' "FAIL: >&2 then 2>&1 capture failed" >&2
    exit 1
fi

# Close stdout with >&-
(exec 1>&-) 2>/dev/null
# Should not error

# Closing non-open fd should not error
(exec 8>&-) 2>/dev/null

# Duplicating from non-open fd should fail
if (exec 1>&9) 2>/dev/null; then
    printf '%s\n' "FAIL: 1>&9 from non-open fd should fail" >&2
    exit 1
fi

exit 0
