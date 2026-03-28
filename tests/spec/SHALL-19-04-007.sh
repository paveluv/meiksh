# SHALL-19-04-007
# "When the word time is recognized as a reserved word in circumstances where it
#  would ... be the command name of a simple command that would execute the time
#  utility ... the behavior shall be as specified for the time utility."
# Verify 'time' keyword works when recognized.

fail=0

# 'time' should measure and succeed — output goes to stderr
result=$(eval 'time true' 2>&1)
rc=$?
[ $rc -eq 0 ] || { printf '%s\n' "FAIL: 'time true' exit status $rc" >&2; fail=1; }

exit "$fail"
