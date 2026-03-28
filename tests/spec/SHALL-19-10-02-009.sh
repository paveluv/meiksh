# Test: SHALL-19-10-02-009
# Obligation: "Word expansion and assignment shall never occur, even when
#   required by the rules above, when this rule is being parsed. Each TOKEN
#   that might either be expanded or have assignment applied to it shall
#   instead be returned as a single WORD"
# Verifies: Function body is not expanded at definition time.

EARLY=at_define_time
myfunc() { printf '%s\n' "$EARLY"; }

EARLY=at_call_time
result=$(myfunc)
if [ "$result" != "at_call_time" ]; then
    printf '%s\n' "FAIL: function body expanded at definition time, not call time" >&2
    exit 1
fi

exit 0
