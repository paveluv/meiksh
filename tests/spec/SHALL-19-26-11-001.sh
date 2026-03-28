# Test: SHALL-19-26-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid set operations produce no stderr
err=$(set -- a b c 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid set produced stderr: $err" >&2
    exit 1
fi

exit 0
