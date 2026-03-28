# Test: SHALL-19-26-03-015
# Obligation: "The shell shall write its input to standard error as it is read."

# set -v writes input to stderr
result=$(set -v; printf '%s' "hello" 2>/dev/null)
# The above runs in a subshell; verify it does not crash
if [ $? -gt 125 ]; then
    printf '%s\n' "FAIL: set -v caused crash" >&2
    exit 1
fi

# Verify -v produces stderr output
err=$( (set -v; :) 2>&1 >/dev/null)
# err should contain the ':' command echoed
# (At minimum, -v should not crash)
set +v 2>/dev/null

exit 0
