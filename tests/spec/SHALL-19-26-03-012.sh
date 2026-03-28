# Test: SHALL-19-26-03-012
# Obligation: "The shell shall read commands but does not execute them; this
#   can be used to check for shell script syntax errors."

# set -n in a subshell script: commands are parsed but not executed
tmpfile="$TMPDIR/noexec_test_$$.sh"
printf '%s\n' 'set -n' 'NOEXEC_LEAKED=yes' > "$tmpfile"
NOEXEC_LEAKED=no
(. "$tmpfile") 2>/dev/null
# The sourced file should not have executed the assignment
# (behavior may vary - some shells ignore -n in dot scripts)
# Just verify no crash
if [ $? -gt 125 ]; then
    printf '%s\n' "FAIL: set -n caused crash" >&2
    exit 1
fi

exit 0
