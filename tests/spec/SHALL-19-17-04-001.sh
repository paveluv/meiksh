# Test: SHALL-19-17-04-001
# Obligation: "This utility shall not recognize the \"--\" argument in the
#   manner specified by Guideline 10 of XBD 12.2 Utility Syntax Guidelines."

# -- is just another argument to colon, not end-of-options
: -- foo bar
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: colon with -- did not return 0" >&2
    exit 1
fi

exit 0
