# Test: SHALL-19-25-11-001
# Obligation: "The standard error shall be used only for diagnostic messages."

# Valid return produces no stderr
f() { return 0; }
err=$(f 2>&1 >/dev/null)
if [ -n "$err" ]; then
    printf '%s\n' "FAIL: valid return produced stderr: $err" >&2
    exit 1
fi

exit 0
