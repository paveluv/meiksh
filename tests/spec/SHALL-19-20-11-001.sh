# Test: SHALL-19-20-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid eval produces no stderr
err=$(eval ':' 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid eval produced stderr: $err" >&2
    exit 1
fi

exit 0
