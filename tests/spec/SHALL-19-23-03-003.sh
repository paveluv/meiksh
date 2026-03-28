# Test: SHALL-19-23-03-003
# Obligation: "The export special built-in shall support XBD 12.2 Utility
#   Syntax Guidelines."

# export supports -- as end-of-options
export -- EXPORT_DASH_TEST=ok
if [ "$EXPORT_DASH_TEST" != "ok" ]; then
    printf '%s\n' "FAIL: export -- did not work" >&2
    exit 1
fi

exit 0
