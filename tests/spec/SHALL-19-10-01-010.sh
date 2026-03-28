# Test: SHALL-19-10-01-010
# Obligation: "The WORD tokens shall have the word expansion rules applied to
#   them immediately before the associated command is executed, not at the time
#   the command is parsed."
# Verifies: Expansion is deferred to execution time, not parse time.

X=parse_time
eval 'X=exec_time; result=$X'
if [ "$result" != "exec_time" ]; then
    printf '%s\n' "FAIL: expansion should use exec-time value, got '$result'" >&2
    exit 1
fi

exit 0
