# Test: SHALL-19-09-05-002
# Obligation: "When the function is declared, none of the expansions in 2.6
#   Word Expansions shall be performed on the text in compound-command ...
#   all expansions shall be performed as normal each time the function is called."
# Verifies: Function body expansion is deferred to invocation time.

X=before
showx() { printf '%s\n' "$X"; }

result=$(showx)
if [ "$result" != "before" ]; then
    printf '%s\n' "FAIL: initial invocation wrong: got '$result'" >&2
    exit 1
fi

X=after
result=$(showx)
if [ "$result" != "after" ]; then
    printf '%s\n' "FAIL: deferred expansion not working: got '$result'" >&2
    exit 1
fi

exit 0
