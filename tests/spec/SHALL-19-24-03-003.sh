# Test: SHALL-19-24-03-003
# Obligation: "The readonly special built-in shall support XBD 12.2 Utility
#   Syntax Guidelines."

readonly -- RO_DASH=ok
if [ "$RO_DASH" != "ok" ]; then
    printf '%s\n' "FAIL: readonly -- did not work" >&2
    exit 1
fi

exit 0
