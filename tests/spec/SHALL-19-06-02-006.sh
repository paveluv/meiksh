# Test: SHALL-19-06-02-006
# Obligation: "Use Default Values. If parameter is unset or null, the expansion
#   of word (or an empty string if word is omitted) shall be substituted;
#   otherwise, the value of parameter shall be substituted."
# Verifies: ${param:-word} behavior.

# Unset parameter: use default
unset uvar
result="${uvar:-default}"
if [ "$result" != "default" ]; then
    printf '%s\n' "FAIL: \${uvar:-default} gave '$result'" >&2
    exit 1
fi

# Null parameter: use default
nvar=""
result2="${nvar:-default}"
if [ "$result2" != "default" ]; then
    printf '%s\n' "FAIL: \${nvar:-default} (null) gave '$result2'" >&2
    exit 1
fi

# Set parameter: use parameter value
svar="setval"
result3="${svar:-default}"
if [ "$result3" != "setval" ]; then
    printf '%s\n' "FAIL: \${svar:-default} gave '$result3', expected 'setval'" >&2
    exit 1
fi

# Word omitted: empty string for unset
unset uvar2
result4="${uvar2:-}"
if [ "$result4" != "" ]; then
    printf '%s\n' "FAIL: \${uvar2:-} gave '$result4', expected ''" >&2
    exit 1
fi

exit 0
