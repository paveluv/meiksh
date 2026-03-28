# Test: SHALL-19-09-02-001
# Obligation: "For each command but the last, the shell shall connect the
#   standard output of the command to the standard input of the next command
#   as if by creating a pipe"
# Verifies: Pipe connects stdout of left to stdin of right.

result=$(printf '%s\n' "hello world" | cat)
if [ "$result" != "hello world" ]; then
    printf '%s\n' "FAIL: pipe did not connect stdout to stdin" >&2
    exit 1
fi

# Multi-stage pipeline
result=$(printf '%s\n' "abc" | cat | cat | cat)
if [ "$result" != "abc" ]; then
    printf '%s\n' "FAIL: multi-stage pipeline failed" >&2
    exit 1
fi

exit 0
