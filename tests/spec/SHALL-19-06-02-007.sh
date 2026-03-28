# Test: SHALL-19-06-02-007
# Obligation: "Assign Default Values. If parameter is unset or null, quote
#   removal shall be performed on the expansion of word and the result [...]
#   shall be assigned to parameter. In all cases, the final value of parameter
#   shall be substituted."
# Verifies: ${param:=word} assigns and substitutes.

# Unset: should assign and return
unset avar
result="${avar:=assigned}"
if [ "$result" != "assigned" ]; then
    printf '%s\n' "FAIL: \${avar:=assigned} returned '$result'" >&2
    exit 1
fi
if [ "$avar" != "assigned" ]; then
    printf '%s\n' "FAIL: avar not actually assigned (got '$avar')" >&2
    exit 1
fi

# Null: should assign and return
bvar=""
result2="${bvar:=filled}"
if [ "$result2" != "filled" ]; then
    printf '%s\n' "FAIL: \${bvar:=filled} (null) returned '$result2'" >&2
    exit 1
fi
if [ "$bvar" != "filled" ]; then
    printf '%s\n' "FAIL: bvar not actually assigned (got '$bvar')" >&2
    exit 1
fi

# Already set: should NOT assign, return existing value
cvar="existing"
result3="${cvar:=newval}"
if [ "$result3" != "existing" ]; then
    printf '%s\n' "FAIL: \${cvar:=newval} returned '$result3'" >&2
    exit 1
fi
if [ "$cvar" != "existing" ]; then
    printf '%s\n' "FAIL: cvar was overwritten (got '$cvar')" >&2
    exit 1
fi

exit 0
