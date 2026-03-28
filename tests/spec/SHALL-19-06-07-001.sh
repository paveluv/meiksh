# Test: SHALL-19-06-07-001
# Obligation: "The quote character sequence <dollar-sign> single-quote and the
#   single-character quote characters (<backslash>, single-quote, and
#   double-quote) that were present in the original word shall be removed
#   unless they have themselves been quoted."
# Verifies: quote removal strips unquoted quoting characters.

# Backslash removed, escaped char remains
result=$(printf '%s\n' he\"llo)
if [ "$result" != 'he"llo' ]; then
    printf '%s\n' "FAIL: backslash-quote not removed: got '$result'" >&2
    exit 1
fi

# Single quotes removed
result2=$(printf '%s\n' 'world')
if [ "$result2" != "world" ]; then
    printf '%s\n' "FAIL: single quotes not removed: got '$result2'" >&2
    exit 1
fi

# Double quotes removed
result3=$(printf '%s\n' "test")
if [ "$result3" != "test" ]; then
    printf '%s\n' "FAIL: double quotes not removed: got '$result3'" >&2
    exit 1
fi

# Quoted quote retained
result4=$(printf '%s\n' "it's")
if [ "$result4" != "it's" ]; then
    printf '%s\n' "FAIL: quoted single-quote not retained: got '$result4'" >&2
    exit 1
fi

exit 0
