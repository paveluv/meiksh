# Test: SHALL-19-22-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid exit produces no stderr
err=$( (exit 0) 2>&1)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid exit produced stderr: $err" >&2
    exit 1
fi

exit 0
