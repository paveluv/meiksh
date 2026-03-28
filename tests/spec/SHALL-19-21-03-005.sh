# Test: SHALL-19-21-03-005
# Obligation: "The exec special built-in shall support XBD 12.2 Utility Syntax
#   Guidelines."

# exec supports -- as end-of-options
result=$(exec -- printf '%s' "after_dashdash")
if [ "$result" != "after_dashdash" ]; then
    printf '%s\n' "FAIL: exec -- did not pass utility correctly" >&2
    exit 1
fi

exit 0
