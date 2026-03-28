# Test: SHALL-19-06-02-009
# Obligation: "Use Alternative Value. If parameter is unset or null, null shall
#   be substituted; otherwise, the expansion of word [...] shall be
#   substituted."
# Verifies: ${param:+word} behavior.

# Unset: substitute null
unset pvar
result="${pvar:+alt}"
if [ "$result" != "" ]; then
    printf '%s\n' "FAIL: \${pvar:+alt} (unset) gave '$result', expected ''" >&2
    exit 1
fi

# Null: substitute null
pvar2=""
result2="${pvar2:+alt}"
if [ "$result2" != "" ]; then
    printf '%s\n' "FAIL: \${pvar2:+alt} (null) gave '$result2', expected ''" >&2
    exit 1
fi

# Set and non-null: substitute word
pvar3="value"
result3="${pvar3:+alternative}"
if [ "$result3" != "alternative" ]; then
    printf '%s\n' "FAIL: \${pvar3:+alternative} gave '$result3'" >&2
    exit 1
fi

exit 0
